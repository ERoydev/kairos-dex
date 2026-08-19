use anchor_lang::prelude::*;

use crate::{events::GlobalInitialized, state::global::GlobalConfig, PerpError, GLOBAL_SEED};

/// Admin
pub fn _initialize_global(
    ctx: Context<InitializeGlobal>,
    fee_receiver: Pubkey,
    max_markets: u16,
) -> Result<()> {
    require!(
        fee_receiver != Pubkey::default(),
        PerpError::InvalidFeeReceiver
    );
    require!(max_markets > 0, PerpError::InvalidConfig);

    let config = &mut ctx.accounts.global_config;
    config.authority = ctx.accounts.payer.key();
    config.fee_receiver = fee_receiver;
    config.is_paused = false;
    config.max_markets = max_markets;
    config.markets_count = 0;
    config.bump = ctx.bumps.global_config;

    emit!(GlobalInitialized {
        authority: ctx.accounts.payer.key(),
        fee_receiver,
        max_markets,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct InitializeGlobal<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    #[account(
        init,
        payer = payer,
        space = 8 + GlobalConfig::INIT_SPACE,
        seeds = [GLOBAL_SEED],
        bump
    )]
    pub global_config: Account<'info, GlobalConfig>,

    pub system_program: Program<'info, System>,
}
