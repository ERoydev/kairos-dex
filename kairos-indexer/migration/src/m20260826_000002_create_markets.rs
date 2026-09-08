use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Markets::Table)
                    .col(ColumnDef::new(Markets::Id).big_integer().not_null().auto_increment().primary_key())
                    .col(ColumnDef::new(Markets::MarketPubkey).text().not_null().unique_key())
                    .col(ColumnDef::new(Markets::Symbol).text().not_null())
                    .col(ColumnDef::new(Markets::Oracle).text().not_null())
                    .col(ColumnDef::new(Markets::OiLong).big_integer().not_null().default(0))
                    .col(ColumnDef::new(Markets::OiShort).big_integer().not_null().default(0))
                    .col(ColumnDef::new(Markets::CumulativeFundingIndex).big_integer().not_null().default(0))
                    .col(ColumnDef::new(Markets::LastFundingTime).big_integer().not_null().default(0))
                    .col(ColumnDef::new(Markets::IntervalSeconds).integer().not_null().default(0))
                    .col(ColumnDef::new(Markets::IsActive).boolean().not_null().default(true))
                    .col(ColumnDef::new(Markets::UpdatedAt).timestamp_with_time_zone().not_null().default(Expr::current_timestamp()))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Markets::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum Markets {
    Table,
    Id,
    MarketPubkey,
    Symbol,
    Oracle,
    OiLong,
    OiShort,
    CumulativeFundingIndex,
    LastFundingTime,
    IntervalSeconds,
    IsActive,
    UpdatedAt,
}