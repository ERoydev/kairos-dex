use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use borsh::BorshDeserialize;

use crate::parser::events::AnchorEvent;
use crate::parser::lp::events::{Credited, Debited, Deposited, LpEvent, Withdrawn};
use crate::parser::notification::{LogsNotification, PROGRAM_DATA_PREFIX};

/// A decoded liquidity-pool event plus the transaction context it came from — see
/// `crate::parser::decoder::DecodedEvent` for why signature/slot travel alongside it.
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedLpEvent {
    pub event: LpEvent,
    pub signature: String,
    pub slot: u64,
}

/// Parses a raw `logsNotification` websocket message into every `liquidity_pool` program
/// event it carries, in log order. Unrelated messages yield an empty vec rather than an error.
pub fn parse_message(raw: &str) -> Vec<DecodedLpEvent> {
    let Ok(notification) = serde_json::from_str::<LogsNotification>(raw) else {
        return Vec::new();
    };

    let signature = notification.params.result.value.signature;
    let slot = notification.params.result.context.slot;

    notification
        .params
        .result
        .value
        .logs
        .iter()
        .filter_map(|log| log.strip_prefix(PROGRAM_DATA_PREFIX))
        .filter_map(|encoded| match decode_event(encoded) {
            Ok(event) => Some(DecodedLpEvent {
                event,
                signature: signature.clone(),
                slot,
            }),
            Err(e) => {
                eprintln!("Failed to decode LP event in tx {signature}: {e}");
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

fn decode_event(encoded: &str) -> Result<LpEvent, DecodeError> {
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
                    .map(LpEvent::$variant)
                    .map_err(DecodeError::Borsh);
            }
        };
    }

    try_decode!(Deposited, Deposited);
    try_decode!(Withdrawn, Withdrawn);
    try_decode!(Credited, Credited);
    try_decode!(Debited, Debited);

    Err(DecodeError::UnknownDiscriminator(discriminator))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a `Program data:` log line the same way Anchor's `emit!` does: an 8-byte
    /// discriminator followed by the borsh-encoded fields, base64'd. We don't have a real
    /// captured `Deposited` log (never run against devnet), so this hand-encodes one from the
    /// struct's known field layout (pool: Pubkey, provider: Pubkey, usdc_amount: u64,
    /// shares_minted: u64) instead.
    fn deposited_log(usdc_amount: u64, shares_minted: u64) -> String {
        let mut payload = Deposited::DISCRIMINATOR.to_vec();
        payload.extend_from_slice(&[1u8; 32]); // pool
        payload.extend_from_slice(&[2u8; 32]); // provider
        payload.extend_from_slice(&usdc_amount.to_le_bytes());
        payload.extend_from_slice(&shares_minted.to_le_bytes());
        format!("Program data: {}", BASE64.encode(payload))
    }

    #[test]
    fn decodes_deposited_event() {
        let log = deposited_log(1_000_000, 500_000);
        let raw = format!(
            r#"{{
                "jsonrpc": "2.0",
                "method": "logsNotification",
                "params": {{
                    "subscription": 1,
                    "result": {{
                        "context": {{ "slot": 42 }},
                        "value": {{
                            "signature": "sig",
                            "err": null,
                            "logs": ["Program log: Instruction: Deposit", "{log}"]
                        }}
                    }}
                }}
            }}"#
        );

        let events = parse_message(&raw);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].slot, 42);
        assert_eq!(events[0].signature, "sig");

        match &events[0].event {
            LpEvent::Deposited(e) => {
                assert_eq!(e.usdc_amount, 1_000_000);
                assert_eq!(e.shares_minted, 500_000);
            }
            other => panic!("expected Deposited, got {other:?}"),
        }
    }

    #[test]
    fn ignores_unrelated_messages() {
        assert!(parse_message(r#"{"jsonrpc":"2.0","result":1,"id":1}"#).is_empty());
    }
}
