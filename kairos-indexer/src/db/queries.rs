use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, Set,
};

use crate::db::entities::{
    funding_updates, lp_events, lp_pool, markets, position_events, positions,
};

pub async fn find_position(
    db: &DatabaseConnection,
    position_pubkey: &str,
) -> Result<Option<positions::Model>, DbErr> {
    positions::Entity::find()
        .filter(positions::Column::PositionPubkey.eq(position_pubkey))
        .one(db)
        .await
}

pub async fn insert_position(
    db: &DatabaseConnection,
    model: positions::ActiveModel,
) -> Result<(), DbErr> {
    model.insert(db).await?;
    Ok(())
}

/// Marks an already-fetched position closed. Takes the row itself (rather than a pubkey and
/// re-querying) since callers need the pre-close row anyway to build the `position_events` entry.
pub async fn close_position(
    db: &DatabaseConnection,
    existing: positions::Model,
) -> Result<(), DbErr> {
    let mut model: positions::ActiveModel = existing.into();
    model.closed_at = Set(Some(Utc::now().into()));
    model.update(db).await?;
    Ok(())
}

pub async fn insert_position_event(
    db: &DatabaseConnection,
    model: position_events::ActiveModel,
) -> Result<(), DbErr> {
    model.insert(db).await?;
    Ok(())
}

pub async fn insert_funding_update(
    db: &DatabaseConnection,
    model: funding_updates::ActiveModel,
) -> Result<(), DbErr> {
    model.insert(db).await?;
    Ok(())
}

pub async fn insert_market(
    db: &DatabaseConnection,
    model: markets::ActiveModel,
) -> Result<(), DbErr> {
    model.insert(db).await?;
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
    }
    Ok(())
}

pub async fn insert_lp_event(
    db: &DatabaseConnection,
    model: lp_events::ActiveModel,
) -> Result<(), DbErr> {
    model.insert(db).await?;
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
        }
    }
    Ok(())
}
