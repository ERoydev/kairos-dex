use anchor_lang::prelude::*;
use anchor_spl::token::{transfer, Mint, Token, TokenAccount, Transfer};
#[cfg(not(feature = "dev"))]
use crate::{DEFAULT_DECIMALS, USDC_MINT};
#[cfg(feature = "dev")]
use crate::DEFAULT_DECIMALS;
use crate::state::pool::Pool;
use crate::constants::{LIQUIDITY_POOL_SEED, USDC_VAULT_SEED};
use crate::error::ErrorCode;

/// Perp contract CPIs here when the pool pays out USDC (trader wins).
pub fn _debit(ctx: Context<Debit>, amount: u64) -> Result<()> {
    let caller = &ctx.accounts.caller;
    let pool = &ctx.accounts.pool;
    let token_program = &ctx.accounts.token_program;
    let perp_program = pool.perp_program;

    // Only the pool program can call this via CPI
    require!(caller.key() == perp_program, ErrorCode::Unauthorized);

    // Solvency: pool must be able to cover the payout
    require!(pool.total_assets >= amount, ErrorCode::InsufficientFunds);
    
    let pool_bump = pool.bump;

    msg!("Debit pool: {} USDC to {}", amount, ctx.accounts.destination.key());
    transfer(
        CpiContext::new_with_signer(
            token_program.key(),
            Transfer {
                from: ctx.accounts.usdc_vault.to_account_info(),
                to: ctx.accounts.destination.to_account_info(),
                authority: ctx.accounts.pool.to_account_info(),
            },
            &[&[LIQUIDITY_POOL_SEED, &[pool_bump]]],
        ),
        amount,
    )?;

    let pool = &mut ctx.accounts.pool;
    pool.total_assets -= amount;

    Ok(())
}

#[derive(Accounts)]
pub struct Debit<'info> {
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
        token::authority = pool,
        seeds = [USDC_VAULT_SEED],
        bump,
    )]
    pub usdc_vault: Account<'info, TokenAccount>,

    /// The perp program's vault or escrow that receives the USDC payout.
    #[account(
        mut,
        token::mint = usdc_mint,
    )]
    pub destination: Account<'info, TokenAccount>,

    #[cfg(not(feature = "dev"))]
    #[account(
        address = USDC_MINT,
        mint::decimals = DEFAULT_DECIMALS,
    )]
    pub usdc_mint: Account<'info, Mint>,

    #[cfg(feature = "dev")]
    #[account(mint::decimals = DEFAULT_DECIMALS)]
    pub usdc_mint: Account<'info, Mint>,

    pub token_program: Program<'info, Token>,
}
