use anchor_lang::prelude::*;
use anchor_spl::token::{transfer, Mint, Token, TokenAccount, Transfer};
#[cfg(not(feature = "dev"))]
use crate::{DEFAULT_DECIMALS, USDC_MINT};
#[cfg(feature = "dev")]
use crate::DEFAULT_DECIMALS;
use crate::state::pool::Pool;
use crate::constants::{LIQUIDITY_POOL_SEED, USDC_VAULT_SEED};
use crate::error::ErrorCode;


// Credit (trader loses): perp pushes USDC → pool vault, pool PDA is not the signer.
// Trader loses → the pool collects their loss as profit → credit the pool (money flows IN)

pub fn _credit(ctx: Context<Credit>, amount: u64) -> Result<()> {
    let caller = &ctx.accounts.caller;
    let pool = &ctx.accounts.pool;
    let token_program = &ctx.accounts.token_program;
    let perp_program = pool.perp_program;

    // Only the pool program can call this via CPI
    require!(caller.key() == perp_program, ErrorCode::Unauthorized);

    let transfer_cpi = CpiContext::new(
        token_program.key(),
        Transfer {
            from: ctx.accounts.source.to_account_info(),
            to: ctx.accounts.usdc_vault.to_account_info(),
            authority: caller.to_account_info(),
        },
    );

    msg!("Credit pool: {} USDC from {}", amount, caller.key());
    transfer(transfer_cpi, amount)?;

    let pool = &mut ctx.accounts.pool;
    pool.total_assets += amount;

    Ok(())
}

#[derive(Accounts)]
pub struct Credit<'info> {
    pub caller: Signer<'info>,

    #[account(
        mut,
        seeds = [LIQUIDITY_POOL_SEED],
        bump = pool.bump,
    )]
    pub pool: Account<'info, Pool>,

    #[account(
        mut,
        token::mint = usdc_mint,
        token::authority = caller,
    )]
    pub source: Account<'info, TokenAccount>,

    #[cfg(not(feature = "dev"))]
    #[account(
        address = USDC_MINT,
        mint::decimals = DEFAULT_DECIMALS,
    )]
    pub usdc_mint: Account<'info, Mint>,

    #[cfg(feature = "dev")]
    #[account(mint::decimals = DEFAULT_DECIMALS)]
    pub usdc_mint: Account<'info, Mint>,

    #[account(
        mut,
        token::mint = usdc_mint,
        token::authority = pool,
        seeds = [USDC_VAULT_SEED],
        bump,
    )]
    pub usdc_vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}
