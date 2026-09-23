// Tiny axum server, one route POST /webhook

// Loads config
// Creates the Postgres pool
// Runs pending migrations
// Starts the stream subscriber loop
// Wires: subscriber -> parser -> dispatcher -> db
// Top-level error handling / restart

use std::sync::Arc;

use axum::{Router, routing::get};
use migration::{Migrator, MigratorTrait};
use sea_orm::DatabaseConnection;

use kairos_indexer::*;
use solana_client::nonblocking::rpc_client::RpcClient;

struct AppState {
    pub db_pool: DatabaseConnection,
    pub config: Config,
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config = Config::default();

    let db_pool = create_pool(&config.database_url).await;
    Migrator::up(&db_pool, None)
        .await
        .expect("Failed to run pending migrations");

    let state = Arc::new(AppState { db_pool, config });

    let app = Router::new();
    // .route("/health", get(health_handler))
    // .with_state(health_state.clone());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    tracing::info!("=====> Listening on port: 0.0.0.0:3000");

    let rpc_client = RpcClient::new(state.config.rpc_http_url.clone()); // TODO connection to redis client instead

    axum::serve(listener, app).await.unwrap();
}
