use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio::time::{interval, sleep};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::health::HealthState;
use crate::parser::parse_message;

const PING_INTERVAL: Duration = Duration::from_secs(30);
const INITIAL_BACKOFF: Duration = Duration::from_secs(1);
const MAX_BACKOFF: Duration = Duration::from_secs(30);

/// Subscribes to a Solana program's logs via a `logsSubscribe` websocket against the Helius RPC.
/// Not a reliant way for prod after all — in prod this DEX will use either a paid service or a Geyser plugin.
pub struct Subscriber {
    rpc_ws_url: String,
    program_id: String,
    health_state: Option<Arc<HealthState>>,
}

impl Subscriber {
    pub fn new(rpc_ws_url: impl Into<String>, program_id: impl Into<String>) -> Self {
        Self {
            rpc_ws_url: rpc_ws_url.into(),
            program_id: program_id.into(),
            health_state: None,
        }
    }

    pub fn with_health_state(mut self, health_state: Arc<HealthState>) -> Self {
        self.health_state = Some(health_state);
        self
    }

    /// Spawns `run` on its own task and returns the handle.
    pub fn spawn(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move { self.run().await })
    }

    /// Runs the subscribe loop forever, reconnecting with exponential backoff whenever the
    /// connection drops (e.g. Helius closing idle sockets).
    pub async fn run(&self) {
        let mut backoff = INITIAL_BACKOFF;

        loop {
            match self.connect_and_listen().await {
                Ok(()) => println!("WS connection closed, reconnecting..."),
                Err(e) => eprintln!("WS error: {e}, reconnecting in {backoff:?}"),
            }

            if let Some(health_state) = &self.health_state {
                health_state.mark_disconnected();
            }

            sleep(backoff).await;
            backoff = (backoff * 2).min(MAX_BACKOFF);
        }
    }

    async fn connect_and_listen(&self) -> Result<(), tokio_tungstenite::tungstenite::Error> {
        let (ws_stream, _) = connect_async(&self.rpc_ws_url).await?;
        let (mut write, mut read) = ws_stream.split();

        let sub_request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "logsSubscribe",
            "params": [
                { "mentions": [self.program_id] },
                { "commitment": "confirmed" }
            ]
        });

        write
            .send(Message::Text(sub_request.to_string().into()))
            .await?;

        if let Some(health_state) = &self.health_state {
            health_state.mark_connected();
        }

        // Connection is fresh, so no need to ping immediately.
        let mut ping_interval = interval(PING_INTERVAL);
        ping_interval.tick().await;

        loop {
            tokio::select! {
                message = read.next() => {
                    let Some(message) = message else {
                        return Ok(());
                    };

                    match message? {
                        Message::Text(text) => {
                            for event in parse_message(&text) {
                                println!("Decoded event: {event:?}");
                                if let Some(health_state) = &self.health_state {
                                    health_state.mark_event_processed();
                                }
                            }
                        }
                        Message::Ping(payload) => write.send(Message::Pong(payload)).await?,
                        Message::Close(frame) => {
                            println!("Server closed the connection: {frame:?}");
                            return Ok(());
                        }
                        _ => {}
                    }
                }
                _ = ping_interval.tick() => {
                    write.send(Message::Ping(Vec::new().into())).await?;
                }
            }
        }
    }
}

// Example of message
/*
{
  "jsonrpc": "2.0",
  "method": "logsNotification",
  "params": {
    "subscription": 3618558,
    "result": {
      "context": {
        "slot": 489835284
      },
      "value": {
        "signature": "teB3nY2KBnKAPTcJhRdxrvcHQ7k2JAJdzbU9hUp9BqL2MGiaNzAGw9NbE6TZY7zJq2FCf8KeVT7hMWox9RZCiv9",
        "err": null,
        "logs": [
          "Program FWmruxC6TfBGZyXbQtzNjjJVrYMzRWLsTr6iscs9bkyK invoke [1]",
          "Program log: Instruction: UpdateGlobal",
          "Program data: g0Lt0bBXd8uVQuWKJAw/5tcK9Tj4A+ArMZKoMKRtuDmardb1XUwvBAAAARQA",
          "Program FWmruxC6TfBGZyXbQtzNjjJVrYMzRWLsTr6iscs9bkyK consumed 4311 of 200000 compute units",
          "Program FWmruxC6TfBGZyXbQtzNjjJVrYMzRWLsTr6iscs9bkyK success"
        ]
      }
    }
  }
}
*/
