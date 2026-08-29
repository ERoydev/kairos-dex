// Tiny axum server, one route POST /webhook

// Loads config
// Creates the Postgres pool
// Runs pending migrations
// Starts the stream subscriber loop
// Wires: subscriber -> parser -> dispatcher -> db
// Top-level error handling / restart

use std::sync::Arc;

use axum::{Router, routing::get};
use sea_orm::DatabaseConnection;

pub mod config;
pub mod db;
pub mod health;
pub mod parser;
pub mod stream;

pub use config::*;
pub use db::*;
use health::{HealthState, health_handler};
pub use parser::*;
pub use stream::*;

struct AppState {
    pub db_pool: DatabaseConnection,
    pub config: Config,
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();

    let config = Config::default();

    let state = Arc::new(AppState {
        db_pool: create_pool(&config.database_url).await,
        config,
    });

    let health_state = Arc::new(HealthState::new());

    let app = Router::new()
        .route("/health", get(health_handler))
        .with_state(health_state.clone());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Listening on http://localhost:3000");

    Subscriber::new(&state.config.rpc_ws_url, state.config.program_id.to_string())
        .with_health_state(health_state)
        .spawn();

    axum::serve(listener, app).await.unwrap();
}
