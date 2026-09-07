pub mod funding;
pub mod lp;
pub mod markets;
pub mod positions;

use sea_orm::DatabaseConnection;
use solana_sdk::pubkey::Pubkey;

use crate::parser::decoder::DecodedEvent;
use crate::parser::events::PerpEvent;
use crate::parser::lp::decoder::DecodedLpEvent;
use crate::parser::lp::events::LpEvent;
use rpc::AnchorAccount;
use rpc::RpcClient;

/// Persists a decoded event to Postgres. Events with no backing table (global/caps config
/// changes) are acknowledged and dropped. `rpc`, when set, lets handlers enrich an event with
/// on-chain account state it doesn't itself carry (e.g. a position's side/collateral/notional).
pub async fn dispatch(decoded: DecodedEvent, db: &DatabaseConnection, rpc: Option<&RpcClient>) {
    let DecodedEvent {
        event,
        signature,
        slot,
    } = decoded;

    let name = perp_event_name(&event);
    println!("Received {name} (tx {signature})");

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

    match result {
        Ok(()) => println!("Persisted {name} (tx {signature})"),
        Err(e) => eprintln!("Failed to persist {name} (tx {signature}): {e}"),
    }
}

fn perp_event_name(event: &PerpEvent) -> &'static str {
    match event {
        PerpEvent::PositionOpened(_) => "PositionOpened",
        PerpEvent::PositionClosed(_) => "PositionClosed",
        PerpEvent::PositionLiquidated(_) => "PositionLiquidated",
        PerpEvent::FundingUpdated(_) => "FundingUpdated",
        PerpEvent::MarketInitialized(_) => "MarketInitialized",
        PerpEvent::MarketPaused(_) => "MarketPaused",
        PerpEvent::CapsUpdated(_) => "CapsUpdated",
        PerpEvent::GlobalInitialized(_) => "GlobalInitialized",
        PerpEvent::GlobalUpdated(_) => "GlobalUpdated",
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

    let name = lp_event_name(&event);
    println!("Received LP {name} (tx {signature})");

    let result = match event {
        LpEvent::Deposited(e) => lp::deposited(db, rpc, e, &signature, slot).await,
        LpEvent::Withdrawn(e) => lp::withdrawn(db, rpc, e, &signature, slot).await,
        LpEvent::Credited(e) => lp::credited(db, rpc, e).await,
        LpEvent::Debited(e) => lp::debited(db, rpc, e).await,
    };

    match result {
        Ok(()) => println!("Persisted LP {name} (tx {signature})"),
        Err(e) => eprintln!("Failed to persist LP {name} (tx {signature}): {e}"),
    }
}

fn lp_event_name(event: &LpEvent) -> &'static str {
    match event {
        LpEvent::Deposited(_) => "Deposited",
        LpEvent::Withdrawn(_) => "Withdrawn",
        LpEvent::Credited(_) => "Credited",
        LpEvent::Debited(_) => "Debited",
    }
}

const ACCOUNT_FETCH_RETRIES: u32 = 3;
const ACCOUNT_FETCH_RETRY_DELAY: std::time::Duration = std::time::Duration::from_millis(300);

/// Fetches and decodes an on-chain account for a field an event doesn't itself carry. Returns
/// `None` (logging why) if there's no RPC client, the account fetch/decode fails, or the
/// account still isn't visible after a few short retries — callers fall back to a placeholder
/// rather than treat this as fatal.
///
/// The retry exists because the WS `logsSubscribe` notification and a plain `getAccountInfo`
/// call don't always see the same state at the same instant — e.g. Helius may serve them from
/// different backend nodes — so the account this event just created can briefly 404 right
/// after we're notified about the transaction that created it.
pub(crate) async fn fetch_account<T: AnchorAccount>(
    rpc: Option<&RpcClient>,
    pubkey: &Pubkey,
    context: &str,
) -> Option<T> {
    let rpc = rpc?;

    for attempt in 1..=ACCOUNT_FETCH_RETRIES {
        match rpc.get_account::<T>(pubkey).await {
            Ok(Some(account)) => return Some(account),
            Ok(None) if attempt < ACCOUNT_FETCH_RETRIES => {
                tokio::time::sleep(ACCOUNT_FETCH_RETRY_DELAY).await;
            }
            Ok(None) => eprintln!("{context}: account {pubkey} still not found after {attempt} attempts"),
            Err(err) => {
                eprintln!("{context}: failed to fetch account {pubkey}: {err}");
                return None;
            }
        }
    }

    None
}
