use anchor_lang::prelude::*;


/*
Market PDA

To handle which markets my protocol supports their risk config(max leverage, OI caps, fees)

Basically is going to be handled by On-chain instruction to create the Market setup
and an off-chain script that reads from .json and calls initialize_market for each entry.
It is not a service, it's just a dev script something that is admin-triggered event manually.
*/

#[account]
#[derive(InitSpace)]
pub struct Market {
    pub version: u8,
    pub bump: u8,

    pub authority: Pubkey,           // admin who can update config
    pub symbol: [u8; 16],            // e.g. "BTC-PERP", padded, each char stored in ASCII number

    pub oracle: Pubkey,              // Pyth/Switchboard price feed account

    pub max_leverage: u64,           // e.g. 20
    pub max_open_interest: u64,      // cap on total notional exposure (USDC)
    pub open_interest_long: u64,     // current total long size
    pub open_interest_short: u64,    // current total short size

    pub maintenance_margin_bps: u64, // liquidation threshold, e.g. 500 = 5% `MMR`
    pub open_fee_bps: u64,           // e.g. 10 = 0.10%
    pub close_fee_bps: u64,          // e.g. 10 = 0.10%

    pub is_active: bool,             // admin can pause a market

    pub _reserved: [u8; 64],
}