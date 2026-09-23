/// Every `perp` program event this subscriber knows how to decode.
#[derive(Debug)]
pub enum PerpEvent {
    GlobalInitialized(perp::events::GlobalInitialized),
    GlobalUpdated(perp::events::GlobalUpdated),
    MarketInitialized(perp::events::MarketInitialized),
    MarketPaused(perp::events::MarketPaused),
    FundingUpdated(perp::events::FundingUpdated),
    PositionOpened(perp::events::PositionOpened),
    PositionClosed(perp::events::PositionClosed),
    PositionLiquidated(perp::events::PositionLiquidated),
}

/// Tries every known `perp` event type against one log line via Anchor's own
/// `handle_program_log` (base64-decode + discriminator check + borsh-deserialize).
fn decode_perp_log(program_id_str: &str, log: &str) -> Option<PerpEvent> {
    macro_rules! try_decode {
        ($ty:path, $variant:ident) => {
            if let Ok((Some(event), _, _)) =
                anchor_client::handle_program_log::<$ty>(program_id_str, log)
            {
                return Some(PerpEvent::$variant(event));
            }
        };
    }

    try_decode!(perp::events::GlobalInitialized, GlobalInitialized);
    try_decode!(perp::events::GlobalUpdated, GlobalUpdated);
    try_decode!(perp::events::MarketInitialized, MarketInitialized);
    try_decode!(perp::events::MarketPaused, MarketPaused);
    try_decode!(perp::events::FundingUpdated, FundingUpdated);
    try_decode!(perp::events::PositionOpened, PositionOpened);
    try_decode!(perp::events::PositionClosed, PositionClosed);
    try_decode!(perp::events::PositionLiquidated, PositionLiquidated);

    None
}

/// Extracts every `Program data:`/`Program log:` line from a raw `logsNotification`
/// websocket message and decodes each into a `PerpEvent`.
pub fn decode_perp_events(program_id_str: &str, text: &str) -> Vec<PerpEvent> {
    logs_from(text)
        .iter()
        .filter_map(|log| decode_perp_log(program_id_str, log))
        .collect()
}

/// Every `liquidity_pool` program event this subscriber knows how to decode.
#[derive(Debug)]
pub enum LpEvent {
    Deposited(liquidity_pool::events::Deposited),
    Withdrawn(liquidity_pool::events::Withdrawn),
    Credited(liquidity_pool::events::Credited),
    Debited(liquidity_pool::events::Debited),
}

/// Tries every known `liquidity_pool` event type against one log line via Anchor's own
/// `handle_program_log` (base64-decode + discriminator check + borsh-deserialize).
fn decode_lp_log(program_id_str: &str, log: &str) -> Option<LpEvent> {
    macro_rules! try_decode {
        ($ty:path, $variant:ident) => {
            if let Ok((Some(event), _, _)) =
                anchor_client::handle_program_log::<$ty>(program_id_str, log)
            {
                return Some(LpEvent::$variant(event));
            }
        };
    }

    try_decode!(liquidity_pool::events::Deposited, Deposited);
    try_decode!(liquidity_pool::events::Withdrawn, Withdrawn);
    try_decode!(liquidity_pool::events::Credited, Credited);
    try_decode!(liquidity_pool::events::Debited, Debited);

    None
}

/// Extracts every `Program data:`/`Program log:` line from a raw `logsNotification`
/// websocket message and decodes each into an `LpEvent`.
pub fn decode_lp_events(program_id_str: &str, text: &str) -> Vec<LpEvent> {
    logs_from(text)
        .iter()
        .filter_map(|log| decode_lp_log(program_id_str, log))
        .collect()
}

/// Pulls the `logs` array out of a raw `logsNotification` websocket message.
fn logs_from(text: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
        return Vec::new();
    };

    let Some(logs) = value["params"]["result"]["value"]["logs"].as_array() else {
        return Vec::new();
    };

    logs.iter()
        .filter_map(|log| log.as_str())
        .map(str::to_owned)
        .collect()
}
