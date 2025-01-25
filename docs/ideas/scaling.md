# Scaling MessagesSent

MessagesSent throughput can increase independently of Command throughput, if the fanout factor increases.

The fanout factor is the number of clients that are expected to receive a message in a given pubsub network.

E.g. if a topic has no clients subscribed to it, messages published to that topic are not sent. Whereas if a topic
has one thousand subscribers, it is sent one thousand times.

With a single server, fanout can quickly cause the server's outgoing network bandwidth to be exhausted.

One approach to supporting a higher number of messages sent is by scaling horizontally, allowing multiple servers
to share the load of publishing all of those messages. I.e. given two servers, route half  of incoming connections to one
and half to the other. The servers each accept incoming messages, and forward it to the others -- each effectively
acting as a client of the others.

There is an upper bound to this approach, because at some point simply the number of different servers will exceed
the fanout factor a single machine can handle.

I see two paths to address _that_ issue:
1) Use a write ahead log -- avoids the problem of fanout entirely at the cost of performance
2) Intelligent server network -- instead of a single layer of a network structure where every server is connected to the others, use a more sparse structure and intelligent message forwarding to broadcast messages
    - E.g. with 3 servers A,B,C don't send from A->B and A->C, instead send A->B, B->C
    - This could theoretically allow the network to propagate a message to an unlimited number of clients, though with deteriorating performance.
    - Goal would be to minimize the distance between any given two nodes, i.e. keep the network dense & fully connected, without cauasing any one node
        to become overutilized.
    

# Scaling Commands

Independent of the number of clients, there is an upper bound on the rate at which a single server can process commands.

The only way to scale the number of commands that can be processed is to use share-nothing shards. Share-nothing shards
each are responsible for a subset of a servers topics.

E.g. two server nodes split the set of topics in half. Clients publish to and receive from both servers.

