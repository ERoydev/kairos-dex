use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio_tungstenite::{connect_async, tungstenite::Message};

/// This is a simple `logSubscribe` to the helius RPC
/// it's not a reliant way after all for my DEX in prod systems i am going to use either payed service or Geyser plugin
pub async fn subscribe(rpc_ws_url: &str, program_id: &str) {
    let (ws_stream, _) = connect_async(rpc_ws_url).await.expect("Failed to connect");
    let (mut write, mut read) = ws_stream.split();

    let sub_request = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "logsSubscribe",
        "params": [
            { "mentions": [program_id] },
            { "commitment": "confirmed" }
        ]
    });

    write
        .send(Message::Text(sub_request.to_string().into()))
        .await
        .expect("Failed to send logsSubscribe request");

    while let Some(message) = read.next().await {
        let message = message.expect("Failed to read the message");
        println!("Received a message: {}", message);
    }
}
