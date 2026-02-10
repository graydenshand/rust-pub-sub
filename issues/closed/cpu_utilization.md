# CPU Utilization

The server currently exhibits high CPU utilization. This is most likely because each message is serialized repeatedly for every client it is forwarded to.

This is wasted effort, and a smarter architecture can make this more efficient.

## Notes

Currently the `Bus` abstracts a single broadcast channel that forwards every message to every other connection.

A more efficient architecture would be to have a channel per topic, and only forward messages to clients that are subscribed to that topic.

This does add some overhead for managing those channels:
- The first time a message is sent on a topic, the `Bus` must create a new channel
- The `Bus` needs some way of communicating to each connection when a new channel is created, so that the client
can determine if they should subscribe.
- Teardown logic, to remove channels when they are no longer needed, and to clean up any resources associated with them.

Architecture
- Add a "management" channel for broadcasting channel lifecycle updates (channel-created, channel-deleted)
- When a message is sent on a topic, the `Bus` checks if a channel exists for that topic, and creates one if not


The subscription manager will need some structural changes. Currently it uses a tokio::select to receive either a
message from the bus or a command from the command router. It also checks each message against its subscriptions.


How can we simultaneously recv from a dynamic number of channels?
- [FuturesUnordered](https://docs.rs/futures/0.3.19/futures/stream/struct.FuturesUnordered.html) ? 

> Note, the bus should forward a tuple of (client_id, bytes) where client_id is the id of the client that sent the message, and bytes is the serialized message. This will allow us to avoid sending a client's own messages back to it.

Current Flow:
-> CommandRouter
  |-> Bus
      |-> SubscriptionManager
          |-> Filtering complete message stream based on subscriptions
          |-> Serialize and send to client
          
New Flow:
-> CommandRouter
  |->Serialize and send to bus
    |-> Bus
        |-> Implicit filtering by channel routing
            |-> SubscriptionManager
              |-> Forward all bytes to client
