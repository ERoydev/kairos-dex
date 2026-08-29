use anchor_lang::prelude::*;

#[event]
pub struct Deposited {
    pub pool: Pubkey,
    pub provider: Pubkey,
    pub usdc_amount: u64,
    pub shares_minted: u64,
}

#[event]
pub struct Withdrawn {
    pub pool: Pubkey,
    pub provider: Pubkey,
    pub usdc_amount: u64,
    pub shares_burned: u64,
}

#[event]
pub struct Credited {
    pub pool: Pubkey,
    pub caller: Pubkey,
    pub usdc_amount: u64,
}

#[event]
pub struct Debited {
    pub pool: Pubkey,
    pub destination: Pubkey,
    pub usdc_amount: u64,
}
