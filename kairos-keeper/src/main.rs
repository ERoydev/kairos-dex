mod config;
mod funding;
mod liquidation;
mod queue;
mod task;
mod utils;

pub use funding::*;
pub use liquidation::*;
pub use task::*;
pub use utils::*;

use std::time::Duration;

use config::Config;
#[cfg(feature = "dev-db")]
use migration::MigratorTrait;
use tokio::sync::mpsc;

use crate::queue::TaskQueue;

// Tokio creates a pool of OS worker threads
// Then tokio::spawn() creates a tokio task, which tokio schedules
// onto one of those existing worker threads

// Tokio
// ├── OS Thread 1
// │   ├── Task A
// │   └── Task C
// ├── OS Thread 2
// │   └── Task B
// └── OS Thread 3
//     └── Task D
#[tokio::main(flavor = "multi_thread")]
async fn main() {
    dotenv::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let (tx, rx) = mpsc::channel::<Task>(100);

    Config::from_env().init();
    let db = kairos_db::create_pool(&config::get().database_url).await;
    let rpc_client = rpc::RpcClient::new(config::get().rpc_url.clone());
    // Dev-only (see `dev-db` feature): auto-creates/updates the schema, so a local sqlite
    // DATABASE_URL just works — delete dev.db and rerun to reset.
    #[cfg(feature = "dev-db")]
    migration::Migrator::up(&db, None)
        .await
        .expect("failed to run migrations");

    // funding_loop — reads cached market state from Postgres (see kairos-db) instead of
    // polling RPC, and derives its own cadence from the tightest on-chain interval_seconds.
    let funding_tx = tx.clone();
    let funding_handle = tokio::spawn(async move {
        FLoop::new(db, funding_tx, Duration::from_secs(5))
            .run()
            .await;
    });

    let queue_handle = tokio::spawn(async move {
        let task_queue = TaskQueue::new(rx);
        task_queue.run().await;
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

    funding_handle.await.ok();
    queue_handle.await.ok();
}
