# lbroker

A fast & flexible Pub/Sub broker, with a lightweight protocol over msgpack.

## Status

Work in progress.

## Example

The following example demonstrates how to connect to a server, and subscribe to every topic.

```rs
use lbroker::client::Client;

// Connect to the server running at the specified address
let mut client = Client::new("127.0.0.1:36912".to_string()).await;

// Subscribe to all topics
client.subscribe("*");

// Print messages until there the connection is broken
while let Some(message: Message) = client.recv().await { 
    let topic = message.topic;
    let value = message.value.to_string();
    debug!("Message received - {topic} - {value}");
}
```

The server can be started using the CLI.

```sh
cargo run -r -- server -p 36912
```

## Delivery guarantees

There are some policies that users should be aware of:

- lbroker promises at-most-once delivery.
- The client and server communicate asynchronously over a TCP stream. With no request/response protocol, the client cannot know if a published message has been received by the server or delivered to all subscribers.
- There is no persistence, so a disconnected client cannot recover messages that it missed when it reconnects.
- The server maintains a small buffer of messages it has received, and a cursor for each client indicating the last message it's received. When full, the oldest message is deteled from the buffer. The cursors of clients that have fallen behind the oldest message are moved to the most recent message, skipping over the other messages in the buffer. Effectively, if a client doesn't process the messages it receives as fast as the server is sending them, it will not receive all messages messages.

## Test client

The CLI comes with a test client that can be used to send some load to a server.

```sh
cargo run  -r -- test-client --address localhost:36912 --number 30 --interval 0.01 --subscribe 'a'
```

This second command spawns `--number` tasks that concurrently publish to and receive from the server. 

The `--interval` option controls how frequently the test clients emit messages (in seconds), if omitted, the clients publish
as fast as possible.

`--subscribe` controls the subscriptions of the test client, and can be provided multiple times. The test clients spread
their messages evenly over the topics: `a`, `b`, `c`, `d`, `e`, `f`, `g`. In conjunction with the number of clients, this
can be used to control the server's fan-out factor. 30 clients subscribing to the `a` topic means that every message
sent to the server must be delivered 30 times; with 60 clients subsribing to both the `a` and `b` topics each message
must be delivered 120 times.

## Server metrics

The server publishes metrics to the `!system/metrics` topic prefix, and the CLI comes with a command to log these metrics.

```sh
cargo run -- log-metrics --address localhost:36912
```

## System topic prefix

While not prohibited, it's advised to avoid publishing to topics with the `!system` prefix.

By default subscribing to all topics `*` will not match system topics. A client can subscribe to all system messages using the pattern `!system*`.

## Protocol & Multi language support

See `docs/protocol.md` for details on the interface between the client and server.

Refer to `examples/python-client` for an example of interfacing with the server from Python. Given the protocol is a thin wrapper over msgpack, it's trivial to write a client in any language with a msgpack implementation.

## GlobTree

This repo also contains a crate named `glob-tree` containing a data structure for efficiently matching a string against a collection of glob patterns. It is used by lbroker to filter which message is sent to each client per its subscriptions.

