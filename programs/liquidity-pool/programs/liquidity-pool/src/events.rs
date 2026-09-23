use anchor_lang::prelude::*;

#[event]
#[derive(Debug, Clone, PartialEq)]
pub struct Deposited {
    pub pool: Pubkey,
    pub provider: Pubkey,
    pub usdc_amount: u64,
    pub shares_minted: u64,
}

#[event]
#[derive(Debug, Clone, PartialEq)]
pub struct Withdrawn {
    pub pool: Pubkey,
    pub provider: Pubkey,
    pub usdc_amount: u64,
    pub shares_burned: u64,
}

#[event]
#[derive(Debug, Clone, PartialEq)]
pub struct Credited {
    pub pool: Pubkey,
    pub caller: Pubkey,
    pub usdc_amount: u64,
}

#[event]
#[derive(Debug, Clone, PartialEq)]
pub struct Debited {
    pub pool: Pubkey,
    pub destination: Pubkey,
    pub usdc_amount: u64,
}
