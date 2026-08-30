use chrono::Utc;
use sea_orm::{DatabaseConnection, DbErr, Set};

use crate::db::entities::{position_events, positions};
use crate::db::queries;
use crate::parser::accounts::Position as PositionAccount;
use crate::parser::events::{PositionClosed, PositionLiquidated, PositionOpened};
use crate::rpc::RpcClient;

pub async fn position_opened(
    db: &DatabaseConnection,
    rpc: Option<&RpcClient>,
    e: PositionOpened,
    signature: &str,
    slot: u64,
) -> Result<(), DbErr> {
    let position_pubkey = e.position.to_string();
    let owner = e.trader.to_string();
    let market = e.market.to_string();
    let entry_price = e.oracle_price as i64;

    // side/collateral/notional aren't in this event — they live on the Position account
    // itself (set by the open_position instruction from its args). Fetch and decode it;
    // fall back to a placeholder if that fails for any reason (see `fetch_account`).
    // TODO: Maybe is good idea to add these in the event, so i can skip fetching
    let (side, collateral, notional) =
        match super::fetch_account::<PositionAccount>(rpc, &e.position, "position_opened").await {
            Some(account) => (
                account.side.as_str().to_string(),
                account.collateral as i64,
                account.notional as i64,
            ),
            None => ("unknown".to_string(), 0, 0),
        };

    queries::upsert_position(
        db,
        &position_pubkey,
        positions::ActiveModel {
            position_pubkey: Set(position_pubkey.clone()),
            owner: Set(owner.clone()),
            market: Set(market.clone()),
            side: Set(side.clone()),
            collateral: Set(collateral),
            notional: Set(notional),
            entry_price: Set(entry_price),
            entry_funding_index: Set(e.entry_funding_index_bps),
            // Explicit rather than relying on the column default — the position could be
            // *reopening* at a pubkey that already has an old (closed) row, in which case a
            // default that only applies on INSERT wouldn't touch this on the UPDATE path.
            opened_at: Set(Utc::now().into()),
            closed_at: Set(None),
            ..Default::default()
        },
    )
    .await?;

    queries::insert_position_event(
        db,
        position_events::ActiveModel {
            event_type: Set("open".to_string()),
            position_pubkey: Set(position_pubkey),
            owner: Set(owner),
            market: Set(market),
            side: Set(side),
            notional: Set(notional),
            price: Set(entry_price),
            pnl: Set(None),
            fee: Set(None),
            accrued_funding: Set(None),
            liquidator: Set(None),
            tx_signature: Set(signature.to_string()),
            slot: Set(slot as i64),
            ..Default::default()
        },
    )
    .await
}

pub async fn position_closed(
    db: &DatabaseConnection,
    e: PositionClosed,
    signature: &str,
    slot: u64,
) -> Result<(), DbErr> {
    let position_pubkey = e.position.to_string();

    // Pull side/owner/notional from our own `positions` row rather than guessing — it's the
    // one place that data exists once a position has gone through `position_opened` above.
    // Reuse the fetched row for `close_position` too instead of re-querying it.
    let existing = queries::find_position(db, &position_pubkey).await?;
    let (owner, market, side, notional) = match &existing {
        Some(p) => (
            p.owner.clone(),
            p.market.clone(),
            p.side.clone(),
            p.notional,
        ),
        None => (
            e.trader.to_string(),
            e.market.to_string(),
            "unknown".to_string(),
            0,
        ),
    };

    if let Some(existing) = existing {
        queries::close_position(db, existing).await?;
    }

    queries::insert_position_event(
        db,
        position_events::ActiveModel {
            event_type: Set("close".to_string()),
            position_pubkey: Set(position_pubkey),
            owner: Set(owner),
            market: Set(market),
            side: Set(side),
            notional: Set(notional),
            price: Set(e.exit_price as i64),
            pnl: Set(Some(e.pnl)),
            fee: Set(None),
            accrued_funding: Set(None),
            liquidator: Set(None),
            tx_signature: Set(signature.to_string()),
            slot: Set(slot as i64),
            ..Default::default()
        },
    )
    .await
}

pub async fn position_liquidated(
    db: &DatabaseConnection,
    e: PositionLiquidated,
    signature: &str,
    slot: u64,
) -> Result<(), DbErr> {
    let position_pubkey = e.position.to_string();

    let existing = queries::find_position(db, &position_pubkey).await?;
    let (owner, market, side, notional) = match &existing {
        Some(p) => (
            p.owner.clone(),
            p.market.clone(),
            p.side.clone(),
            p.notional,
        ),
        None => (
            e.trader.to_string(),
            e.market.to_string(),
            "unknown".to_string(),
            0,
        ),
    };

    if let Some(existing) = existing {
        queries::close_position(db, existing).await?;
    }

    queries::insert_position_event(
        db,
        position_events::ActiveModel {
            event_type: Set("liquidation".to_string()),
            position_pubkey: Set(position_pubkey),
            owner: Set(owner),
            market: Set(market),
            side: Set(side),
            notional: Set(notional),
            price: Set(e.exit_price as i64),
            pnl: Set(Some(e.pnl)),
            fee: Set(None),
            accrued_funding: Set(None),
            liquidator: Set(Some(e.liquidator.to_string())),
            tx_signature: Set(signature.to_string()),
            slot: Set(slot as i64),
            ..Default::default()
        },
    )
    .await
}
