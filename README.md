# g_pubsub

A TCP Pub/Sub server & client, with a lightweight protocol over msgpack.

## Priorities

- Flexible: Using a flexible, self describing protocol supports a flexible and versitile message broker
- Performant: Async client and server, compressed data over wire to efficiently use network IO
- Usable: Simple, minimal interface, open protocol supports integration with other systems

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

### Roadmap

Security
- TLS & Authentication

Client Usability
- Python bindings
- Thread based (not async) client
- Automatic reconnect
