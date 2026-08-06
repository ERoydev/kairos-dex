use anchor_lang::prelude::*;

#[account]
#[derive(InitSpace)]
pub struct Position {
    pub version: u8,
    pub bump: u8,

    pub owner: Pubkey,  // trader who owns this position
    pub market: Pubkey, // which market (or a market ID/enum if not per-account)

    pub side: PositionType, // Long or Short
    pub size: u64,          // notional exposure in USDC
    pub margin: u64,        // collateral locked (after open fee deducted)
    pub entry_price: u64,   // oracle price at open, same decimals as USDC

    pub opened_at: i64, // unix timestamp

    pub is_open: bool, // false once closed/liquidated (or just close the account)
    pub entry_funding_index: i64, // funding index of the Market at entry, used to calculate owed fees

    pub _reserved: [u8; 64], // room for future fields
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace)]
pub enum PositionType {
    Long,
    Short,
}
