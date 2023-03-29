
Protocol
----------

This page documents the design decisions on the PyProxy Client<->Server Protocol.

There are *two* streams opened by a single PyProxySession,
**mainstream** and **outputstream**.

At a high level mainstream is a request<->response protocol.
NOTE: Unlike the very common HTTP, the server makes no guarantee
about the delivery order of the responses.

outputstream is used for server -> client streaming.
(One can think stdout and stderr streaming).
