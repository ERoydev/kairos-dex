use crate::alliases::MicroUsdc;

// TODO: Re-adjust, research which are the best constants for these properties
const MAX_POSITION_NOTIONAL_BPS: u64 = 100; // 1% of TVL
const MAX_OI_SIDE_BPS: u64 = 4_000; // 40% of TVL
const MAX_SKEW_BPS: u64 = 2_000; // 20% of TVL

/// These are never stored — the shared LP pool's TVL moves every deposit/withdraw/trade,
/// so a cap computed from it and then cached on a market account goes stale the moment
/// TVL changes again. Instead every check calls these live, off whatever `lp_pool.total_assets`
/// is at that instant. No bot, no admin resync instruction, no staleness window.
pub fn max_position_notional(lp_tvl: MicroUsdc) -> MicroUsdc {
    scale(lp_tvl, MAX_POSITION_NOTIONAL_BPS)
}

pub fn max_oi_long(lp_tvl: MicroUsdc) -> MicroUsdc {
    scale(lp_tvl, MAX_OI_SIDE_BPS)
}

pub fn max_oi_short(lp_tvl: MicroUsdc) -> MicroUsdc {
    scale(lp_tvl, MAX_OI_SIDE_BPS)
}

/// Bounds *aggregate* skew across every market (see `GlobalConfig::total_oi_long/short`),
/// not a single market's — skew is net directional liability the shared pool carries in
/// USDC, and that sums across markets regardless of which one it came from.
pub fn max_skew(lp_tvl: MicroUsdc) -> MicroUsdc {
    scale(lp_tvl, MAX_SKEW_BPS)
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
