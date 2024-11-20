# 2. UDP

Status: Draft

## Summary

Using a UDP stream instead of a TCP stream could improve performance.

UDP has fewer protections in place which reduces the overhead of communicating, so theoretically a 1:1 connection over UDP should still be faster.

Allowing the server to support both protocols allows users to decide what makes sense for their application. 

UDP supports multicast which reduces the IO cost of sending a message to a group of N clients from O(N) to O(1).
- "High throughput topics" -- Special topics with a dedicated UDP multicast address.

## Implementation

The server must bind a UDP Socket to an unoccupied port - i.e. a different port from the tcp listener.

With UDP, there is no notification when the client disconnects.