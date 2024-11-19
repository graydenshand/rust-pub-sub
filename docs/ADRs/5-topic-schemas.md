# 5. Message Schemas

Status: draft

## Overvieww

Using msgpack allows a high degree of flexibility because as a self describing data format, clients and servers don't need to agree ahead of time on what data a message will contain.

While more performant than JSON strings, this flexibile message structure likely causes a performance hit when compared with a more rigidly defined alternative.

Introducing a rigid data packing format could be one way to improve message throughput by more efficiently packing bytes and using network bandwidth.

## Implementation

Prior art includes protobufs, CAN dbc.
