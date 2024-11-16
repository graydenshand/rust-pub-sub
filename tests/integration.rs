use rmpv::Value;
use rps;
use tokio;

fn capture(
    mut client: rps::client::Client,
    n_messages: usize,
    timeout_interval_ms: u64,
) -> tokio::task::JoinHandle<Vec<rps::datagram::Message>> {
    // Spawn a task to listen for messages
    //
    // The task will receive messages until all messages are accounted for, or
    // until there have been no new messages received in the specified interval
    tokio::spawn(async move {
        // Collect received messages into a vec to compare at end of test
        let mut received_messages = vec![];

        // Loop until no messages received in specified interval
        while let Some(message) = client
            .recv(Some(tokio::time::Duration::from_millis(
                timeout_interval_ms,
            )))
            .await
        {
            // capture the message
            received_messages.push(message);

            // all messages received
            if received_messages.len() >= n_messages {
                break;
            }
        }
        // Give message receiver time to spin up
        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
        received_messages
    })
}

#[tokio::test]
async fn it_publishes_and_receives_messages() {
    // Define variables
    let port = 36912;
    let messages = vec![
        ("test", Value::from("test")),
        ("test", Value::from(0.001)),
        ("test", Value::from(true)),
    ];

    // Start server
    let mut server = rps::server::Server::new(port)
        .await
        .expect("Server can bind to address");
    tokio::spawn(async move {
        server.run().await.expect("Ok");
    });

    // Give server 500ms to start, then initialize a client
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
    let client: rps::client::Client =
        rps::client::Client::new(format!("127.0.0.1:{port}"), "test".into()).await;

    // Subscribe to all messages
    client.subscribe("*").await;

    // Spawn a task to listen for messages
    let client_clone = client.clone();
    let n_messages = messages.len();
    let timeout_interval_ms = 500;
    let read_future = capture(client, n_messages, timeout_interval_ms);

    // Publish each of the messages defined above
    let messages_clone = messages.clone();
    for (topic, value) in messages_clone {
        client_clone.publish(topic, value).await;
    }

    // Wait for spawned task to complete
    let mut received_messages = read_future.await.unwrap();

    // Verify messages received matches messages sent
    for i in 0..messages.len() {
        assert_eq!(messages[i].0, received_messages[i].topic);
        assert_eq!(messages[i].1, received_messages[i].value);
    }

    // Reinitialize client
    let client: rps::client::Client =
        rps::client::Client::new(format!("127.0.0.1:{port}"), "test".into()).await;

    // Unsubscribe
    client.unsubscribe("*").await;

    // Spawn a new task to listen for messages
    let client_clone = client.clone();
    let read_future = capture(client, n_messages, timeout_interval_ms);

    // Publish each of the messages defined above
    let messages_clone = messages.clone();
    for (topic, value) in messages_clone {
        client_clone.publish(topic, value).await;
    }

    // Verify 0 messages were received (meaning the unsubscribe most likely worked)
    received_messages = read_future.await.unwrap();
    assert_eq!(received_messages.len(), 0);
}
