# 7. Replay Server

A write-along-log (not ahead) writes every message to persistent storage. Instead of receiving the messages from the
server in ral time, a client can receive messages starting from an arbitrary point in time.

A client listening to the replay server is guaranteed to receive every message, there is no concept of falling behind
like there is when the live server's buffers are full. This means the client can process messages slower than they arrive.

If a client is listening to the replay server but is caught up, it will receive messages later than clients listening
to the live server.

## Storage format

### Postgres

Easily support time indexing.

Flexible querying provides some overlap in capabilities with real-time metrics.

### Log files

Simple dumping of messages with log rotation.

Fewer dependencies.

