use crate::alliases::MicroUsdc;

pub mod caps;
pub mod fee_model;
pub mod pnl;

/// Project the skew that this trade is going to result into (after)
pub fn projected_skew(
    curr_oi_long: &MicroUsdc,
    curr_oi_short: &MicroUsdc,
    notional: &MicroUsdc,
    is_long: bool,
) -> MicroUsdc {
    if is_long {
        (curr_oi_long + notional).abs_diff(*curr_oi_short)
    } else {
        curr_oi_long.abs_diff(curr_oi_short + notional)
    }
}
