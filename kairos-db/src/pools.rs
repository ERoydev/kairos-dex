use sea_orm::{Database, DatabaseConnection};

/// Connect to database
pub async fn create_pool(database_url: &str) -> DatabaseConnection {
    let conn = Database::connect(database_url)
        .await
        .expect("Failed to connect to database");
    tracing::info!("database connection pool established");
    conn
}
