pub mod entities;
pub use entities::*;

use sea_orm::{Database, DatabaseConnection};

pub async fn create_pool(database_url: &str) -> DatabaseConnection {
    Database::connect(database_url)
        .await
        .expect("Failed to connect to database")
}