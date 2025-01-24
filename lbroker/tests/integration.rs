use std::fmt::format;

use lbroker;
use rmpv::Value;
use tokio;

fn capture(
    mut client: lbroker::client::Client,
    n_messages: usize,
) -> tokio::task::JoinHandle<Vec<lbroker::interface::Message>> {
    // Spawn a task to listen for messages
    //
    // The task will receive messages until all messages are accounted for, or
    // until there have been no new messages received in the specified interval
    tokio::spawn(async move {
        // Collect received messages into a vec to compare at end of test
        let mut received_messages = vec![];

        // Loop until n messages received
        while let Some(message) = client.recv().await {
            // capture the message
            received_messages.push(message);

            // all messages received
            if received_messages.len() >= n_messages {
                client.publish("test-complete", Value::Boolean(true)).await;
                break;
            }
        }

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
    let mut server = lbroker::server::Server::new(port)
        .await
        .expect("Server can bind to address");
    tokio::spawn(async move {
        server.run().await.expect("Ok");
    });

    // Give server 500ms to start, then initialize a client
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    let client1 = lbroker::client::Client::new(format!("127.0.0.1:{port}")).await;

    // Subscribe to all messages
    client1.subscribe("*").await;

    // Spawn a task to listen for messages
    let mut client2 = lbroker::client::Client::new(format!("127.0.0.1:{port}")).await;
    let n_messages = messages.len();
    let read_future = capture(client1, n_messages);

    // Publish each of the messages defined above
    let mut i = 0;
    client2.subscribe("test-complete").await;
    let mut finished = false;
    while !finished {
        client2
            .publish(
                messages[i % n_messages].0,
                messages[i % n_messages].1.clone(),
            )
            .await;
        tokio::time::timeout(tokio::time::Duration::from_millis(10), client2.recv())
            .await
            .inspect(|o| {
                o.as_ref().inspect(|m| finished = true);
            })
            .ok();
        i += 1;
    }

    // Wait for spawned task to complete
    let mut received_messages = read_future.await.unwrap();

    // Verify messages received matches messages sent
    assert_eq!(messages.len(), received_messages.len());
}
