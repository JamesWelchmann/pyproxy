#!/usr/bin/env python
# -*- coding: utf-8 -*-
import pickle

from pyproxy_client import PyProxyClient, new_simple_connection

from .future import Future, future_id


class PyProxySession:
    def __init__(self, addr="localhost:9000"):
        self._addr = addr

        # block - waitint for client conenction
        # raise exception if we fail
        self._client_conn = new_simple_connection(addr)

    def __enter__(self):
        return self.connect()

    def connect(self):
        client = PyProxyClient(self._client_conn)
        return RemoteProcess(client)

    def __exit__(self, exc_typ, exc_val, trcb):
        self._client.disconnect()


class RemoteProcess:
    def __init__(self, client):
        self._client = client

    def output(self, fut=None):
        """
        output retrieves stdout and stderr line from the remote process
        """

        if fut is None:
            break_now = True
        elif isinstance(fut, Future):
            break_now = False
        else:
            raise TypeError("output method expected None or Future as first argument")

        while True:
            while True:
                # out is a tuple
                # first arg is 1 or 2 (stdout or stderr)
                # second arg is a string for one pipe line
                out = self._client.next_output()
                if out:
                    yield out
                    continue

                break

            if break_now:
                break

            if fut and fut.is_done():
                # loop once more, then break
                break_now = True

    def stdin(self, lines):
        """
        send lines to stdin on remote process
        NOTE: lines *MUST* be a list, with each element being bytes
        otherwise we raise TypeError
        """

        lines = lines or []
        id = future_id()
        inner_fut = self._client.stdin(id, lines)
        return Future(id, inner_fut)


    def eval(self, code, locs=None, globs=None):
        """
        eval will execute a code object on the remote process
        code may be a str or a code object

        if code_object is a string then we send to the server as a String
        if code_object is a code_object then we attempt to pickle
        """

        id = future_id()

        locs = locs or {}
        globs = globs or {}

        locs = pickle.dumps(locs)
        globs = pickle.dumps(globs)

        if isinstance(code, str):
            inner_fut = self._client.eval_str(id, code, locs, globs)
        else:
            raise TypeError("code must be a string")

        return Future(id, inner_fut)
