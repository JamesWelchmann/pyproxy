#!/usr/bin/env python
# -*- coding: utf-8 -*-

import subprocess as sp
import os
import signal
from random import randint
from pathlib import Path

from tests.simple import run as run_simple

CWD = Path(__file__).absolute().parent
SERVER_BIN = CWD.parent / 'target' / 'release' / 'master'


class Server:
    def __init__(self, path):
        self._path = path
        self._bind_port = randint(6000, 10000)
        self._output_port = randint(6000, 10000)

    def __enter__(self):
        bind_addr = f"0.0.0.0:{self._bind_port}"
        output_addr = f"0.0.0.0:{self._output_port}"
        env = {
            "PYPROXY_BIND_ADDR": bind_addr,
            "PYPROXY_OUTPUT_ADDR": output_addr,
            "PYPROXY_NUM_WORKERS": "5",
        }

        self._proc = sp.Popen([self._path], env=env, shell=False)

        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        os.kill(self._proc.pid, signal.SIGINT)


def main():
    with Server(SERVER_BIN) as server:
        run_simple(server)


if __name__ == '__main__':
    main()
