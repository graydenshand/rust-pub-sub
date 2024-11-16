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

- Extensible server class (e.g. custom handlers)
- Unsubscribe function on client
- Robust error handling (retries & graceful failures)
    - Server disconnect, client retry logic
- Python bindings
- TLS/SSL
- Authentication
- History/Replay server for processing historical data
- Archive data format (iceberg table)
