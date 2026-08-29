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
