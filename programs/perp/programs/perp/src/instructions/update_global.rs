use anchor_lang::prelude::*;

use crate::{events::GlobalUpdated, state::global::GlobalConfig, PerpError, GLOBAL_SEED};

pub fn _update_global(ctx: Context<UpdateGlobal>, params: UpdateGlobalParams) -> Result<()> {
    let config = &mut ctx.accounts.global_config;

    if let Some(authority) = params.authority {
        require!(authority != Pubkey::default(), PerpError::InvalidAuthority);
        config.authority = authority;
    }
    if let Some(fee_receiver) = params.fee_receiver {
        require!(
            fee_receiver != Pubkey::default(),
            PerpError::InvalidFeeReceiver
        );
        config.fee_receiver = fee_receiver;
    }
    if let Some(is_paused) = params.is_paused {
        config.is_paused = is_paused;
    }
    if let Some(max_markets) = params.max_markets {
        config.max_markets = max_markets;
    }

    emit!(GlobalUpdated {
        authority: ctx.accounts.authority.key(),
        new_fee_receiver: params.fee_receiver,
        new_is_paused: params.is_paused,
        new_max_markets: params.max_markets,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct UpdateGlobal<'info> {
    #[account(
        constraint = authority.key() == global_config.authority @ PerpError::Unauthorized
    )]
    pub authority: Signer<'info>,

    #[account(
        mut,
        seeds = [GLOBAL_SEED],
        bump = global_config.bump,
    )]
    pub global_config: Account<'info, GlobalConfig>,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct UpdateGlobalParams {
    pub authority: Option<Pubkey>,
    pub fee_receiver: Option<Pubkey>,
    pub is_paused: Option<bool>,
    pub max_markets: Option<u16>,
}
