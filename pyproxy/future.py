#!/usr/bin/env python
# -*- coding: utf-8 -*-


from os import urandom


def future_id():
    """
    generate an 8 byte hex string
    these are short as they only intended to be used
    by the client - not by the server
    """
    return urandom(8).hex()


class Future:
    def __init__(self, id, inner_fut):
        self._id = id
        self._inner_fut = inner_fut

    def wait(self, timeout=None):
        """
        wait for a future to complete
        this will also raise any exception
        the future wishes to raise

        We can call wait as many times as we like
        it will reliably produce the same behaviour
        once done
        """
        timeout = timeout or 0
        return self._inner_fut.wait(timeout)

    def is_done(self):
        """
        True if done, False otherwise
        this function is exception safe
        """

        self._inner_fut.try_recv()


    def wait_no_except(self, timeout=None):
        """
        wait_no_except is the same as wait
        except that it will return an exception,
        not raise it
        """
        timeout = timeout or 0
        try:
            return self._inner.wait(timeout)
        except Exception as exc:
            return exc

    @property
    def id(self):
        return self._id
