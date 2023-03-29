
Walkthrough
---------------

This page documents a walkthrough of the PyProxy Client API,
along with some relevant implementation details for users.

1. PyProxySession
~~~~~~~~~~~~~~~~~~~

We always start with a PyProxySession.

.. code-block:: python

   session = PyProxySession(addr="localhost:9000")

This code will block our main python thread until the following happens:

#. mainstream TCP connection is established
#. client sends client hello-message to server
#. client waits for server-hello message from server

The session is now established.

We can search server logs for anything on this session with the **session_id**.

.. code-block:: python

   print(session.session_id)
  '265e30837d2153ce526d6a27b654e420'

PyProxy Server will assign a unique session_id to every PyProxySession.

.. note::

   Internally PyProxy server multiplexes M PyProxySession over N processes 
   in a process pool. It is therefore often a good idea to create multiple
   sessions and round robin required remote code execution over them.

.. code-block:: python

   from itertools import cycle

   sessions = cycle([
    PyProxySession(addr='localhost:9000'),
    PyProxySession(addr='localhost:9000'),
    PyProxySession(addr='localhost:9000'),
   ])


2. Connect
~~~~~~~~~~~~~~

Secondly we always call connect.

.. code-block:: python

   remote_process = session.connect()

This call will:

#. spawn an OS thread, which takes ownership of the mainstream
#. the dedicated OS thread will open the output stream

API calls on the remote_process object generally return futures
and send/recv data to the dedicated thread.

.. note::

   The OS threads isolate a PyProxy session to a thread.

   By design these threads never ask for the Python global
   interpreter lock so spinning up many of them doesn't
   affect user interaction with a python prompt.
   (Think Jupyter Notebook).

   The isolation means that the thread dedicated to the
   PyProxySession can crash in any way and other sessions
   are unaffected in their isolation.


3. Eval
~~~~~~~~~~

The RemoteProcess object has a method *eval*.
This is used for sending code to be executed by the server.

.. code-block:: python

   future = remote_process.eval('print(hello)', locs={'hello': "hello world"})

First we discuss the signature of the eval function.

.. code-block:: python

  eval(code, locs=None, globs=None): Future 

We make several notes about this function.

#. In contrast to the builtin Python eval function
   the first argument *code* **MUST** be a string.
#. We use the abbreviations *locs* and *globs* inplace of locals and globals.
   The author feels actually using the symbols "locals" and "globals" is asking
   for trouble.
#. Again in contrast to the builtin eval function, locs and globs must either be
   None, or a dictionary. They **MAY NOT** be some custom mapping, as is allowed
   with the builtin eval.
#. Calls to eval will never block, it'll place the arguments into a queue for
   the background thread to pick up and send to the server.

The return value of eval is a Future object.
Calls to eval *may* raise a SessionError exception (or a subexception of it).

4. Waiting on the Future
~~~~~~~~~~~~~~~~~~~~~~~~~~~
