use sea_orm::{DatabaseConnection, DbErr, Set};
use solana_sdk::pubkey::Pubkey;

use crate::db::entities::lp_events;
use crate::db::queries;
use crate::parser::lp::accounts::Pool;
use crate::parser::lp::events::{Credited, Debited, Deposited, Withdrawn};
use rpc::RpcClient;

pub async fn deposited(
    db: &DatabaseConnection,
    rpc: Option<&RpcClient>,
    e: Deposited,
    signature: &str,
    slot: u64,
) -> Result<(), DbErr> {
    queries::insert_lp_event(
        db,
        lp_events::ActiveModel {
            event_type: Set("deposit".to_string()),
            provider: Set(e.provider.to_string()),
            usdc_amount: Set(e.usdc_amount as i64),
            shares_amount: Set(e.shares_minted as i64),
            tx_signature: Set(signature.to_string()),
            slot: Set(slot as i64),
            ..Default::default()
        },
    )
    .await?;

    sync_pool(db, rpc, &e.pool, "deposited").await
}

pub async fn withdrawn(
    db: &DatabaseConnection,
    rpc: Option<&RpcClient>,
    e: Withdrawn,
    signature: &str,
    slot: u64,
) -> Result<(), DbErr> {
    queries::insert_lp_event(
        db,
        lp_events::ActiveModel {
            event_type: Set("withdraw".to_string()),
            provider: Set(e.provider.to_string()),
            usdc_amount: Set(e.usdc_amount as i64),
            shares_amount: Set(e.shares_burned as i64),
            tx_signature: Set(signature.to_string()),
            slot: Set(slot as i64),
            ..Default::default()
        },
    )
    .await?;

    sync_pool(db, rpc, &e.pool, "withdrawn").await
}

/// credit/debit are pnl settlement flows between perp and the pool (a market paying into or
/// drawing from the vault), not LP provider actions — `lp_events` only models 'deposit' |
/// 'withdraw' per its schema. We still refresh `lp_pool`'s totals since these do move
/// `total_assets`; there's just no per-provider row to write for them.
pub async fn credited(
    db: &DatabaseConnection,
    rpc: Option<&RpcClient>,
    e: Credited,
) -> Result<(), DbErr> {
    sync_pool(db, rpc, &e.pool, "credited").await
}

pub async fn debited(
    db: &DatabaseConnection,
    rpc: Option<&RpcClient>,
    e: Debited,
) -> Result<(), DbErr> {
    sync_pool(db, rpc, &e.pool, "debited").await
}

/// Refreshes `lp_pool`'s totals from the on-chain `Pool` account rather than accumulating
/// deltas locally — avoids drift if an event is ever missed or double-delivered.
async fn sync_pool(
    db: &DatabaseConnection,
    rpc: Option<&RpcClient>,
    pool_pubkey: &Pubkey,
    context: &str,
) -> Result<(), DbErr> {
    let Some(pool) = super::fetch_account::<Pool>(rpc, pool_pubkey, context).await else {
        return Ok(());
    };

    queries::upsert_lp_pool(
        db,
        &pool_pubkey.to_string(),
        pool.total_assets as i64,
        pool.total_shares as i64,
    )
    .await
}
