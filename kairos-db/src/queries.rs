use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, Set,
};

use crate::entities::{funding_updates, lp_events, lp_pool, markets, position_events, positions};

pub async fn find_position(
    db: &DatabaseConnection,
    position_pubkey: &str,
) -> Result<Option<positions::Model>, DbErr> {
    positions::Entity::find()
        .filter(positions::Column::PositionPubkey.eq(position_pubkey))
        .one(db)
        .await
}

/// Inserts a fresh row, or resets an existing one back to "open" if this pubkey was already
/// tracked — the program closes a position's account on `close_position`, so the same PDA can
/// legitimately be reused by a later, unrelated `open_position`. A blind INSERT would collide
/// with `position_pubkey`'s unique constraint the second time that address gets reopened.
pub async fn upsert_position(
    db: &DatabaseConnection,
    position_pubkey: &str,
    mut model: positions::ActiveModel,
) -> Result<(), DbErr> {
    match find_position(db, position_pubkey).await? {
        Some(existing) => {
            model.id = Set(existing.id);
            model.update(db).await?;
            println!("DB: updated positions row for {position_pubkey}");
        }
        None => {
            model.insert(db).await?;
            println!("DB: inserted positions row for {position_pubkey}");
        }
    }
    Ok(())
}

/// Marks an already-fetched position closed. Takes the row itself (rather than a pubkey and
/// re-querying) since callers need the pre-close row anyway to build the `position_events` entry.
pub async fn close_position(
    db: &DatabaseConnection,
    existing: positions::Model,
) -> Result<(), DbErr> {
    let position_pubkey = existing.position_pubkey.clone();
    let mut model: positions::ActiveModel = existing.into();
    model.closed_at = Set(Some(Utc::now().into()));
    model.update(db).await?;
    println!("DB: closed positions row for {position_pubkey}");
    Ok(())
}

pub async fn insert_position_event(
    db: &DatabaseConnection,
    model: position_events::ActiveModel,
) -> Result<(), DbErr> {
    let event_type = model.event_type.as_ref().clone();
    let position_pubkey = model.position_pubkey.as_ref().clone();
    model.insert(db).await?;
    println!("DB: inserted position_events row ({event_type}) for {position_pubkey}");
    Ok(())
}

pub async fn insert_funding_update(
    db: &DatabaseConnection,
    model: funding_updates::ActiveModel,
) -> Result<(), DbErr> {
    let market = model.market.as_ref().clone();
    model.insert(db).await?;
    println!("DB: inserted funding_updates row for market {market}");
    Ok(())
}

pub async fn insert_market(
    db: &DatabaseConnection,
    model: markets::ActiveModel,
) -> Result<(), DbErr> {
    let market_pubkey = model.market_pubkey.as_ref().clone();
    model.insert(db).await?;
    println!("DB: inserted markets row for {market_pubkey}");
    Ok(())
}

async fn find_market(
    db: &DatabaseConnection,
    market_pubkey: &str,
) -> Result<Option<markets::Model>, DbErr> {
    markets::Entity::find()
        .filter(markets::Column::MarketPubkey.eq(market_pubkey))
        .one(db)
        .await
}

/// All markets the keeper should consider for funding/liquidation loops — lets callers read
/// cached on-chain state (oi, last_funding_time, interval_seconds) without an RPC round trip.
pub async fn find_active_markets(db: &DatabaseConnection) -> Result<Vec<markets::Model>, DbErr> {
    markets::Entity::find()
        .filter(markets::Column::IsActive.eq(true))
        .all(db)
        .await
}

/// No-op if the market isn't tracked yet (its `MarketInitialized` event predates this indexer).
pub async fn set_market_active(
    db: &DatabaseConnection,
    market_pubkey: &str,
    is_active: bool,
) -> Result<(), DbErr> {
    if let Some(existing) = find_market(db, market_pubkey).await? {
        let mut model: markets::ActiveModel = existing.into();
        model.is_active = Set(is_active);
        model.updated_at = Set(Utc::now().into());
        model.update(db).await?;
        println!("DB: updated markets row for {market_pubkey} (is_active={is_active})");
    }
    Ok(())
}

/// No-op if the market isn't tracked yet (its `MarketInitialized` event predates this indexer).
pub async fn update_market_funding(
    db: &DatabaseConnection,
    market_pubkey: &str,
    cumulative_funding_index: i64,
    last_funding_time: i64,
) -> Result<(), DbErr> {
    if let Some(existing) = find_market(db, market_pubkey).await? {
        let mut model: markets::ActiveModel = existing.into();
        model.cumulative_funding_index = Set(cumulative_funding_index);
        model.last_funding_time = Set(last_funding_time);
        model.updated_at = Set(Utc::now().into());
        model.update(db).await?;
        println!("DB: updated markets row for {market_pubkey} (funding)");
    }
    Ok(())
}

pub async fn insert_lp_event(
    db: &DatabaseConnection,
    model: lp_events::ActiveModel,
) -> Result<(), DbErr> {
    let event_type = model.event_type.as_ref().clone();
    let provider = model.provider.as_ref().clone();
    model.insert(db).await?;
    println!("DB: inserted lp_events row ({event_type}) for {provider}");
    Ok(())
}

async fn find_lp_pool(
    db: &DatabaseConnection,
    pool_pubkey: &str,
) -> Result<Option<lp_pool::Model>, DbErr> {
    lp_pool::Entity::find()
        .filter(lp_pool::Column::PoolPubkey.eq(pool_pubkey))
        .one(db)
        .await
}

/// Inserts the pool row on first sight, otherwise overwrites its totals — always called with
/// freshly-fetched on-chain `Pool` account data, so last-write-wins is correct here.
pub async fn upsert_lp_pool(
    db: &DatabaseConnection,
    pool_pubkey: &str,
    total_usdc: i64,
    total_shares: i64,
) -> Result<(), DbErr> {
    match find_lp_pool(db, pool_pubkey).await? {
        Some(existing) => {
            let mut model: lp_pool::ActiveModel = existing.into();
            model.total_usdc = Set(total_usdc);
            model.total_shares = Set(total_shares);
            model.updated_at = Set(Utc::now().into());
            model.update(db).await?;
            println!("DB: updated lp_pool row for {pool_pubkey}");
        }
        None => {
            lp_pool::ActiveModel {
                pool_pubkey: Set(pool_pubkey.to_string()),
                total_usdc: Set(total_usdc),
                total_shares: Set(total_shares),
                ..Default::default()
            }
            .insert(db)
            .await?;
            println!("DB: inserted lp_pool row for {pool_pubkey}");
        }
    }
    Ok(())
}
