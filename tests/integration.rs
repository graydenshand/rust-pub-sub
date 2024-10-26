use rmpv::Value;
use rps;
use tokio;

#[tokio::test]
async fn it_publishes_and_receives_messages() {
    // Define variables
    let port = 36912;
    let messages = vec![
        rps::datagram::Message::new("test", Value::from("test")),
        rps::datagram::Message::new("test", Value::from(0.001)),
        rps::datagram::Message::new("test", Value::from(true)),
    ];

    // Start server
    let mut server = rps::server::Server::new(port)
        .await
        .expect("Server can bind to address");
    tokio::spawn(async move {
        server.run().await.expect("Ok");
    });

    // Give server 500ms to start, then initialize a client
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    let mut client = rps::client::Client::new(format!("127.0.0.1:{port}")).await;

    // Subscribe to all messages
    client.subscribe("*").await;

    // Spawn a task to listen for messages
    //
    // The task will receive messages until all messages are accounted for, or
    // until there have been no new messages received in 500ms
    let client_clone = client.clone();
    let n_messages = messages.len();
    let read_future = tokio::spawn(async move {
        // Collect received messages into a vec to compare at end of test
        let mut received_messages = vec![];
        while let Some(message) = client
            .recv(Some(tokio::time::Duration::from_millis(500)))
            .await
        {
            received_messages.push(message);
            if received_messages.len() >= n_messages + 1 {
                break;
            }
        }
        received_messages
    });

    // Publish each of the messages defined above
    let messages_clone = messages.clone();
    for message in messages_clone {
        client_clone.publish(message).await;
    }

    // Wait for spawned task to complete
    let received_messages = read_future.await.unwrap();

    // Verify messages received matches messages sent
    assert_eq!(messages, received_messages);
}
