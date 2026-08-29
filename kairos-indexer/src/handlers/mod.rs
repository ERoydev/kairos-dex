pub mod funding;
pub mod lp;
pub mod markets;
pub mod positions;

use sea_orm::DatabaseConnection;
use solana_sdk::pubkey::Pubkey;

use crate::parser::accounts::AnchorAccount;
use crate::parser::decoder::DecodedEvent;
use crate::parser::events::PerpEvent;
use crate::parser::lp::decoder::DecodedLpEvent;
use crate::parser::lp::events::LpEvent;
use crate::rpc::RpcClient;

/// Persists a decoded event to Postgres. Events with no backing table (global/caps config
/// changes) are acknowledged and dropped. `rpc`, when set, lets handlers enrich an event with
/// on-chain account state it doesn't itself carry (e.g. a position's side/collateral/notional).
pub async fn dispatch(decoded: DecodedEvent, db: &DatabaseConnection, rpc: Option<&RpcClient>) {
    let DecodedEvent {
        event,
        signature,
        slot,
    } = decoded;

    let result = match event {
        PerpEvent::PositionOpened(e) => {
            positions::position_opened(db, rpc, e, &signature, slot).await
        }
        PerpEvent::PositionClosed(e) => positions::position_closed(db, e, &signature, slot).await,
        PerpEvent::PositionLiquidated(e) => {
            positions::position_liquidated(db, e, &signature, slot).await
        }
        PerpEvent::FundingUpdated(e) => {
            funding::funding_updated(db, rpc, e, &signature, slot).await
        }
        PerpEvent::MarketInitialized(e) => markets::market_initialized(db, rpc, e).await,
        PerpEvent::MarketPaused(e) => markets::market_paused(db, e).await,
        PerpEvent::CapsUpdated(_)
        | PerpEvent::GlobalInitialized(_)
        | PerpEvent::GlobalUpdated(_) => Ok(()),
    };

    if let Err(e) = result {
        eprintln!("Failed to persist event from tx {signature}: {e}");
    }
}

/// Same as `dispatch`, for the `liquidity_pool` program's events.
pub async fn dispatch_lp(
    decoded: DecodedLpEvent,
    db: &DatabaseConnection,
    rpc: Option<&RpcClient>,
) {
    let DecodedLpEvent {
        event,
        signature,
        slot,
    } = decoded;

    let result = match event {
        LpEvent::Deposited(e) => lp::deposited(db, rpc, e, &signature, slot).await,
        LpEvent::Withdrawn(e) => lp::withdrawn(db, rpc, e, &signature, slot).await,
        LpEvent::Credited(e) => lp::credited(db, rpc, e).await,
        LpEvent::Debited(e) => lp::debited(db, rpc, e).await,
    };

    if let Err(e) = result {
        eprintln!("Failed to persist LP event from tx {signature}: {e}");
    }
}

/// Fetches and decodes an on-chain account for a field an event doesn't itself carry. Returns
/// `None` (logging why) if there's no RPC client, the account isn't there yet, or the
/// fetch/decode fails — callers fall back to a placeholder rather than treat this as fatal.
pub(crate) async fn fetch_account<T: AnchorAccount>(
    rpc: Option<&RpcClient>,
    pubkey: &Pubkey,
    context: &str,
) -> Option<T> {
    let account = rpc?.get_account::<T>(pubkey).await;

    match account {
        Ok(Some(account)) => Some(account),
        Ok(None) => {
            eprintln!("{context}: account {pubkey} not found yet");
            None
        }
        Err(err) => {
            eprintln!("{context}: failed to fetch account {pubkey}: {err}");
            None
        }
    }
}
