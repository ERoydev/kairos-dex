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

use kairos_indexer::health::{HealthState, health_handler};
use kairos_indexer::*;

struct AppState {
    pub db_pool: DatabaseConnection,
    pub config: Config,
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    let config = Config::default();

    let db_pool = create_pool(&config.database_url).await;
    Migrator::up(&db_pool, None)
        .await
        .expect("Failed to run pending migrations");

    let state = Arc::new(AppState { db_pool, config });

    let health_state = Arc::new(HealthState::new());

    let app = Router::new()
        .route("/health", get(health_handler))
        .with_state(health_state.clone());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Listening on http://localhost:3000");

    // Both subscribers hit the same RPC URL — share one client so they share its connection
    // pool instead of each opening their own.
    let rpc_client = rpc::RpcClient::new(&state.config.rpc_http_url);

    Subscriber::new(
        &state.config.rpc_ws_url,
        state.config.program_id.to_string(),
        ProgramKind::Perp,
    )
    .with_health_state(health_state.clone())
    .with_db(state.db_pool.clone())
    .with_rpc(rpc_client.clone())
    .spawn();

    Subscriber::new(
        &state.config.rpc_ws_url,
        state.config.lp_pool_program_id.to_string(),
        ProgramKind::LpPool,
    )
    .with_health_state(health_state)
    .with_db(state.db_pool.clone())
    .with_rpc(rpc_client)
    .spawn();

    axum::serve(listener, app).await.unwrap();
}
