use crate::{alliases::MicroUsdc, syntetic_market::TvlScaledCaps};

// TODO: Re-adjust, research which are the best constants for these properties
const MAX_POSITION_NOTIONAL_BPS: u64 = 100; // 1% of TVL
const MAX_USER_NOTIONAL_BPS: u64 = 300; // 3% of TVL
const MAX_OI_SIDE_BPS: u64 = 4_000; // 40% of TVL
const MAX_SKEW_BPS: u64 = 2_000; // 20% of TVL

/// Used on Market initialization
pub fn compute_caps(lp_tvl: MicroUsdc) -> TvlScaledCaps {
    TvlScaledCaps {
        max_position_notional: scale(lp_tvl, MAX_POSITION_NOTIONAL_BPS),
        max_user_notional: scale(lp_tvl, MAX_USER_NOTIONAL_BPS),
        max_oi_long: scale(lp_tvl, MAX_OI_SIDE_BPS),
        max_oi_short: scale(lp_tvl, MAX_OI_SIDE_BPS),
        max_skew: scale(lp_tvl, MAX_SKEW_BPS),
    }
}

fn scale(tvl: MicroUsdc, bps: u64) -> MicroUsdc {
    (tvl as u128 * bps as u128 / 10_000) as u64
}

// ANCHOR
/*
Updating over time:
I have two approaches:
1. Bot-driven
    - When LP TVL changes for example with +-10% from last update, calls `update_caps(new_tvl)`
2.Live-computed (no maintanance)
    - I can stop storing caps, instead only bps constants, on every trade compute from live TVL
    `max_skew = scale(lp_pool.tvl, MAX_SKEW_BPS`
    downside is the extra compute cost for the extra accounts in tx, but zero maintenance

*/
