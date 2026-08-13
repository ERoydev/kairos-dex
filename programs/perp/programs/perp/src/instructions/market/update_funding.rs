use anchor_lang::prelude::*;

use crate::{alliases::MicroUsdc, events::FundingUpdated, syntetic_market::SynteticMarket, PerpError};

pub fn _update_funding(ctx: Context<UpdateFunding>) -> Result<()> {
    let market = &mut ctx.accounts.market;

    let now = Clock::get()?.unix_timestamp;
    let elapsed = now - market.funding_fees.last_funding_time;
    require!(
        elapsed >= market.funding_config.interval_seconds as i64,
        PerpError::FundingTooEarly
    );

    // 1. Compute skew_ratio = (oi_long - oi_short) / max_skew.
    let skew_ratio_bps = UpdateFunding::compute_skew_ratio_bps(
        market.oi_long,
        market.oi_short,
        market.risk_management.caps.max_skew,
    );

    // 2. Compute funding_rate = skew_ratio × sensitivity_bps, clamped.
    let funding_rate_bps: i64 = UpdateFunding::compute_funding_rate(
        skew_ratio_bps,
        market.funding_config.sensitivity_bps,
        market.funding_config.max_rate_bps,
    );

    // 3. cumulative_funding_index += funding_rate.
    market.funding_fees.cumulative_funding_index_bps += funding_rate_bps;

    // 4. last_funding_time = now.
    market.funding_fees.last_funding_time = now;

    emit!(FundingUpdated {
        market: market.key(),
        funding_rate_bps,
        cumulative_funding_index_bps: market.funding_fees.cumulative_funding_index_bps,
        last_funding_time: now,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct UpdateFunding<'info> {
    pub signer: Signer<'info>,

    #[account(mut)]
    pub market: Account<'info, SynteticMarket>,
    pub system_program: Program<'info, System>,
}

impl<'info> UpdateFunding<'info> {
    pub fn compute_skew_ratio_bps(
        curr_oi_long: MicroUsdc,
        curr_oi_short: MicroUsdc,
        max_skew: MicroUsdc,
    ) -> i64 {
        if max_skew == 0 {
            return 0;
        }
        let skew = curr_oi_long as i128 - curr_oi_short as i128;
        (skew * 10_000 / max_skew as i128) as i64 // Scale before divison to keep floats -> 7_500_000 / 500 = 15_000 instead of `1`
    }

    pub fn compute_funding_rate(
        skew_ratio_bps: i64,
        sensitivity_bps: u16,
        max_rate_bps: u16,
    ) -> i64 {
        let funding_rate_bps = skew_ratio_bps * sensitivity_bps as i64 / 10_000;
        let max = max_rate_bps as i64;
        funding_rate_bps.clamp(-max, max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn compute_skew_ratio() {
        let curr_oi_long = 1250;
        let curr_oi_short = 500;
        let max_skew = 500;

        let result = UpdateFunding::compute_skew_ratio_bps(curr_oi_long, curr_oi_short, max_skew);
        assert_eq!(result, 15_000, "Must be 15_000");
    }

    #[test]
    pub fn compute_funding_rate() {
        let skew_ratio_bps: i64 = 2_000;
        let sensitivity_bps: u16 = 75;
        let max_rate_bps: u16 = 75;

        let result =
            UpdateFunding::compute_funding_rate(skew_ratio_bps, sensitivity_bps, max_rate_bps);
        assert_eq!(result, 15, "Must be 15 funding rate bps");
    }
}
