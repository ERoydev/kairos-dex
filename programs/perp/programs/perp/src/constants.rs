use anchor_lang::prelude::*;

// --- Global configuration variables

#[constant]
pub const MARKET_VERSION: u8 = 1;

#[constant]
pub const DEFAULT_DECIMALS: u8 = 6;


// --- Numerical constants


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
