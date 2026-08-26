use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PositionEvents::Table)
                    .col(ColumnDef::new(PositionEvents::Id).big_integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(PositionEvents::EventType).text().not_null())
                    .col(ColumnDef::new(PositionEvents::PositionPubkey).text().not_null())
                    .col(ColumnDef::new(PositionEvents::Owner).text().not_null())
                    .col(ColumnDef::new(PositionEvents::Market).text().not_null())
                    .col(ColumnDef::new(PositionEvents::Side).text().not_null())
                    .col(ColumnDef::new(PositionEvents::Notional).big_integer().not_null())
                    .col(ColumnDef::new(PositionEvents::Price).big_integer().not_null())
                    .col(ColumnDef::new(PositionEvents::Pnl).big_integer().null())
                    .col(ColumnDef::new(PositionEvents::Fee).big_integer().null())
                    .col(ColumnDef::new(PositionEvents::AccruedFunding).big_integer().null())
                    .col(ColumnDef::new(PositionEvents::Liquidator).text().null())
                    .col(ColumnDef::new(PositionEvents::TxSignature).text().not_null())
                    .col(ColumnDef::new(PositionEvents::Slot).big_integer().not_null())
                    .col(ColumnDef::new(PositionEvents::OccurredAt).timestamp_with_time_zone().not_null().default(Expr::current_timestamp()))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .table(PositionEvents::Table)
                    .name("idx_position_events_owner")
                    .col(PositionEvents::Owner)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .table(PositionEvents::Table)
                    .name("idx_position_events_market")
                    .col(PositionEvents::Market)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .table(PositionEvents::Table)
                    .name("idx_position_events_type")
                    .col(PositionEvents::EventType)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(PositionEvents::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum PositionEvents {
    Table,
    Id,
    EventType,        // 'open' | 'close' | 'liquidation'
    PositionPubkey,
    Owner,
    Market,
    Side,
    Notional,
    Price,
    Pnl,              // null on open
    Fee,
    AccruedFunding,   // null on open
    Liquidator,       // set only for liquidation events
    TxSignature,
    Slot,
    OccurredAt,
}