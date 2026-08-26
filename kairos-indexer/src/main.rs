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

pub use db::*;
pub use config::*;
use health::{HealthState, health_handler};

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
        config
    });

    // The subscriber loop (ws -> decoder -> dispatch -> handler -> DB) will call
    // health_state.mark_connected()/mark_disconnected()/mark_event_processed() once built.
    let health_state = Arc::new(HealthState::new());

    let app = Router::new()
        .route("/health", get(health_handler))
        .with_state(health_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Listening on http://localhost:3000");
    axum::serve(listener, app).await.unwrap();
}

// async fn list_users(State(state): State<Arc<AppState>>) -> Json<Vec<User>> {
//     let users = sqlx::query_as!(User, "SELECT * FROM users")
//         .fetch_all(&state.db_pool)
//         .await
//         .unwrap();
//     Json(users)
// }