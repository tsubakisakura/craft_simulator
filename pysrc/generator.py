import os
import sys
import subprocess

import tools
from credentials import *

def run_generator(args):
    path = "target/release/craft-simulator"
    env = os.environ.copy()
    env["LD_LIBRARY_PATH"]="/workdir/target/release/"
    env["MYSQL_PASSWORD"]=tools.get_mysql_password()
    cmdline = [path,"generator",
        "--plays-per-write",str(args.plays_per_write),
        "--thread-num",str(args.thread_num),
        "--mcts-simulation-num",str(args.mcts_simulation_num),
        "--mysql-user",mysql_user]
    if args.flamegraph:
        cmdline.append("--flamegraph")
    proc = subprocess.run(cmdline, stdout=sys.stdout, env=env)
    if proc.returncode != 0:
        print("Simulator has exit with error code:" + str(proc.returncode))
        return False

    return True

def command_generator(args):
    tools.with_ssh_tunnel( lambda: run_generator( args ) )
