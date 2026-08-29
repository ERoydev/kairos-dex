use sea_orm::{DatabaseConnection, DbErr, Set};

use crate::db::entities::funding_updates;
use crate::db::queries;
use crate::parser::accounts::SynteticMarket;
use crate::parser::events::FundingUpdated;
use crate::rpc::RpcClient;

pub async fn funding_updated(
    db: &DatabaseConnection,
    rpc: Option<&RpcClient>,
    e: FundingUpdated,
    signature: &str,
    slot: u64,
) -> Result<(), DbErr> {
    let market = e.market.to_string();

    // oi_long/oi_short aren't in this event, but they're fields on the SynteticMarket account.
    let (oi_long, oi_short) =
        super::fetch_account::<SynteticMarket>(rpc, &e.market, "funding_updated")
            .await
            .map(|account| (account.oi_long as i64, account.oi_short as i64))
            .unwrap_or((0, 0));

    queries::insert_funding_update(
        db,
        funding_updates::ActiveModel {
            market: Set(market.clone()),
            funding_rate_bps: Set(e.funding_rate_bps),
            cumulative_funding_index: Set(e.cumulative_funding_index_bps),
            oi_long: Set(oi_long),
            oi_short: Set(oi_short),
            tx_signature: Set(signature.to_string()),
            slot: Set(slot as i64),
            ..Default::default()
        },
    )
    .await?;

    queries::update_market_funding(
        db,
        &market,
        e.cumulative_funding_index_bps,
        e.last_funding_time,
    )
    .await
}
