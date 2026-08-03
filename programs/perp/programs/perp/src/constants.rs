use anchor_lang::prelude::*;

// --- Numerical constants
#[constant]
pub const DEFAULT_DECIMALS: u8 = 6;

#[constant]
pub const TRADE_FEE_BPS: u64 = 10; // 0.10%, charged on both open and close

#[constant]
pub const MARKET_VERSION: u8 = 1;

// --- Seeds

#[constant]
pub const POSITION_SEED: &[u8] = b"position";

#[constant]
pub const MARKET_SEED: &[u8] = b"market_seed";

#[constant]
pub const GLOBAL_SEED: &[u8] = b"global";
