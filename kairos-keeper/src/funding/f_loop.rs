use std::time::{Duration, SystemTime, UNIX_EPOCH};

use kairos_db::{DatabaseConnection, DbErr, queries};
use tokio::sync::mpsc::Sender;

use crate::queue::Job;

/// Scans cached market state in Postgres (populated by kairos-indexer) instead of hitting RPC,
/// and pushes a `FundingTick` for every market whose on-chain funding interval has elapsed.
pub struct FLoop {
    db: DatabaseConnection,
    tx: Sender<Job>,
    /// Poll cadence used only when there are no active markets to derive one from.
    fallback_interval: Duration,
}

impl FLoop {
    pub fn new(db: DatabaseConnection, tx: Sender<Job>, fallback_interval: Duration) -> Self {
        Self {
            db,
            tx,
            fallback_interval,
        }
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

        for market in &markets {
            let elapsed = now - market.last_funding_time;
            if elapsed >= market.interval_seconds as i64 {
                self.tx
                    .send(Job::FundingTick(market.market_pubkey.clone()))
                    .await
                    .ok();
            }
        }

        let next = markets
            .iter()
            .map(|m| m.interval_seconds.max(1) as u64)
            .min()
            .map(Duration::from_secs)
            .unwrap_or(self.fallback_interval);
        Ok(next)
    }
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_secs() as i64
}
