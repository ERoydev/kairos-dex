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

    // Aggregate open interest across every market, backed by the single shared LP
    // pool. Risk caps (skew, gross OI) that bound the pool's exposure must be
    // checked against these totals, not per-market ones — a market-scoped check
    // against the shared pool's TVL lets every market independently claim the same
    // capacity, so N markets could jointly overdraw a pool sized for one.
    pub total_oi_long: u64,
    pub total_oi_short: u64,
}
