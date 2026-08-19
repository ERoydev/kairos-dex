use anchor_lang::prelude::*;

use crate::{alliases::MicroUsdc, events::CapsUpdated, syntetic_market::SynteticMarket, PerpError};

/// Admin override for a market's TVL-scaled risk caps.
pub fn _update_caps(ctx: Context<UpdateCaps>, params: UpdateCapsParams) -> Result<()> {
    let caps = &mut ctx.accounts.market.risk_management.caps;

    if let Some(max_position_notional) = params.max_position_notional {
        caps.max_position_notional = max_position_notional;
    }
    if let Some(max_user_notional) = params.max_user_notional {
        caps.max_user_notional = max_user_notional;
    }
    if let Some(max_oi_long) = params.max_oi_long {
        caps.max_oi_long = max_oi_long;
    }
    if let Some(max_oi_short) = params.max_oi_short {
        caps.max_oi_short = max_oi_short;
    }
    if let Some(max_skew) = params.max_skew {
        caps.max_skew = max_skew;
    }

    emit!(CapsUpdated {
        market: ctx.accounts.market.key(),
        caps: ctx.accounts.market.risk_management.caps.clone(),
    });

    Ok(())
}

#[derive(Accounts)]
pub struct UpdateCaps<'info> {
    #[account(
        constraint = authority.key() == market.authority @ PerpError::Unauthorized
    )]
    pub authority: Signer<'info>,

    #[account(mut)]
    pub market: Account<'info, SynteticMarket>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct UpdateCapsParams {
    pub max_position_notional: Option<MicroUsdc>,
    pub max_user_notional: Option<MicroUsdc>,
    pub max_oi_long: Option<MicroUsdc>,
    pub max_oi_short: Option<MicroUsdc>,
    pub max_skew: Option<MicroUsdc>,
}
