use anchor_lang::prelude::*;

use crate::{
    position::{Position, PositionType},
    syntetic_market::SynteticMarket,
    POSITION_SEED,
};

pub fn _update_funding(ctx: Context<UpdateFunding>) -> Result<()> {
    // 1. Read oi_long, oi_short, max_skew from market state.
    // 2. Compute skew_ratio = (oi_long - oi_short) / max_skew.
    // 3. Compute funding_rate = skew_ratio × sensitivity_bps, clamped.
    // 4. cumulative_funding_index += funding_rate.
    // 5. last_funding_time = now.
    msg!("Openning Position");
    Ok(())
}

#[derive(Accounts)]
pub struct UpdateFunding<'info> {
    pub system_program: Program<'info, System>,
}
