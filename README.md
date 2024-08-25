# Project

A TCP Pub/Sub server & client, using msgpack.

## Priorities

- Performant
    - Low latency of messages from publisher to subscriber
    - High throughput (messages sent & received) of server & client
    - Large network
        - Many Producers
        - Many Subscribers
        - Many Subscriptions
    - Efficient resource utilization (cpu & memory)
- Usable
    - Clear Interface
    - Portable / Open
    - Convenient
## Status

Work in progress.

### Roadmap

- Code cleanup, refactoring, etc
- Extensible server class (e.g. custom handlers)
- Unsubscribe function on client
- Robust error handling (retries & graceful failures)
    - Server disconnect, client retry logic
- Benchmarking setup
- Python bindings
- TLS/SSL
- Authentication
- Higher test coverage
- Broker type
    - FIFO: maintains message ordering per connection
    - BURST: high throughput, concurrent message processing
- Catch up / Exactly once level of service (making stream completeness reliable over spotty connection)

