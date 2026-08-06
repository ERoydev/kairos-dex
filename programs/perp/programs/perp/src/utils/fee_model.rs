use crate::{alliases::MicroUsdc, syntetic_market::FeeSchedule, utils::projected_skew};

/*
Those are `Trade Fees`, they are charged on `open` and `close`. One-time
Paid at open → already deducted from collateral before position exists.
Paid at close → applied when user closes, doesn't affect liquidation trigger.
*/

/// fee_bps: fee in basis points (1 bps = 0.01%). e.g. 10 = 0.10%
/// size: notional position size in USDC base units
/// returns: fee amount in USDC base units
pub fn calculate_base_fee(size: MicroUsdc, fee_bps: MicroUsdc) -> MicroUsdc {
    (size as u128 * fee_bps as u128 / 10_000) as u64
}

pub fn calculate_trade_fee(
    notional: MicroUsdc,
    fee_schedule: &FeeSchedule,
    projected_skew: MicroUsdc,
    max_skew: MicroUsdc,
    worsens_skew: bool,
) -> MicroUsdc {
    let base_fee = calculate_base_fee(notional, fee_schedule.base_fee_bps as u64);

    let skew_fee = if worsens_skew {
        let scaled_bps = (fee_schedule.skew_fee_max_bps as u128 * projected_skew as u128
            / max_skew as u128) as u64;
        calculate_base_fee(notional, scaled_bps)
    } else {
        0
    };

    base_fee + skew_fee
}

/// Calculate if skew gets worse with new position that is going to result in charging extra fee
pub fn worsens_skew(
    curr_oi_long: MicroUsdc,
    curr_oi_short: MicroUsdc,
    notional: MicroUsdc,
    is_long: bool,
) -> bool {
    let curr_skew = curr_oi_long.abs_diff(curr_oi_short);

    let projected_skew = projected_skew(curr_oi_long, curr_oi_short, notional, is_long);

    projected_skew > curr_skew
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn fee_model_test() {}
}
