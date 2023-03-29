#!/usr/bin/env python
# -*- coding: utf-8 -*-
import pickle
from functools import partial

from pyproxy_client import PyProxyClient, new_simple_connection

from .future import Future, future_id


class PyProxySession:
    """
    PyProxySession is a class for establishing the mainstream
    with the PyProxy Server.
    """
    def __init__(self, addr="localhost:9000"):
        self._addr = addr

        # block - waitint for client conenction
        # raise exception if we fail
        self._client_conn = new_simple_connection(addr)

    @property
    def session_id(self):
        """
        session_id, as sent in the server-hello message
        """
        return self._client_conn.session_id

    def __enter__(self):
        """
        """
        return self.connect()

    def connect(self):
        """
        connect will spawn a background thread
        we return a RemoteProcess object
        """
        client = PyProxyClient(self._client_conn)
        return RemoteProcess(client)

    def __exit__(self, exc_typ, exc_val, trcb):
        self._client.disconnect()


class RemoteProcess:
    """
    RemoteProcess is our object wrapping a single "remote python process",
    we can send code to execute and interact with stdin/stdout and stderr.
    """
    def __init__(self, client):
        self._client = client

    def output(self, fut=None):
        """
        output retrieves stdout and stderr line from the remote process
        """

        if fut:
            func = partial(self._client.wait_output, fut.id)
        else:
            func = self._client.next_output

        while True:
            # out is a tuple
            # first arg is 1 or 2 (stdout or stderr)
            # second arg is a string for one pipe line
            out = func()
            if out:
                yield out
                continue

            break

    def stdin(self, lines):
        """
        stdin will send lines to stdin on the remote process.
        Currently we raise NotImplementedError
        """

        raise NotImplementedError("stdin not implemented")


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
