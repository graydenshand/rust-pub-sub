# Metrics

How to best monitor the performance of the server and client?

- How many messages have been processed?
- What are the 10 topics with highest message volume?
- What are the 10 topics with highest subscriber count?
- Resource utilization -- cpu, memory, IO
- Connections
- Latency - how long does it take to receive a message you send

# Brainstorm

## Global object shared via mutex

Not really a good idea because of performance

## Global object w/ channel

Async queuing

Choice 
a) One Stats object with attributes for each metric
b) Each metric is its own object

Probably option b, keeping the ownership of each metric granular gives more flexibility in their use.
