use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};
use crate::POOL_VERSION;
use crate::state::pool::Pool;
use crate::constants::{LIQUIDITY_POOL_SEED, USDC_VAULT_SEED};
use crate::error::ErrorCode;

pub fn _initialize_pool(ctx: Context<InitializePool>) -> Result<()> {
    require!(POOL_VERSION == 1, ErrorCode::InvalidPoolVersion);

    let pool = &mut ctx.accounts.pool;
    pool.version = POOL_VERSION;
    pool.bump = ctx.bumps.pool;
    pool.authority = ctx.accounts.authority.key();
    pool.usdc_mint = ctx.accounts.usdc_mint.key();
    pool.usdc_vault = ctx.accounts.usdc_vault.key();
    pool.lp_mint = Pubkey::default();
    pool.total_assets = 0;
    pool.total_shares = 0;

    Ok(())
}

#[derive(Accounts)]
pub struct InitializePool<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + Pool::SIZE,
        seeds = [LIQUIDITY_POOL_SEED],
        bump
    )]
    pub pool: Account<'info, Pool>,

    pub usdc_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = authority,
        token::mint = usdc_mint,
        token::authority = pool,
        seeds = [USDC_VAULT_SEED],
        bump,
    )]
    pub usdc_vault: Account<'info, TokenAccount>, // my token account holding USDC, owned by the pool

    #[account(mut)]
    pub authority: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}
