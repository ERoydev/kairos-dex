pub mod catch_up_worker;
pub mod config;
pub mod cursor_manager;
pub mod error;
pub mod queue_publisher;
pub mod stream;

pub use catch_up_worker::*;
pub use config::*;
pub use cursor_manager::*;
pub use error::*;
pub use queue_publisher::*;
use solana_client::nonblocking::rpc_client::RpcClient;
pub use stream::*;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    Config::from_env().init();
    let db = kairos_db::create_pool(&config::get().database_url).await;

    let rpc_client = RpcClient::new(config::get().rpc_http_url.clone());

    let program_ids = config::load_devnet_program_ids();
    let queue_publisher = QueuePublisher::new(program_ids);

    // TODO maybe is good idea to implement retry mechanism if this fails
    let _ = StartUpCatchUpWorker::new(db, &queue_publisher, &rpc_client)
        .run()
        .await;

    println!("Hello, world!");
}
