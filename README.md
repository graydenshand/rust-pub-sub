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
```