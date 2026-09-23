use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(IngestorCursor::Table)
                    .col(ColumnDef::new(IngestorCursor::ProgramId).text().not_null().primary_key())
                    .col(ColumnDef::new(IngestorCursor::LastSignature).text().not_null())
                    .col(ColumnDef::new(IngestorCursor::LastSlot).big_integer().not_null())
                    .col(ColumnDef::new(IngestorCursor::UpdatedAt).timestamp_with_time_zone().not_null().default(Expr::current_timestamp()))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(IngestorCursor::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum IngestorCursor {
    Table,
    ProgramId,
    LastSignature,
    LastSlot,
    UpdatedAt,
}
