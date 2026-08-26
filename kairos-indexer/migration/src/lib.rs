pub use sea_orm_migration::prelude::*;

mod m20260826_000001_create_positions;
mod m20260826_000002_create_markets;
mod m20260826_000003_create_position_events;
mod m20260826_000004_create_funding_updates;
mod m20260826_000005_create_lp_pool;

pub struct Migrator;

// Migration Register
#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260826_000001_create_positions::Migration),
            Box::new(m20260826_000002_create_markets::Migration),
            Box::new(m20260826_000003_create_position_events::Migration),
            Box::new(m20260826_000004_create_funding_updates::Migration),
            Box::new(m20260826_000005_create_lp_pool::Migration)
        ]
        
    }
}
