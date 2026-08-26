use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(FundingUpdates::Table)
                    .col(ColumnDef::new(FundingUpdates::Id).big_integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(FundingUpdates::Market).text().not_null())
                    .col(ColumnDef::new(FundingUpdates::FundingRateBps).big_integer().not_null())
                    .col(ColumnDef::new(FundingUpdates::CumulativeFundingIndex).big_integer().not_null())
                    .col(ColumnDef::new(FundingUpdates::OiLong).big_integer().not_null())
                    .col(ColumnDef::new(FundingUpdates::OiShort).big_integer().not_null())
                    .col(ColumnDef::new(FundingUpdates::TxSignature).text().not_null())
                    .col(ColumnDef::new(FundingUpdates::Slot).big_integer().not_null())
                    .col(ColumnDef::new(FundingUpdates::TickedAt).timestamp_with_time_zone().not_null().default(Expr::current_timestamp()))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .table(FundingUpdates::Table)
                    .name("idx_funding_updates_market_time")
                    .col(FundingUpdates::Market)
                    .col(FundingUpdates::TickedAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(FundingUpdates::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum FundingUpdates {
    Table,
    Id,
    Market,
    FundingRateBps,
    CumulativeFundingIndex,
    OiLong,
    OiShort,
    TxSignature,
    Slot,
    TickedAt,
}