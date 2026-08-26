use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Positions::Table)
                    .col(ColumnDef::new(Positions::Id).big_integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(Positions::PositionPubkey).text().not_null().unique_key())
                    .col(ColumnDef::new(Positions::Owner).text().not_null())
                    .col(ColumnDef::new(Positions::Market).text().not_null())
                    .col(ColumnDef::new(Positions::Side).text().not_null())
                    .col(ColumnDef::new(Positions::Collateral).big_integer().not_null())
                    .col(ColumnDef::new(Positions::Notional).big_integer().not_null())
                    .col(ColumnDef::new(Positions::EntryPrice).big_integer().not_null())
                    .col(ColumnDef::new(Positions::EntryFundingIndex).big_integer().not_null())
                    .col(ColumnDef::new(Positions::OpenedAt).timestamp_with_time_zone().not_null().default(Expr::current_timestamp()))
                    .col(ColumnDef::new(Positions::ClosedAt).timestamp_with_time_zone().null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Positions::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum Positions {
    Table,
    Id,
    PositionPubkey,
    Owner,
    Market,
    Side,
    Collateral,
    Notional,
    EntryPrice,
    EntryFundingIndex,
    OpenedAt,
    ClosedAt,
}