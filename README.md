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

lbroker promises at-most-once delivery.

The client and server communicate asynchronously over a TCP stream, with no request/response protocol, so the client
cannot know if a published message has delivered to all subscribers.

There is no persistence, so a disconnected client cannot recover messages that it missed when it reconnects.

Mostly this has been chosen to ensure the system is as lightweight and fast as possible.

## Load testing

The CLI comes with two commands useful for load testing. On my 2020 M1 Macbook Pro, I have reached processing rates
beyond 1.7M commands per second - completely exhausting my CPU.

```sh
cargo run -- log-metrics --address localhost:36912
```

This first command logs server metrics to the terminal. The server publishes metrics to the `!system/metrics` topic
prefix.


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

## System topic prefix

The system uses the namespace `!system` for the topics it publishes. While not prohibited, it's best to avoid publishing
to topics with that prefix.

For example, this is used for publishing server metrics.

By default subscribing to all topics `*` will not match system topics. A client can subscribe to all system messages
using the pattern `!system*`.

## Protocol

See `docs/protocol.md` for details on the interface between the client and server.

Refer to `examples/python-client` for an example of interfacing with the server from Python. Given the protocol is a thin wrapper over msgpack, it's trivial to write a client in any language with a msgpack implementation.
