pub mod catch_up_worker;
pub mod config;
pub mod cursor_manager;
pub mod error;
pub mod queue_publisher;

pub use catch_up_worker::*;
pub use config::*;
pub use cursor_manager::*;
pub use error::*;
pub use queue_publisher::*;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    Config::from_env().init();
    let db = kairos_db::create_pool(&config::get().database_url).await;

    let queue_publisher = QueuePublisher::new();

    StartUpCatchUpWorker::new(db, &queue_publisher).run().await;

    println!("Hello, world!");
}
