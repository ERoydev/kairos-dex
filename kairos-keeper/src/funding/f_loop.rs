use std::time::Duration;

use kairos_db::{DatabaseConnection, DbErr, queries};
use tokio::sync::mpsc::Sender;

use crate::{
    now_unix,
    task::{Task, TaskType},
};

/// Scans cached market state in Postgres (populated by kairos-indexer) instead of hitting RPC,
/// and pushes a `FundingTick` for every market whose on-chain funding interval has elapsed.
pub struct FLoop {
    db: DatabaseConnection,
    tx: Sender<Task>,
    /// Poll cadence used only when there are no active markets to derive one from.
    fallback_interval: Duration,
}

impl FLoop {
    pub fn new(db: DatabaseConnection, tx: Sender<Task>, fallback_interval: Duration) -> Self {
        Self {
            db,
            tx,
            fallback_interval,
        }
    }

    pub fn get_task_type(&self) -> TaskType {
        TaskType::FundingTick
    }

    pub async fn run(&self) {
        loop {
            let sleep_for = match self.tick().await {
                Ok(next) => next,
                Err(e) => {
                    eprintln!("funding loop: db query failed: {e}");
                    self.fallback_interval
                }
            };
            tokio::time::sleep(sleep_for).await;
        }
    }

    /// Enqueues due markets and returns how long to sleep before the next scan — the tightest
    /// `interval_seconds` among active markets, so the loop stays responsive to fast markets
    /// without busy-polling slow ones.
    async fn tick(&self) -> Result<Duration, DbErr> {
        let now = now_unix();
        let markets = queries::find_active_markets(&self.db).await?;
        tracing::info!("Funding loop tick, active markets: {:#?}", markets);

        for market in &markets {
            let elapsed = now - market.last_funding_time;
            if elapsed >= market.interval_seconds as i64 {
                let task = Task::new(&market.market_pubkey, self.get_task_type());
                // If enough time passed, then send the market as a job to the queue
                self.tx.send(task).await.ok();
            }
        }

        // Take the smallest interval secs from all active markets
        // or `fallback_interval` if there are none active
        let next = markets
            .iter()
            .map(|m| m.interval_seconds.max(1) as u64)
            .min()
            .map(Duration::from_secs)
            .unwrap_or(self.fallback_interval);
        Ok(next)
    }
}
