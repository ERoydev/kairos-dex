use anchor_lang::prelude::*;

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