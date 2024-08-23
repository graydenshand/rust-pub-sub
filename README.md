# Project

## Server

1. Listen for data from a socket
2. Transform the data using multiple parallel threads
3. Publish over a socket

## Architecture
```
SERVER (listner) <--> CLIENT (caller)

BIDIRECTIONAL PUB/SUB
- Client can publish messages to server
- Server can send messages to client based on subscriptions


Server
Client
MessageReader: Owns read side of a TCP connection
MessageWriter: Owns write side of a TCP connection

ApplicataionClient
ApplicationFunction


Publish: Client --> ClientMessageWriter --> ServerMessageReader --> Server
Message Received: Server --> ServerMessageWriter --> ClientMessageReader --> Client

Client --(channel)--> ClientMessageWriter
Client <--(channel)-- ClientMessageReader
Server <--(channel)-- ServerMessageReader
Server --(channel)--> ServerMessageWriter
```



Client should be able to:
- Publish a message to a server
- Subscribe to messages on a server matching a topic pattern, registering a function to execute every time a message matching that pattern is published.
- Publish a message to a server from a subscription handler

- Client may be publishing to the same, or different servers than it's subscribed to

