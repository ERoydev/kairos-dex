pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("4ZuUyESpBpUfAYperMNg9h6uzKMCNjfL47RsNfD3TnLL");

#[program]
pub mod liquidity_pool {
    use super::*;

    pub fn initialize_pool(ctx: Context<InitializePool>) -> Result<()> {
        _initialize_pool(ctx)
    }

    pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
        _deposit(ctx, amount)
    }

    pub fn withdraw(ctx: Context<Withdraw>, lp_amount: u64) -> Result<()> {
        _withdraw(ctx, lp_amount)
    }

    pub fn credit(ctx: Context<Credit>, amount: u64) -> Result<()> {
        _credit(ctx, amount)
    }

    pub fn debit(ctx: Context<Debit>, amount: u64) -> Result<()> {
        _debit(ctx, amount)
    }
}
