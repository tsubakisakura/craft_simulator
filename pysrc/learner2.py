import os
import sys
import subprocess

import tools
from credentials import *

def run_learner2(args):
    path = "target/release/craft-simulator"
    env = os.environ.copy()
    env["MYSQL_PASSWORD"]=tools.get_mysql_password()
    cmdline = [path,"learner",
        "--mysql-user",mysql_user]
    if args.flamegraph:
        cmdline.append("--flamegraph")
    proc = subprocess.run(cmdline, stdout=sys.stdout, env=env)
    if proc.returncode != 0:
        print("Simulator has exit with error code:" + str(proc.returncode))
        return False

    return True

def command_learner2(args):
    tools.with_ssh_tunnel( lambda: run_learner2( args ) )
