use anchor_lang::prelude::*;

use crate::{alliases::MicroUsdc, POSITION_VERSION};

#[account]
#[derive(InitSpace)]
pub struct Position {
    pub version: u8,
    pub bump: u8,

    pub owner: Pubkey,  // trader who owns this position
    pub market: Pubkey, // which market (or a market ID/enum if not per-account)

    pub side: PositionType,     // Long or Short
    pub collateral: MicroUsdc,  // margin deposited (minus opening fee)
    pub notional: MicroUsdc,    // Position_size = margin x leverage
    pub entry_price: MicroUsdc, // oracle price at open (in MicroUSDC precision)

    pub opened_at: i64, // unix timestamp

    pub entry_funding_index_bps: i64, // funding index of the Market at entry, used to calculate owed fees

    // When position is closed, this account get's closed and rent is returned to owner, no need for bool var
    pub _reserved: [u8; 64], // room for future fields
}

impl Position {
    pub fn get_version() -> u8 {
        POSITION_VERSION
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace)]
pub enum PositionType {
    Long,
    Short,
}
