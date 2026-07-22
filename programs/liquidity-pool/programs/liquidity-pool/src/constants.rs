use anchor_lang::prelude::*;

#[constant]
pub const LIQUIDITY_POOL_SEED: &[u8] = b"liquidity_pool";

#[constant]
pub const POOL_VERSION: u8 = 1;

#[constant]
pub const USDC_VAULT_SEED: &[u8] = b"usdc_vault";

#[constant]
pub const LP_MINT_SEED: &[u8] = b"lp_mint";

#[constant]
pub const DEFAULT_DECIMALS: u8 = 6;

#[constant]
#[cfg(feature = "dev")]
pub const USDC_MINT: Pubkey = pubkey!("4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU");

#[constant]
#[cfg(not(feature = "dev"))]
pub const USDC_MINT: Pubkey = pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");