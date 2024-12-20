# blastrs

A fast & flexible Pub/Sub broker, with a lightweight protocol over msgpack.

## Priorities

- Flexible: Using a flexible, self describing protocol supports a flexible and versitile message broker
- Performant: Async. Multi-core. Compression. Horizontal scaling.
- Usable: Simple, minimal interface. Open protocol.

## Example

The following example demonstrates how to connect to a server, and subscribe to every topic.

```rs
use g_pubsub::client::Client;

// Connect to the server running at the specified address
let mut client = Client::new("127.0.0.1:36912".to_string()).await;

// Subscribe to all topics using the wildcard character
client.subscribe("*");

// We'll wait as long as needed for a message from the server
let receive_timeout = None;

// Print messages until there the connection is broken
while let Some(message: Message) = client.recv(receive_timeout).await { 
    let topic = message.topic;
    let value = message.value.to_string();
    debug!("Message received - {topic} - {value}");
}
```

## Status

Work in progress.

### Backlog

Core
- High availability
- Horizontal scaling (cluster mode)
- Replay
- Websocket port
- UDP port

Usability
- Python bindings
- Thread based (not async) client
- Automatic reconnect
- Docker image

Security
- TLS
- Access control
