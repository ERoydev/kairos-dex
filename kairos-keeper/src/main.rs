mod config;
mod funding;
mod queue;

use std::sync::Arc;
pub use funding::*;

use rpc::RpcClient;

use config::Config;
use tokio::sync::{mpsc, Mutex};

use crate::queue::Job;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    let (tx, rx) = mpsc::channel::<Job>(100);
    let rx = Arc::new(Mutex::new(rx));

    // funding_loop
    // tokio::spawn(async move {
    //     loop {
    //         for market in fetch_markets().await {
    //             funding_tx.send(Job::FundingTick(market)).await.ok();
    //         }
    //         tokio::time::sleep(Duration::from_secs(5)).await;
    //     }
    // });

    // liquidation_loop
    // tokio::spawn(async move {
    //     loop {
    //         for position in fetch_positions().await {
    //             liq_tx.send(Job::LiquidationCheck(position)).await.ok();
    //         }
    //         tokio::time::sleep(Duration::from_secs(2)).await;
    //     }
    // });

    let config = Config::from_env();
    let rpc = RpcClient::new(config.rpc_url.clone());

}
