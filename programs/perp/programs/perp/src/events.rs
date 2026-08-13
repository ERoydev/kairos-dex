use anchor_lang::prelude::*;

use crate::alliases::MicroUsdc;

#[event]
pub struct MarketInitialized {
    pub market: Pubkey,
    pub symbol: [u8; 16],
    pub authority: Pubkey,
}

#[event]
pub struct GlobalInitialized {
    pub authority: Pubkey,
    pub fee_receiver: Pubkey,
    pub max_markets: u16,
}

#[event]
pub struct GlobalUpdated {
    pub authority: Pubkey,
    pub new_fee_receiver: Option<Pubkey>,
    pub new_is_paused: Option<bool>,
    pub new_max_markets: Option<u16>,
}

#[event]
pub struct MarketPaused {
    pub market: Pubkey,
    pub authority: Pubkey,
    pub is_active: bool,
}

#[event]
pub struct FundingUpdated {
    pub market: Pubkey,
    pub funding_rate_bps: i64,
    pub cumulative_funding_index_bps: i64,
    pub last_funding_time: i64,
}

#[event]
pub struct PositionOpened {
    pub market: Pubkey,
    pub position: Pubkey,
    pub trader: Pubkey,
    pub oracle_price: MicroUsdc,
    pub entry_funding_index: i64,
}