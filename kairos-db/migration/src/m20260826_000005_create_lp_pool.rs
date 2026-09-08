use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(LpPool::Table)
                    .col(ColumnDef::new(LpPool::Id).big_integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(LpPool::PoolPubkey).text().not_null().unique_key())
                    .col(ColumnDef::new(LpPool::TotalUsdc).big_integer().not_null().default(0))
                    .col(ColumnDef::new(LpPool::TotalShares).big_integer().not_null().default(0))
                    .col(ColumnDef::new(LpPool::UpdatedAt).timestamp_with_time_zone().not_null().default(Expr::current_timestamp()))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(LpEvents::Table)
                    .col(ColumnDef::new(LpEvents::Id).big_integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(LpEvents::EventType).text().not_null())
                    .col(ColumnDef::new(LpEvents::Provider).text().not_null())
                    .col(ColumnDef::new(LpEvents::UsdcAmount).big_integer().not_null())
                    .col(ColumnDef::new(LpEvents::SharesAmount).big_integer().not_null())
                    .col(ColumnDef::new(LpEvents::TxSignature).text().not_null())
                    .col(ColumnDef::new(LpEvents::Slot).big_integer().not_null())
                    .col(ColumnDef::new(LpEvents::OccurredAt).timestamp_with_time_zone().not_null().default(Expr::current_timestamp()))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .table(LpEvents::Table)
                    .name("idx_lp_events_provider")
                    .col(LpEvents::Provider)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(LpEvents::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(LpPool::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum LpPool {
    Table,
    Id,
    PoolPubkey,
    TotalUsdc,
    TotalShares,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum LpEvents {
    Table,
    Id,
    EventType,        // 'deposit' | 'withdraw'
    Provider,
    UsdcAmount,
    SharesAmount,
    TxSignature,
    Slot,
    OccurredAt,
}