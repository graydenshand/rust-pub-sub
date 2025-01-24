# Protocol

The protocol is a thin wrapper over [msgpack](https://msgpack.org/), meaning it's trivial to implement a client in any language with a msgpack library.

See the example python-client, or the official rust client, for reference.

## Commands

### Subscribe(pattern)

The client will receive messages on topics matching that pattern.

**Examples**
```toml
{"Subscribe": ["topic/*/pattern"]}
```

### Unsubscribe(pattern)

The client will stop receiving messages on topics matching that pattern.

**Examples**
```toml
{"Unsubscribe": ["topic/*/pattern"]}
```

### Publish(message)

The client publishes a message containing a topic and a value.

**Examples**
```toml
{"Publish": [["a-topic", True]]} # Scalar value
{"Publish": [["my-topic", {"Foo": 0.5}]]} # Nested value
```

## Receives

Every message from the server will be a two element list containing the topic and the message.

**Examples**
```toml
["a-topic", True] # Scalar value
["my-topic", {"Foo": 0.5}] # Nested value
```