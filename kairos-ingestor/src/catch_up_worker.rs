// On startup, fetches signatures since cursor via RPC and pushes them to the queue.

use std::str::FromStr;

use kairos_db::DatabaseConnection;

use solana_client::{
    nonblocking::rpc_client::RpcClient, rpc_client::GetConfirmedSignaturesForAddress2Config,
};
use solana_sdk::{pubkey::Pubkey, signature::Signature};

use crate::{Cursor, CursorManager, Error, QueuePublisher, Result, config};

/// StartUp Mechanism: make sure no events are missed between "where i stopped last time" and "now".
/// Cursor exists → fetch from cursor → now.
/// Cursor is empty (first run) → apply first-run policy (start from now, or backfill all).
pub struct StartUpCatchUpWorker<'a> {
    pub cursor_manager: CursorManager,
    pub rpc_client: RpcClient,
    pub queue_publisher: &'a QueuePublisher,
}

impl<'a> StartUpCatchUpWorker<'a> {
    pub fn new(db: DatabaseConnection, queue_publisher: &'a QueuePublisher) -> Self {
        let rpc = RpcClient::new(config::get().rpc_url.clone());

        StartUpCatchUpWorker {
            cursor_manager: CursorManager::new(db),
            rpc_client: rpc,
            queue_publisher,
        }
    }

    pub async fn run(&self) {
        // Runs everything
    }

    pub async fn dispatch(&self) -> Result<()> {
        let program_ids = Vec::new();
        let cursors = self.cursor_manager.load(program_ids).await;
        for (program_id, cursor) in cursors {
            let p_id = Pubkey::from_str(&program_id)?;
            match cursor {
                // cursor exists — fetch everything since the last known signature
                Cursor::Existing(model) => {
                    let until = model.last_signature.parse()?;
                    self.fetch_signatures(&p_id, Some(until)).await?;
                }
                // first run — no stop point, walk all the way back to genesis
                Cursor::Empty(_) => {
                    self.fetch_signatures(&p_id, None).await?;
                }
            }
        }
        Ok(())
    }

    async fn fetch_signatures(&self, program_id: &Pubkey, until: Option<Signature>) -> Result<()> {
        let mut before: Option<Signature> = None;

        loop {
            let sigs = self
                .rpc_client
                .get_signatures_for_address_with_config(
                    program_id,
                    GetConfirmedSignaturesForAddress2Config {
                        before,
                        until,
                        limit: Some(1000),
                        ..Default::default()
                    },
                )
                .await?;

            if sigs.is_empty() {
                break;
            }

            before = Some(
                sigs.last()
                    .ok_or(Error::EmptySignatures)?
                    .signature
                    .parse()?,
            );

            // Push those into Redis task queue
            for sig in sigs.into_iter().rev() {
                let signature = sig.signature.parse()?;
                self.queue_publisher.publish(program_id, signature).await;
            }
        }
        Ok(())
    }
}
