import argparse
import importlib
import sys

# 新規モデルを作成します。
# 現状はデバッグ用途など単発で行うためのコマンドです。
def command_new(args):
    print("loading tensorflow...", file=sys.stderr)
    learner = importlib.import_module('learner')
    print("done.", file=sys.stderr)
    learner.command_new(args)

# 既にあるサンプルファイルを読み込んで学習します。
# 現状はデバッグ用途など単発で行うためのコマンドです。
def command_fit(args):
    print("loading tensorflow...", file=sys.stderr)
    learner = importlib.import_module('learner')
    print("done.", file=sys.stderr)
    learner.command_fit(args)

# デーモンモードで起動します
def command_learner(args):
    print("loading tensorflow...", file=sys.stderr)
    learner = importlib.import_module('learner')
    print("done.", file=sys.stderr)
    learner.command_learner(args)

def command_learner2(args):
    print("loading modules...", file=sys.stderr)
    learner = importlib.import_module('learner2')
    print("done.", file=sys.stderr)
    learner.command_learner2(args)

def command_evaluator(args):
    print("loading modules...", file=sys.stderr)
    evaluator = importlib.import_module('evaluator')
    print("done.", file=sys.stderr)
    evaluator.command_evaluator(args)

def command_generator(args):
    print("loading modules...", file=sys.stderr)
    generator = importlib.import_module('generator')
    print("done.", file=sys.stderr)
    generator.command_generator(args)

def command_gcs(args):
    print("loading modules...", file=sys.stderr)
    misc = importlib.import_module('misc')
    print("done.", file=sys.stderr)
    misc.command_gcs(args)

def command_upload(args):
    print("loading modules...", file=sys.stderr)
    misc = importlib.import_module('misc')
    print("done.", file=sys.stderr)
    misc.command_upload(args)

def command_download(args):
    print("loading modules...", file=sys.stderr)
    misc = importlib.import_module('misc')
    print("done.", file=sys.stderr)
    misc.command_download(args)

# Pythonのargparseでサブコマンドを実現する
# https://qiita.com/oohira/items/308bbd33a77200a35a3d
def main():
    parser = argparse.ArgumentParser(description='Learn neural network.')
    # 本当ならsubparserを作るときにrequiredフラグを入れたいのですがバグで入れられないようです。
    # このため、もしhandlerが手に入らなかった場合にヘルプを表示します。詳細はこちらのURLを参照。
    # https://bugs.python.org/issue33109
    subparser = parser.add_subparsers()

    parser_new = subparser.add_parser('new', help='create initial network.')
    parser_new.add_argument('filename', help='output filename.')
    parser_new.set_defaults(handler=command_new)

    parser_fit = subparser.add_parser('fit', help='create network adapted to the sample.')
    parser_fit.add_argument('source', help='samples generated by generator.')
    parser_fit.add_argument('destination', help='output filename.')
    parser_fit.add_argument('--epochs', type=int, default=100, help='epoch num.')
    parser_fit.set_defaults(handler=command_fit)

    parser_learner = subparser.add_parser('learner', help='Run learner mode.')
    parser_learner.add_argument('--buffer', type=int, default=200000, help='replay buffer size.')
    parser_learner.add_argument('--epochs', type=int, default=100, help='epoch num.')
    parser_learner.set_defaults(handler=command_learner)

    parser_learner2 = subparser.add_parser('learner2', help='Run learner mode(Rust ver).')
    parser_learner2.add_argument('--buffer', type=int, default=200000, help='replay buffer size.')
    parser_learner2.add_argument('--epochs', type=int, default=100, help='epoch num.')
    parser_learner2.add_argument('--flamegraph', action='store_true', help='output flamegraph')
    parser_learner2.set_defaults(handler=command_learner2)

    parser_generator = subparser.add_parser('generator', help='Run generator mode.')
    parser_generator.add_argument('--plays-per-write', type=int, default=100, help='plays per write.')
    parser_generator.add_argument('--thread-num', type=int, default=4, help='thread num.')
    parser_generator.add_argument('--mcts-simulation-num', type=int, default=500, help='mcts simulation num.')
    parser_generator.add_argument('--flamegraph', action='store_true', help='output flamegraph')
    parser_generator.set_defaults(handler=command_generator)

    parser_evaluator = subparser.add_parser('evaluator', help='run evaluator mode.')
    parser_evaluator.add_argument('--plays-per-write', type=int, default=10, help='plays per write.')
    parser_evaluator.add_argument('--thread-num', type=int, default=4, help='thread num.')
    parser_evaluator.add_argument('--mcts-simulation-num', type=int, default=500, help='mcts simulation num.')
    parser_evaluator.add_argument('--flamegraph', action='store_true', help='output flamegraph')
    parser_evaluator.set_defaults(handler=command_evaluator)

    parser_gcs = subparser.add_parser('gcs', help='run gcs api proxy.')
    parser_gcs.set_defaults(handler=command_gcs)

    parser_upload = subparser.add_parser('upload', help='upload to gcs.')
    parser_upload.add_argument('source', type=str, help='source path.')
    parser_upload.add_argument('destination', type=str, help='destination path.')
    parser_upload.add_argument('--content-type', type=str, default='application/octet-stream', help='content type.')
    parser_upload.set_defaults(handler=command_upload)

    parser_download = subparser.add_parser('download', help='download to gcs.')
    parser_download.add_argument('source', type=str, help='source path.')
    parser_download.add_argument('destination', type=str, help='destination path.')
    parser_download.set_defaults(handler=command_download)

    args = parser.parse_args()
    if hasattr( args, "handler" ):
        args.handler(args)
    else:
        parser.print_help()

if __name__ == '__main__':
    main()
