use rmpv::Value;
use rps;
use tokio;

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
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    let mut client = rps::client::Client::new(format!("127.0.0.1:{port}"), "test".into()).await;

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
            println!("{:?}", message);
            received_messages.push(message);
            if received_messages.len() >= n_messages {
                break;
            }
        }
        received_messages
    });

    // Publish each of the messages defined above
    let messages_clone = messages.clone();
    for (topic, value) in messages_clone {
        println!("PUBLISH - {topic} - {value}");
        client_clone.publish(topic, value).await;
    }

    // Wait for spawned task to complete
    let received_messages = read_future.await.unwrap();

    // Verify messages received matches messages sent
    println!("a");
    for i in 0..messages.len() {
        assert_eq!(messages[i].0, received_messages[i].topic);
        assert_eq!(messages[i].1, received_messages[i].value);
    }
}
