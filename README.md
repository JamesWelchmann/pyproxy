
# PyProxy

PyProxy is an experimental (very!) remote python code execution tool.

Please do not use this in production anywhere!

## Building

PyProxy contains two parts:

  - A server, consisting of two binaries (directory server)
  - A python client library for calling the server (directory client)

In both cases you will need the Rust compiler.
See [https://www.rust-lang.org/tools/install](here) for instructions on obtaining the Rust toolchain.

Once obtained, simple run the following the root directory of this project.

## Building (Linux) - Client + Server

```bash
  $ cargo build --release
```

If that succeeds you should now have three files in `/target/release/`

  - master (binary)
  - worker (binary)
  - libpyproxy_client.so

master and worker are binaries which constitute the server component of pyproxy.
libpyproxy_client.so is a Python extension which can be imported into Python as so.

```bash
  $ cp target/release/libpyproxy_client.so pyproxy_client.so
  $ python
  >>> import pyproxy_client
  >>> pyproxy_client.new_simple_connection
  <built-in function new_simple_connection>
  >>>
```

## Building (OS/X and Windows) - Client

The server is aggressively Linux specific so only the client is available on other platforms.
(NOTE: The Author believes this to be the case, he has not actually tested it).

```
  cd client
  cargo build --release
```

Then either (OS/X):

```
  cp target/release/libpyproxy_client.dylib libpyproxy_client.so
```

or (Windows):

```
  cp target/release/libpyproxy_client.dll libpyproxy_client.pyd
