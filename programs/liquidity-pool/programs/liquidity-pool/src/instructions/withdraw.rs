use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{burn, transfer, Burn, Mint, Token, TokenAccount, Transfer},
};
#[cfg(not(feature = "dev"))]
use crate::{DEFAULT_DECIMALS, USDC_MINT};
#[cfg(feature = "dev")]
use crate::DEFAULT_DECIMALS;
use crate::state::pool::Pool;
use crate::constants::{LIQUIDITY_POOL_SEED, LP_MINT_SEED, USDC_VAULT_SEED};
use crate::error::ErrorCode;

/// LP burns LP shares → program sends them back proportional USDC.
pub fn _withdraw(ctx: Context<Withdraw>, lp_amount: u64) -> Result<()> {
    let pool = &ctx.accounts.pool;
    let token_program = &ctx.accounts.token_program;
    let provider = &ctx.accounts.provider;

    require!(pool.total_shares > 0, ErrorCode::ZeroShares);

    let usdc_to_return = Withdraw::calc_usdc_to_return(pool.total_shares, pool.total_assets, lp_amount);
    let pool_bump = pool.bump;

    // Burn LP tokens from provider's LP ATA (provider signs as token account owner)
    burn(
        CpiContext::new(
            token_program.key(),
            Burn {
                mint: ctx.accounts.lp_mint.to_account_info(),
                from: ctx.accounts.provider_lp_ata.to_account_info(),
                authority: provider.to_account_info(),
            },
        ),
        lp_amount,
    )?;

    msg!(
        "User: {}, burns LP amount: {}, receives USDC: {}",
        provider.key(), lp_amount, usdc_to_return
    );

    // Transfer USDC from vault to provider, pool PDA signs as vault authority
    transfer(
        CpiContext::new_with_signer(
            token_program.key(),
            Transfer {
                from: ctx.accounts.usdc_vault.to_account_info(),
                to: ctx.accounts.provider_ata.to_account_info(),
                authority: ctx.accounts.pool.to_account_info(),
            },
            &[&[LIQUIDITY_POOL_SEED, &[pool_bump]]],
        ),
        usdc_to_return,
    )?;

    // Update pool accounting
    let pool = &mut ctx.accounts.pool;
    pool.total_shares -= lp_amount;
    pool.total_assets -= usdc_to_return;

    Ok(())
}

#[derive(Accounts)]
pub struct Withdraw<'info> {
    #[account(mut)]
    pub provider: Signer<'info>,

    #[account(
        mut,
        seeds = [LIQUIDITY_POOL_SEED],
        bump = pool.bump,
    )]
    pub pool: Account<'info, Pool>,

    #[account(
        init_if_needed,
        payer = provider,
        associated_token::mint = usdc_mint,
        associated_token::authority = provider,
    )]
    pub provider_ata: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        associated_token::mint = lp_mint,
        associated_token::authority = provider,
    )]
    pub provider_lp_ata: Box<Account<'info, TokenAccount>>,

    // Usdc mint
    #[cfg(not(feature = "dev"))]
    #[account(
        address = USDC_MINT,
        mint::decimals = DEFAULT_DECIMALS,
    )]
    pub usdc_mint: Box<Account<'info, Mint>>,

    #[cfg(feature = "dev")]
    #[account(mint::decimals = DEFAULT_DECIMALS)]
    pub usdc_mint: Box<Account<'info, Mint>>,

    #[account(
        mut,
        token::mint = usdc_mint,
        token::authority = pool,
        seeds = [USDC_VAULT_SEED],
        bump,
    )]
    pub usdc_vault: Box<Account<'info, TokenAccount>>,

    #[account(
        mut,
        mint::decimals = DEFAULT_DECIMALS,
        mint::authority = pool,
        seeds = [LP_MINT_SEED],
        bump,
    )]
    pub lp_mint: Box<Account<'info, Mint>>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

impl<'info> Withdraw<'info> {
    pub fn calc_usdc_to_return(total_shares: u64, total_assets: u64, lp_amount: u64) -> u64 {
        // Proportional share of the pool
        lp_amount * total_assets / total_shares
    }
}
