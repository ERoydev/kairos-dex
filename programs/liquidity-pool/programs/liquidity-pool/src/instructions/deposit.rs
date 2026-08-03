use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{mint_to, transfer, Mint, MintTo, Token, TokenAccount, Transfer},
};
#[cfg(not(feature = "dev"))]
use crate::{DEFAULT_DECIMALS, USDC_MINT};
#[cfg(feature = "dev")]
use crate::DEFAULT_DECIMALS;
use crate::state::pool::Pool;
use crate::constants::{LIQUIDITY_POOL_SEED, LP_MINT_SEED, USDC_VAULT_SEED};

/// LP sends USDC → program mints them LP shares based on current share price.
pub fn _deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {

    let provider_ata = &mut ctx.accounts.provider_ata;
    let token_program = &ctx.accounts.token_program;
    let usdc_vault = &mut ctx.accounts.usdc_vault;
    let provider = &mut ctx.accounts.provider;
    let pool = &ctx.accounts.pool;

    // Transfer USDC from user ata to vault
    let transfer_token_cpi = CpiContext::new(
        token_program.key(),
        Transfer {
            from: provider_ata.to_account_info(),
            to: usdc_vault.to_account_info(),
            authority: provider.to_account_info()
        }
    );

    msg!(
        "User: {}, send USDC amount: {} to vault",
        provider.key(), amount
    );

    transfer(transfer_token_cpi, amount)?;

    // Mint LP tokens to user ata, signed by pool PDA
    let amount_to_mint = Deposit::calc_lp_to_mint(pool.total_shares, pool.total_assets, amount);
    let pool_bump = pool.bump;

    mint_to(
        CpiContext::new_with_signer(
            token_program.key(),
            MintTo {
                mint: ctx.accounts.lp_mint.to_account_info(),
                to: ctx.accounts.provider_lp_ata.to_account_info(),
                authority: ctx.accounts.pool.to_account_info(),
            },
            // The PDA must sign as authority
            &[&[LIQUIDITY_POOL_SEED, &[pool_bump]]],
        ),
        amount_to_mint

    )?;

    // Update pool accounting
    let pool = &mut ctx.accounts.pool;
    pool.total_assets += amount;
    pool.total_shares += amount_to_mint;

    Ok(())
}

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(mut)]
    pub provider: Signer<'info>,

    #[account(
        mut,
        seeds = [LIQUIDITY_POOL_SEED],
        bump = pool.bump,
    )]
    pub pool: Account<'info, Pool>,

    #[account(
        mut,
        associated_token::mint = usdc_mint,
        associated_token::authority = provider,
        )]
    pub provider_ata: Account<'info, TokenAccount>,

    #[account(
        init_if_needed,
        payer = provider,
        associated_token::mint = lp_mint,
        associated_token::authority = provider,
    )]
    pub provider_lp_ata: Account<'info, TokenAccount>,

    // Usdc mint
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

    #[account(
        mut,
        mint::decimals = DEFAULT_DECIMALS,
        mint::authority = pool,
        seeds = [LP_MINT_SEED],
        bump,
    )]
    pub lp_mint: Account<'info, Mint>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

impl<'info> Deposit<'info> {
    pub fn calc_lp_to_mint(total_shares: u64, total_assets: u64, amount: u64) -> u64 {
        if total_shares == 0 {
            amount
        } else {
            // On deposit users get fewer shares as the pool grows
            amount * total_shares / total_assets
        }
    }
}
