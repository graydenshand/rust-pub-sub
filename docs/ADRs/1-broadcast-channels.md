# 1. Broadcast channels

Status: Draft

Tokio's [broadcast channels](https://docs.rs/tokio/1.41.0/tokio/sync/broadcast/index.html) support many to many message sending between tasks.

Using broadcast channels would result in a simpler architecture, compared with the current mpsc channel design. In particular the server would not need to maintain a global subscription tree, and client_id-channel mapping.

It would also achieve a system that is more robust to transient connection errors. Similar in goal to the **#0 Write Ahead Log** proposal, broadcaast channels maintain a buffer of messages that are only deleted when all subscribers have received the message or when the buffer is full.

Finally, it would reduce the impact of a noisy neighbor on the performance of other clients workloads.

## System implications

The major implications of this change involve shifting responsibilities away from the `Server` object.

|Before|After|
|---|---|
|The server processes each message, looking up the subscribers to the message topic and forwarding the message to the subscribed clients. | Every client connection processes each message independently. It checks if it's client is subscribed to the topic and forwards the message if true. |
|A global subscription tree that contains every client's subscriptions. | Each client connection stores that client's subscriptions (shared between read and write halfs). Subscription tree needs to be adapted as such to store subscriptions for a single client. |
| When a client disconnects its information is immediately deleted from server's records. | A disconnected client can reconnect and resume the same session. A separate task culls dead sessions. |

### Classes

```
Server { addr }
- pub run()
- accept_connection()

Connection { stream, subscriptions, tx, rx }
- new()
- recv()
- send()
```