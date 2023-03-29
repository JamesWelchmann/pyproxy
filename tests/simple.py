#!/usr/bin/env python
# -*- coding: utf-8 -*-

import unittest
from itertools import cycle

from pyproxy import PyProxySession

class Connect(unittest.TestCase):
    def __init__(self, server, test_name):
        self._server = server
        super().__init__(test_name)

class SimpleTests(unittest.TestCase):
    def __init__(self, server, test_name):
        self._server = server
        super().__init__(test_name)

    def setUp(self):
        addr = f"localhost:{self._server._bind_port}"

        # Create 5 sessions (with 5 threads)
        self._py_proxy_sessions = [
            PyProxySession(addr).connect(),
            PyProxySession(addr).connect(),
            PyProxySession(addr).connect(),
            PyProxySession(addr).connect(),
            PyProxySession(addr).connect(),
        ]

        self._py_proxy_sessions_round_robin = cycle(self._py_proxy_sessions)

    def tearDown(self):
        for s in self._py_proxy_sessions:
            s.disconnect()

    def test_add(self):
        future = next(self._py_proxy_sessions_round_robin).eval("2 + 2")
        self.assertEqual(future.wait(), 4)

    def test_print(self):
        remote_proc = next(self._py_proxy_sessions_round_robin)
        future = remote_proc.eval('print("hello world")')
        output = list(remote_proc.output(future=future))
        assert output == [(1, 'hello world')]


def run(server):
    runner = unittest.TextTestRunner()
    suite = unittest.TestSuite()
    # suite.addTest(SimpleTests(server, "test_add"))
    suite.addTest(SimpleTests(server, "test_add"))

    runner.run(suite)
