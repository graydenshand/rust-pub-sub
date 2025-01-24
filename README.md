# lbroker

A fast & flexible Pub/Sub broker, with a lightweight protocol over msgpack.

## Priorities

- Flexible: Using a flexible, self describing protocol supports a flexible and versitile message broker
- Performant: Async. Multi-core. Compression. Horizontal scaling.
- Usable: Simple, minimal interface. Open protocol.

## Example

The following example demonstrates how to connect to a server, and subscribe to every topic.

```rs
use lbroker::client::Client;

// Connect to the server running at the specified address
let mut client = Client::new("127.0.0.1:36912".to_string()).await;

// Subscribe to all topics using the wildcard character
client.subscribe("*");

// Print messages until there the connection is broken
while let Some(message: Message) = client.recv().await { 
    let topic = message.topic;
    let value = message.value.to_string();
    debug!("Message received - {topic} - {value}");
}
```

## Status

Work in progress.

