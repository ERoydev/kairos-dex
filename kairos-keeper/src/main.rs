mod config;
mod funding;
mod queue;

pub use funding::*;
use std::sync::Arc;
use std::time::Duration;

use rpc::RpcClient;

use config::Config;
use tokio::sync::{Mutex, mpsc};

use crate::queue::Job;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    let (tx, rx) = mpsc::channel::<Job>(100);
    let rx = Arc::new(Mutex::new(rx));

    let config = Config::from_env();
    let rpc = RpcClient::new(config.rpc_url.clone());
    let db = kairos_db::create_pool(&config.database_url).await;

    // funding_loop — reads cached market state from Postgres (see kairos-db) instead of
    // polling RPC, and derives its own cadence from the tightest on-chain interval_seconds.
    let funding_tx = tx.clone();
    tokio::spawn(async move {
        FLoop::new(db, funding_tx, Duration::from_secs(5)).run().await;
    });

    // liquidation_loop
    // tokio::spawn(async move {
    //     loop {
    //         for position in fetch_positions().await {
    //             liq_tx.send(Job::LiquidationCheck(position)).await.ok();
    //         }
    //         tokio::time::sleep(Duration::from_secs(2)).await;
    //     }
    // });
}
