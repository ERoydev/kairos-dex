use crate::constants::{LIQUIDITY_POOL_SEED, USDC_VAULT_SEED};
use crate::error::ErrorCode;
use crate::state::pool::Pool;
#[cfg(feature = "dev")]
use crate::DEFAULT_DECIMALS;
#[cfg(not(feature = "dev"))]
use crate::{DEFAULT_DECIMALS, USDC_MINT};
use anchor_lang::prelude::*;
use anchor_spl::token::{transfer, Mint, Token, TokenAccount, Transfer};
use solana_instructions_sysvar::{get_instruction_relative, ID as INSTRUCTIONS_SYSVAR_ID};

// Credit (trader loses): perp pushes USDC → pool vault, pool PDA is not the signer.
// Trader loses → the pool collects their loss as profit → credit the pool (money flows IN)
pub fn _credit(ctx: Context<Credit>, amount: u64) -> Result<()> {
    // The caller here is an actual market.vault PDA, so it is the only thing that can call that.
    // TODO: Maybe is good idea to research how other protocols implement that since it is good idea to have something
    // like whitelist with allowed market vaults or another way to validate authenticity when calling this instruction
    let caller = &ctx.accounts.caller;
    let pool = &ctx.accounts.pool;
    let token_program = &ctx.accounts.token_program;
    let perp_program = pool.perp_program;

    // Only the perp program may reach this instruction. A PDA signer's address
    // is never equal to the program id that derived it, and a token account's
    // AccountInfo.owner is always the token program — neither can be compared
    // directly to perp_program. Instead check which program owns the
    // transaction's currently-executing top-level instruction (instruction
    // introspection via the instructions sysvar): since perp CPIs into this
    // instruction directly, that's perp_program if and only if perp itself
    // was the one dispatched by this transaction.
    let calling_ix = get_instruction_relative(0, &ctx.accounts.instructions.to_account_info())
        .map_err(|_| ErrorCode::Unauthorized)?;
    require!(
        calling_ix.program_id == perp_program,
        ErrorCode::Unauthorized
    );

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

    /// CHECK: address-constrained to the instructions sysvar; used for CPI-caller introspection
    #[account(address = INSTRUCTIONS_SYSVAR_ID)]
    pub instructions: UncheckedAccount<'info>,
}
