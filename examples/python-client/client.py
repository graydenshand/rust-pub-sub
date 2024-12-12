"""This example demonstrates how to subscribe to a topic and publish a message from python."""
import socket
import msgpack
import time
import threading

# Connect to the server
clientsocket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
clientsocket.connect(('localhost', 8089))

# Thread target
def listen(socket):
    # Subscribe to a topic
    clientsocket.send(msgpack.packb({"Subscribe": ["my-topic"]}))

    # Parse data from socket
    unpacker = msgpack.Unpacker()
    message_count = 0
    while True:
        # Fill buffer
        buf = socket.recv(1024**2)
        if not buf:
            break
        unpacker.feed(buf)
        for message in unpacker:
            # Print the message
            print(message)
            # Exit after receiving two messages
            message_count += 1
            if message_count > 1:
                return


# Listen in the background
t = threading.Thread(target=listen, args=(clientsocket,))
t.start()

# Give thread a moment to start
time.sleep(0.1)
# Send a message
clientsocket.send(msgpack.packb({"Publish": [["my-topic", True]]}))
clientsocket.send(msgpack.packb({"Publish": [["my-topic", {"Foo": 0.5}]]}))
# Wait for listen thread to exit
t.join()
