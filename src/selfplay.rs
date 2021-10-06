﻿
use std::sync::{Arc,Mutex};
use std::sync::mpsc::{channel,Sender,Receiver,TryRecvError};
use std::thread::JoinHandle;
use std::time::{Instant,SystemTime,Duration};
use std::cell::RefCell;
use std::rc::Rc;

use mysql::*;
use serde::Serialize;
use xorshift::{SeedableRng};

use super::selector::{Selector,UCB1Context};
use super::logic::{State,Setting,Action,Modifier};
use super::mcts::{MCTSContext,ActionVector,select_action_weighted,select_action_greedy,get_reward};
use super::writer::*;
use super::cache::*;
use super::executor::*;
use super::predictor::*;

#[derive(Debug,Clone)]
pub enum WriterParameter {
    Evaluation,
    Generation,
}

#[derive(Debug,Clone)]
pub struct EpisodeParameter {
    pub setting : Setting,
    pub mcts_simulation_num : u32,
    pub alpha : f32,
    pub eps : f32,
    pub start_greedy_turn : u32,
}

#[derive(Debug,Clone)]
pub struct SelfPlayParameter {
    pub episode_param : EpisodeParameter,
    pub selector : Selector,
    pub plays_per_write : usize,
    pub mysql_user : String,
    pub thread_num : u32,
    pub batch_size : usize,
    pub writer_param : WriterParameter,
}

#[derive(Serialize)]
pub struct Sample {
    pub action : Action, // 無くても問題ないけどログ見るのに便利なので出しておく
    pub state : State,
    pub mcts_policy : ActionVector,
}

#[derive(Serialize)]
pub struct Replay {
    pub samples : Vec<Sample>,
    pub name : String,
    pub last_state : State,
    pub reward : f32,
}

struct ThreadContext {
    episode_param : EpisodeParameter,
    batch_size : usize,
    selfplay_receiver : Receiver<(String,Arc<tensorflow::Graph>)>,
    writer_sender : Sender<Replay>,
}

struct CoroutineContext {
    episode_param : EpisodeParameter,
    writer_sender : Sender<Replay>,
    predict_queue : PredictQueue,
    graph_info : RefCell<(String,Arc<tensorflow::Graph>)>, // CellはCopy traitを要求します。StringもArcもCloneが無いのでRefCellが必要であるようです
}

async fn selfplay_craftone( param:&EpisodeParameter, graph_filename:&String, predict_queue:&PredictQueue ) -> Replay {

    let seed : u64 = From::from( SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).expect("Failed to get UNIXTIME").subsec_nanos() );
    let seeds = [seed, seed];
    let mut modifier = Modifier { setting:param.setting.clone(), rng:SeedableRng::from_seed(&seeds[..]) };

    let mut samples = vec![];
    let mut state = param.setting.initial_state();

    // コンテキストを１手ごとに初期化するかゲーム中で完全記憶するのが良いかが分かりませんが、一旦ここにしておきます。
    // 多分こっちのほうが良いんだけどメモリは使います
    let mut mcts_context = MCTSContext::new(1.0, param.alpha, param.eps, predict_queue.clone(), graph_filename.clone());

    while !state.is_terminated() {
        let mcts_policy = mcts_context.search(&state, &mut modifier, param.mcts_simulation_num).await;

        let action = if state.turn < param.start_greedy_turn {
            select_action_weighted(&mcts_policy, &mut modifier.rng)
        }
        else {
            select_action_greedy(&mcts_policy, &mut modifier.rng)
        };

        samples.push( Sample { action:action.clone(), state:state.clone(), mcts_policy:mcts_policy } );

        state = state.run_action(&mut modifier,&action);
    }

    // 最終的な報酬を計算します。
    let reward = get_reward(&state,&modifier.setting);

    // 結果を返す
    Replay { samples:samples, name:graph_filename.clone(), last_state:state, reward:reward }
}

async fn selfplay_coroutine( co_ctx:Rc<CoroutineContext> ) {
    loop {
        let (graph_filename,_) = co_ctx.graph_info.borrow().clone();
        let replay = selfplay_craftone(&co_ctx.episode_param, &graph_filename, &co_ctx.predict_queue);
        co_ctx.writer_sender.send(replay.await).unwrap();
    }
}

fn selfplay_thread( ctx:ThreadContext ) {

    // 最初の１つだけ初期化のために同期待ちします
    let graph_info = match ctx.selfplay_receiver.recv() {
        Ok(x) => x,
        Err(_) => return,
    };

    let mut predictor = Predictor::new();
    predictor.load_network( graph_info.0.clone(), &*graph_info.1 );

    // コルーチン間の共有コンテキスト
    let co_ctx = Rc::new(CoroutineContext {
        episode_param:ctx.episode_param,
        writer_sender:ctx.writer_sender,
        predict_queue:predictor.get_queue(),
        graph_info:RefCell::new(graph_info),
    });

    // 非同期Executor
    let mut executor = Executor::new();
    for _ in 0..ctx.batch_size {
        executor.spawn( selfplay_coroutine( co_ctx.clone() ) );
    }

    // 以下制作ループ
    loop {
        // キューにあるだけ取得して最新状態を更新します
        loop {
            match ctx.selfplay_receiver.try_recv() {
                Ok(graph_info) => {
                    predictor.load_network( graph_info.0.clone(), &*graph_info.1 );
                    *co_ctx.graph_info.borrow_mut() = graph_info;
                },
                Err(TryRecvError::Disconnected) => { return },
                Err(TryRecvError::Empty) => { break },
            };
        };

        for _ in 0..5 {
            executor.poll_all();
            predictor.predict_batch( &co_ctx.episode_param.setting );
        }
    }
}

// 戻り値の型は利用者側の都合でVecのタプルで返したほうが良いと思います
fn spawn_selfplay_threads( episode_param:&EpisodeParameter, writer_sender:&Sender<Replay>, thread_num:u32, batch_size:usize ) -> (Vec<JoinHandle<()>>,Vec<Sender<(String,Arc<tensorflow::Graph>)>>) {
    let mut handles = vec![];
    let mut senders = vec![];
    for thread_id in 0..thread_num {
        let (sender,receiver) = channel();
        let ctx = ThreadContext {
            episode_param:episode_param.clone(),
            batch_size:batch_size,
            selfplay_receiver:receiver,
            writer_sender:writer_sender.clone(),
        };
        let handle = std::thread::Builder::new().name(format!("selfplay{}",thread_id)).spawn( move ||{selfplay_thread(ctx);} ).unwrap();
        handles.push(handle);
        senders.push(sender);
    }
    (handles,senders)
}

// ここは借用ではなくmoveである必要があるようです。詳しくはこちら
// https://users.rust-lang.org/t/how-to-join-handles-of-threads/52494
fn wait_threads(handles:Vec<JoinHandle<()>>) {
    for handle in handles {
        handle.join().unwrap();
    }
}

fn write_replays<W:WriteReplay>( mut writer:W, receiver:Receiver<Replay> ) {

    let start = Instant::now();
    let interval = Duration::new(5,0);
    let mut next_time = start + interval;
    let mut replay_count = 0;
    let mut sample_count = 0;

    while let Ok(replay) = receiver.recv() {
        replay_count += 1;
        sample_count += replay.samples.len();

        writer.write_replay(replay).unwrap();

        let now = Instant::now();
        if now >= next_time {
            let duration = now - start;
            let secs = duration.as_millis() as f64 / 1000.0;
            eprintln!("{:.3}[secs] {}[replays] {}[samples] {:.3}[replays/secs] {:.3}[samples/sec]",
                secs, replay_count, sample_count, replay_count as f64 / secs, sample_count as f64 / secs );
            next_time += interval;
        }
    }

    writer.flush().unwrap();
}

fn write_thread( mysql_pool:Arc<Mutex<Pool>>, param:SelfPlayParameter, receiver:Receiver<Replay> ) {
    match &param.writer_param {
        WriterParameter::Evaluation => write_replays( EvaluationWriter::new( mysql_pool, param.plays_per_write ), receiver ),
        WriterParameter::Generation => write_replays( GenerationWriter::new( mysql_pool, param.plays_per_write, param.episode_param.setting.clone() ), receiver ),
    };
}

fn run_simulation(param:&SelfPlayParameter ) {

    let mysql_password = match std::env::var("MYSQL_PASSWORD") {
        Ok(val) => format!(":{}", val ),
        Err(_) => String::new(),
    };

    let url = format!("mysql://{}{}@localhost:3306/craft", param.mysql_user, mysql_password );
    eprintln!("Connect to mysql...");
    let mysql_pool_base = Pool::new_manual(2,2,Opts::from_url(&url).unwrap()).unwrap();
    let mysql_pool = Arc::new(Mutex::new(mysql_pool_base));

    let (writer_sender,writer_receiver) = channel();

    // 並列処理でセルフプレイします
    let (selfplay_handles,selfplay_senders) = spawn_selfplay_threads( &param.episode_param, &writer_sender, param.thread_num, param.batch_size );

    // 書き込みスレッド作成
    let send_param : SelfPlayParameter = param.clone();
    let send_mysql_pool = mysql_pool.clone();
    let writer_handle = std::thread::Builder::new().name("writer".to_string()).spawn( move || { write_thread( send_mysql_pool, send_param, writer_receiver ) } ).unwrap();

    // 以下、終了条件を満たすまで無限ループします
    let mut graph_cache = GraphCache::new();
    let mut ucb1_context = UCB1Context::new( mysql_pool.clone() );

    loop {
        let model = match param.selector {
            Selector::UCB1(x) => ucb1_context.get_ucb1_model(x),
            Selector::Optimistic(x) => ucb1_context.get_optimistic_model(x),
        };

        match model {
            Ok(None) => {
                eprintln!("wait for ucb1 model...");
            },
            Ok(Some(graph_filename)) => {
                let graph = graph_cache.load_graph(&graph_filename).unwrap();
                for sender in &selfplay_senders {
                    sender.send((graph_filename.clone(), graph.clone())).unwrap()
                }
            },
            Err(x) => {
                eprintln!("error on mysql {}", x);
                break;
            },
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
    }

    for sender in &selfplay_senders {
        drop(sender)
    }
    wait_threads(selfplay_handles);
    drop(writer_sender);
    writer_handle.join().unwrap();
}

pub fn run(param:&SelfPlayParameter) {

    eprintln!("selfplay parameters:{:?}", param);
    run_simulation(param);
}
