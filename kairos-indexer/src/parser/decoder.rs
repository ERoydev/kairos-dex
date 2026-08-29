use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use borsh::BorshDeserialize;
use serde::Deserialize;

use crate::parser::events::{
    AnchorEvent, CapsUpdated, FundingUpdated, GlobalInitialized, GlobalUpdated,
    MarketInitialized, MarketPaused, PerpEvent, PositionClosed, PositionLiquidated,
    PositionOpened,
};

const PROGRAM_DATA_PREFIX: &str = "Program data: ";

#[derive(Debug, Deserialize)]
struct LogsNotification {
    params: NotificationParams,
}

#[derive(Debug, Deserialize)]
struct NotificationParams {
    result: NotificationResult,
}

#[derive(Debug, Deserialize)]
struct NotificationResult {
    value: NotificationValue,
}

#[derive(Debug, Deserialize)]
struct NotificationValue {
    signature: String,
    logs: Vec<String>,
}

/// Parses a raw `logsNotification` websocket message into every `perp` program event it
/// carries, in log order. Unrelated messages (subscription acks, other programs' logs,
/// unrecognized events) yield an empty vec rather than an error.
pub fn parse_message(raw: &str) -> Vec<PerpEvent> {
    let Ok(notification) = serde_json::from_str::<LogsNotification>(raw) else {
        return Vec::new();
    };

    let signature = &notification.params.result.value.signature;
    notification
        .params
        .result
        .value
        .logs
        .iter()
        .filter_map(|log| log.strip_prefix(PROGRAM_DATA_PREFIX))
        .filter_map(|encoded| match decode_event(encoded) {
            Ok(event) => Some(event),
            Err(e) => {
                eprintln!("Failed to decode event in tx {signature}: {e}");
                None
            }
        })
        .collect()
}

#[derive(Debug)]
pub enum DecodeError {
    InvalidBase64(base64::DecodeError),
    TooShort,
    UnknownDiscriminator([u8; 8]),
    Borsh(std::io::Error),
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidBase64(e) => write!(f, "invalid base64: {e}"),
            Self::TooShort => write!(f, "payload shorter than an 8-byte discriminator"),
            Self::UnknownDiscriminator(d) => write!(f, "unknown discriminator: {d:?}"),
            Self::Borsh(e) => write!(f, "borsh decode failed: {e}"),
        }
    }
}

fn decode_event(encoded: &str) -> Result<PerpEvent, DecodeError> {
    let bytes = BASE64.decode(encoded).map_err(DecodeError::InvalidBase64)?;
    if bytes.len() < 8 {
        return Err(DecodeError::TooShort);
    }

    let (discriminator, mut data) = bytes.split_at(8);
    let discriminator: [u8; 8] = discriminator.try_into().unwrap();

    macro_rules! try_decode {
        ($ty:ty, $variant:ident) => {
            if discriminator == <$ty as AnchorEvent>::DISCRIMINATOR {
                return <$ty>::deserialize(&mut data)
                    .map(PerpEvent::$variant)
                    .map_err(DecodeError::Borsh);
            }
        };
    }

    try_decode!(GlobalInitialized, GlobalInitialized);
    try_decode!(GlobalUpdated, GlobalUpdated);
    try_decode!(MarketInitialized, MarketInitialized);
    try_decode!(MarketPaused, MarketPaused);
    try_decode!(CapsUpdated, CapsUpdated);
    try_decode!(FundingUpdated, FundingUpdated);
    try_decode!(PositionOpened, PositionOpened);
    try_decode!(PositionClosed, PositionClosed);
    try_decode!(PositionLiquidated, PositionLiquidated);

    Err(DecodeError::UnknownDiscriminator(discriminator))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_global_updated_from_a_real_log() {
        let raw = r#"{
            "jsonrpc": "2.0",
            "method": "logsNotification",
            "params": {
                "subscription": 3618558,
                "result": {
                    "context": { "slot": 489835284 },
                    "value": {
                        "signature": "teB3nY2KBnKAPTcJhRdxrvcHQ7k2JAJdzbU9hUp9BqL2MGiaNzAGw9NbE6TZY7zJq2FCf8KeVT7hMWox9RZCiv9",
                        "err": null,
                        "logs": [
                            "Program FWmruxC6TfBGZyXbQtzNjjJVrYMzRWLsTr6iscs9bkyK invoke [1]",
                            "Program log: Instruction: UpdateGlobal",
                            "Program data: g0Lt0bBXd8uVQuWKJAw/5tcK9Tj4A+ArMZKoMKRtuDmardb1XUwvBAAAARQA",
                            "Program FWmruxC6TfBGZyXbQtzNjjJVrYMzRWLsTr6iscs9bkyK consumed 4311 of 200000 compute units",
                            "Program FWmruxC6TfBGZyXbQtzNjjJVrYMzRWLsTr6iscs9bkyK success"
                        ]
                    }
                }
            }
        }"#;

        let events = parse_message(raw);
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], PerpEvent::GlobalUpdated(_)));
    }

    #[test]
    fn ignores_messages_without_program_data() {
        let raw = r#"{
            "jsonrpc": "2.0",
            "method": "logsNotification",
            "params": {
                "subscription": 1,
                "result": {
                    "context": { "slot": 1 },
                    "value": {
                        "signature": "sig",
                        "err": null,
                        "logs": ["Program log: hello"]
                    }
                }
            }
        }"#;

        assert!(parse_message(raw).is_empty());
    }

    #[test]
    fn ignores_non_notification_messages() {
        let raw = r#"{"jsonrpc":"2.0","result":3618558,"id":1}"#;
        assert!(parse_message(raw).is_empty());
    }
}
