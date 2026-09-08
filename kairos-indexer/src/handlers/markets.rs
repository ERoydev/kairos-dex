use sea_orm::{DatabaseConnection, DbErr, Set};

use crate::db::entities::markets;
use crate::db::queries;
use crate::parser::accounts::SynteticMarket;
use crate::parser::events::{MarketInitialized, MarketPaused};
use rpc::RpcClient;

pub async fn market_initialized(
    db: &DatabaseConnection,
    rpc: Option<&RpcClient>,
    e: MarketInitialized,
) -> Result<(), DbErr> {
    // `oracle`/`interval_seconds` aren't in this event, but they're fields on the SynteticMarket account itself.
    let (oracle, interval_seconds) =
        super::fetch_account::<SynteticMarket>(rpc, &e.market, "market_initialized")
            .await
            .map(|account| {
                (
                    account.oracle.to_string(),
                    account.funding_config.interval_seconds as i32,
                )
            })
            .unwrap_or_default();

    queries::insert_market(
        db,
        markets::ActiveModel {
            market_pubkey: Set(e.market.to_string()),
            symbol: Set(e.symbol_str()),
            oracle: Set(oracle),
            interval_seconds: Set(interval_seconds),
            ..Default::default()
        },
    )
    .await
}

pub async fn market_paused(db: &DatabaseConnection, e: MarketPaused) -> Result<(), DbErr> {
    queries::set_market_active(db, &e.market.to_string(), e.is_active).await
}
