#!/usr/bin/env python
# -*- coding: utf-8 -*-
import sys
from functools import partial
from time import sleep

eprint = partial(print, file=sys.stderr)


from pyproxy import PyProxySession

session = PyProxySession()
print("created session")
remote_proc = session.connect()
print("created remote proc")
remote_proc.eval(open("my_code.py").read())

sleep(10)


"""
print("calling PyProxySession()")
with PyProxySession() as remote_proc:
    fut = remote_proc.eval('print("hello world"); 2 + 2')
    for (fd, line) in remote_proc.output(fut):
        if fd == 1:
            print(line)
        elif fd == 2:
            eprint(line)
        else:
            raise RuntimeError("expected fd from remote process to be 1 or 2")

    # future done
    print("output = ", fut.wait())
"""
