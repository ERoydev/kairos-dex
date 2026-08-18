use anchor_lang::prelude::*;

// --- Global configuration variables

#[constant]
pub const MARKET_VERSION: u8 = 1;

#[constant]
pub const POSITION_VERSION: u8 = 1;

#[constant]
pub const DEFAULT_DECIMALS: u8 = 6;

#[constant]
pub const PROTOCOL_FEES_BPS: u16 = 1500;

#[constant]
pub const LP_FEES_BPS: u16 = 8500;

#[constant]
pub const PYTH_MAX_PRICE_AGE_SECONDS: u64 = 30;

// Pyth prices are `price * 10^exponent`. Every feed this program reads is expected
// to publish at exponent -8; convert_price_to_micro_usdc relies on this to scale
// down to MicroUsdc's 6 decimals.
#[constant]
pub const PYTH_PRICE_EXPONENT: i32 = -8;

// --- Numerical constants

#[constant]
pub const SENSITIVITY_BPS: u16 = 75; // 0.75 multiplier, it is replacement for premium

#[constant]
pub const MAX_RATE_BPS: u16 = 75; // 0.75 cap of funding rate per hour. Strong enough to correct skew, not so brutal to liquidate via funding alone

#[constant]
pub const INTERVAL_SECONDS: u16 = 3600;

// FeeSchedule
#[constant]
pub const BASE_FEE_BPS: u16 = 10; // 0.10%, charged on both open and close

#[constant]
pub const SKEW_FEE_MAX_BPS: u16 = 20; // up to 0.20% extra at max skew

// --- Seeds

#[constant]
pub const POSITION_SEED: &[u8] = b"position";

#[constant]
pub const MARKET_SEED: &[u8] = b"market_seed";

#[constant]
pub const GLOBAL_SEED: &[u8] = b"global";

#[constant]
pub const MARKET_VAULT: &[u8] = b"market_vault";
