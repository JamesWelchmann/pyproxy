.. PyProxy documentation master file, created by
   sphinx-quickstart on Sun Mar 26 19:30:12 2023.
   You can adapt this file completely to your liking, but it should at least
   contain the root `toctree` directive.

#############
PyProxy
#############

PyProxy is a server and a client library for remotely executing Python code.
It is very new, so please don't run this in production anywhere!

And here is PyProxy Hello World.

.. code-block:: python

     import sys
     # import client library
     from pyproxy import PyProxySession

     # connect to the server
     with PyProxySession(addr="localhost:9000") as remote_process:

         # send our code to the server for remote exectuon
         # return a future
         future = remote_process.eval('print(hello)', locs={'hello': 'hello world'})

         # block on the future, gathering all stdout and stderr
         for (fd, line) in remote_process.output(future=future):
             if fd == 1:
                 # print to stdout
                 print(line)
            elif fd == 2:
                 # print to stderr
                 print(line, file=sys.stderr)
            else:
                raise RuntimeError(f"unrecognised file descriptor in output {fd}")

        # the future is now complete.
        # the Python builtin function print returns None so this
        # should be the return value from the above eval statement
        assert future.wait() is None, "print function did not return None"


Contents
==========

.. toctree::
   walkthrough.rst
   protocol.rst
   configuration.rst
   api-docs.rst
