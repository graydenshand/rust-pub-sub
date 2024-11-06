# 1. Broadcast channels

Status: Accepted

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

### EDIT 1: Broadcast channel per topic

Under this scheme, instead of a global broadcast channel that distributes every message to every connection, each topic could have an associated channel for delivering messages only to the connections of subscribers.

#### Pros

The principal benefit of this approach is that each connection does not need to check every message to see if it matches a subscription. The check is a function call that runs in linear time with respect to the length of the topic name.

Some napkin math:
    Each call to `GlobTree.check` takes on avg 2.2 microseconds (benchmarked against a 100 character string).
    At 1000 clients, this call costs 2.2 milliseconds per message.

Switching to channel-per-topic shifts some of that work out of the innermost nested code of the application. The per-connection invocation of `check` is replace with a single hashmap lookup to find the channel to broadcast to.

#### Cons

The tradeoff involves added complexity on the server to manage these channels. The first time a message is published to a topic, a new channel must be created for that topic, and every client must check if it should receive messages from this topic. Additionally, when a client subscribes to a new pattern, all topics known to the server must be scanned to see which channels to subscribe to.

This would also require reintroducing a mpsc channel for publishing messages. Each connection would emit the message back to the server task via a mpsc. The server then looks up the broadcast channel for the messages topic (creating a new one if needed), and forwards the message over the broadcast channel to any subscribed connections.

A robust implementation would also bound the number of topics supported by the server, as well as provide a means of pruning channels which are no longer needed. One approach to this would be for a background task to prune channels on an interval, as determined by the time since the last message in that channel.

At a minimum this proposal requires the Server to keep a collection of active Topics. A specific Topic can be retrieved by name. Each Topic stores the sender and receiver for that channel. The server also needs to store a sender and receiver for a broadcast channel, to enable it to broadcast system changes to connections (such as the creation of a new channel). Every connection should have a reference to the server in order to subscribe to new channels.