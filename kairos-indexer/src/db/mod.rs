use sqlx::{Pool, Postgres, postgres::PgPoolOptions};

pub async fn create_pool(database_url: &str) -> Pool<Postgres> {
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(database_url)
        .await
        .expect("Failed to connect to database");

    // Run migrations
    sqlx::migrate!("./src/migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    pool
}
