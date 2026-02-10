# Bridge

Add a `--bridge` flag to the server CLI that joins the server process to another already running one.

When in bridge mode, it will forward all messages it receives and subscribe to all messages from the target server.

This allows two servers to share the load, particularly helping to address the read fan out problem common in pub/sub.

## Notes

One challenge is avoiding message loops.
- Either the bridge-server needs to filter out a client's own messages
- Or, the bridge-client needs to filter out the messages it is forwarding

Doing this on the server side would allow for hiding the behavior from clients, and reduce load on the network.

However, it adds complexity and increases memory requirements for the server.
> Not necessarily, tracking just the client ids and using that to filter messages is more lightweight than recording individual message ids.

So, server assigns a client id to the connection and passes it along to both the CommandRouter and SubscriptionManager.

I've also been considering reducing the CPU load on the server by serializing messages once and using a channel-per-topic pattern to distribute the message contents.
It may make sense to implement that first, as it will impact the implementation of the bridge functionality.
