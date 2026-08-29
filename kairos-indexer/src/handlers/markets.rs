use sea_orm::{DatabaseConnection, DbErr, Set};

use crate::db::entities::markets;
use crate::db::queries;
use crate::parser::accounts::SynteticMarket;
use crate::parser::events::{MarketInitialized, MarketPaused};
use crate::rpc::RpcClient;

pub async fn market_initialized(
    db: &DatabaseConnection,
    rpc: Option<&RpcClient>,
    e: MarketInitialized,
) -> Result<(), DbErr> {
    // `oracle` isn't in this event, but it's a field on the SynteticMarket account itself.
    let oracle = super::fetch_account::<SynteticMarket>(rpc, &e.market, "market_initialized")
        .await
        .map(|account| account.oracle.to_string())
        .unwrap_or_default();

    queries::insert_market(
        db,
        markets::ActiveModel {
            market_pubkey: Set(e.market.to_string()),
            symbol: Set(e.symbol_str()),
            oracle: Set(oracle),
            ..Default::default()
        },
    )
    .await
}

pub async fn market_paused(db: &DatabaseConnection, e: MarketPaused) -> Result<(), DbErr> {
    queries::set_market_active(db, &e.market.to_string(), e.is_active).await
}
