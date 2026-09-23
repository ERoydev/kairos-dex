pub mod catch_up_worker;
pub mod config;
pub mod cursor_manager;

pub use catch_up_worker::*;
pub use config::*;
pub use cursor_manager::*;

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    Config::from_env().init();
    let db = kairos_db::create_pool(&config::get().database_url).await;

    let start_up_catch_up_worker: StartUpCatchUpWorker = StartUpCatchUpWorker::new(db);

    println!("Hello, world!");
}
