Abstractions - PyProxySession and PyProxyAtom
-------------------------------------------------

A client application can open a pool of **TCP Streams**,
then send and receive **data** over said **TCP Streams**.

A client application can open a pool of **PyProxySession**,
then send **PyProxyAtom** and receive **PyProxyOutput** over said **PyProxySession**.

We continue our discussion of this concepts below.

Existing Technologies - Terminology
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

We begin our discussion with several existing technologies.
This will establish the terminology used for the remainder of this
article.

A TCP socket connection has a **session**.

A Linux process is started as a certain *user*, inside a certain *directory*.
We say a process must load any *state*, (read from DB and/or file system to get data etc.)

The author defines a **laptop-like-node** as follows:

  - One where a person is likely interacting and thus must be responsive to events
  - Spinning up and killing threads dynamically is not only okay, but in many
    cases a good design.


Existing Architectures
~~~~~~~~~~~~~~~~~~~~~~~~~

We will be comparing PyProxy architecture will a variety


PyProxySession - Abstraction
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

PyProxySession is best viewed as an abstraction over a TCP Stream.
Like a TCP Stream - one needs to connect, then any amount of information
can be sent (two ways) over the stream.

A TCP stream can send and receive *bytes*.
By contrast a PyProxySession can only send **PyProxyAtom**
and receive the outputs of PyProxyAtom (see below).

Every PyProxySession is given *exactly* one **session_id** by the server.
A PyProxySession, like a TCP Stream cannot be re-opened when it is closed,
one can only create a new PyProxySession.

Conceptually PyProxySession holds no state.

One can *send* information:

  - PyProxyAtom

Or one can *receive* information:

  - logs for given PyProxySession
  - stdout for PyProxyAtom sent on this PyProxySession
  - stderr for PyProxyAtom sent on this PyProxySession


PyProxySession - Implementation
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

A PyProxySession encapsulates exactly two TCP streams.
We name these two TCP streams, *main-stream* and *output-stream*.

main-stream is a client-request -> server-response protocol.
The client may fire many requests, before waiting for responses.

output-stream is purely server-push.
Data pushed on this stream stream include stdout/stderr of **remote process**
as well as logs pushed to the client from the server.

From the clients implementation perspective each PyProxySession gets it's
own **remote-process**. In practice the server does some clever multiplexing
of many main-stream over a process pool and server-push stdout/stderr bytes
to the relevant PyProxySession as required.

PyProxySession encapsulates *exactly* one background thread.
This background thread will never ask for the Python Global Interpreter Lock.
The author's reasoning is that if running on a **laptop-like-node**, it is
advantageous to let PyProxySession threads run/die/crash while keeping the main
Python interpreter responsive.
We further note the comparison to a web browser and it's heavy use of threads.

**Example** - creating a pool of PyProxySession and a round-robin load balancer.

.. code-block:: python

  # proxy pool
  py_proxy_pool = [
    PyProxySession("localhost:9000").connect(),
    PyProxySession("localhost:9000").connect(),
    PyProxySession("localhost:9000").connect(),
    PyProxySession("localhost:9000").connect(),
    PyProxySession("localhost:9000").connect(),
  ]

  # round-robin for sending things
  from itertools import cycle
  py_proxy_load_balancer = cycle(py_proxy_pool)

.. note::
  By design, if a PyProxySession dies for **any reason**
  (e.g thread crashes, server sends TCP close, network connection drops)
  then like TCP sessions in practice, the old session is **not** recoverable.
  You can however open a new one :)


PyProxyAtom - Abstraction
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

A PyProxyAtom is an executable unit of python source code.
One may send one or many PyProxyAtom over a PyProxySession.

One defines a PyProxyAtom with three things:

  1. Source Code or a Code Object
  2. Python Locals
  3. Python Globals


Pythonistas will recognise the similarity to the python **eval** function.
A PyProxyAtom has the following outputs, sent from server to client
over a PyProxySession:

 - stdout/stderr
 - return value of python **eval**

PyProxyAtom - Implementation
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

**EXample** - Run a python function remotely (continued from above)

.. code-block:: python

  def my_func():
      print("hello world")
      return 5

  # py_proxy_load_balancer is defined above
  py_proxy_session = next(py_proxy_load_balancer)
  future = py_proxy_session.eval(my_func, locals=None, globals=None)

  # print all stdout/stderr lines from my_func
  # the method output will block until the future is complete
  from sys import stderr
  for (fd, line) in py_proxy_session.output(future=future):
      if fd == 1:
          print(line)
      elif fd == 2:
          print(line, file=stderr)
      else:
          raise ValueError(f"unrecognised file descriptor {fd}")

  # print the return value
  print("output = ", future.wait())
