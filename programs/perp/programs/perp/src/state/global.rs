use anchor_lang::prelude::*;

/*
Global State for the Perp Program
*/

#[account]
#[derive(InitSpace, Debug)]
pub struct GlobalConfig {
    pub authority: Pubkey,    // admin
    pub fee_receiver: Pubkey, // where protocol fees go (fees that goes for the profit)
    pub is_paused: bool,      // emergency circuit breaker
    pub max_markets: u16,     // cap on number of markets
    pub markets_count: u16,
    pub bump: u8,
}
