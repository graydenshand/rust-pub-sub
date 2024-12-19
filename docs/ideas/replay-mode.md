# Replay Mode

Log messages to a persistent storage device, and then allow on-demand replay of those messages.

Replay mode behaves more like a Kafka stream or a Redis stream, where messages are retained possibly indefinitely 
and consumers can start reading messages from an arbitrary point in the stream.

## Persistance

The goal is to support persistance without disrupting the performance of the live stream.

To achieve this, no guarantees are made that the persistent message store is complete or eventually consistent. It
is possible that a message received by the live server is not persisted.

A sidecar pattern is used, where a persistence process acts as a client, listening to the message stream from the
server. Messages are appended to a buffer that is occasionally flushed to disk.

