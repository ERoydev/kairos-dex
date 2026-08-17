use anchor_lang::prelude::*;

use crate::{error::PerpError, events::MarketPaused, syntetic_market::SynteticMarket};

// Admin instruction
pub fn _market_pause(ctx: Context<MarketPause>, is_active: bool) -> Result<()> {

    require!(
        ctx.accounts.market.is_active != is_active,
        PerpError::MarketAlreadyInState
    );
    ctx.accounts.market.is_active = is_active;

    emit!(MarketPaused {
        market: ctx.accounts.market.key(),
        authority: ctx.accounts.authority.key(),
        is_active,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct MarketPause<'info> {
    #[account(
        constraint = authority.key() == market.authority @ PerpError::Unauthorized
    )]
    pub authority: Signer<'info>,

    #[account(mut)]
    pub market: Account<'info, SynteticMarket>,
}
