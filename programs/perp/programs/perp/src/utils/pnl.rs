use crate::{alliases::MicroUsdc, position::PositionType};

/// Computes PnL in USDC (same precision as `size`/`margin`) for a position.
/// `size` is the notional exposure in USDC, entry/exit prices share the same decimals.
pub fn calculate_pnl(
    side: PositionType,
    entry_price: MicroUsdc,
    exit_price: MicroUsdc,
    position_size: MicroUsdc,
) -> i64 {
    let entry = entry_price as i128;
    let exit = exit_price as i128;
    let size = position_size as i128;

    // pnl = (exit - entry) * size_usdc / entry,  reversed on `Short`
    let price_diff = match side {
        PositionType::Long => exit - entry,
        PositionType::Short => entry - exit,
    };

    let pnl = price_diff * size / entry;
    pnl as i64
}

/// Applies funding accrued since the position was opened to its collateral.
/// `delta` is how much the market's cumulative funding index moved since entry:
/// a positive delta means longs were crowded, so longs pay and shorts receive.
/// Clamped at 0 — funding can't push collateral negative.
pub fn apply_funding(
    side: PositionType,
    collateral: MicroUsdc,
    notional: MicroUsdc,
    entry_funding_index: i64,
    cumulative_funding_index: i64,
) -> MicroUsdc {
    let delta = cumulative_funding_index - entry_funding_index;
    // actual funding payment in MicroUsdc that this position owes or is owed
    let accrued = (notional as i128 * delta as i128 / 10_000) as i64;

    let adjusted = match side {
        PositionType::Long => collateral as i64 - accrued,
        PositionType::Short => collateral as i64 + accrued,
    };

    adjusted.max(0) as u64 // The collateral value without funding 
}

/// Called from close_position / liquidate after calculate_pnl().
/// Returns (amount_to_pay_trader, credit_amount_to_pool, debit_amount_from_pool)
pub fn settle(margin: u64, pnl: i64, fee: u64) -> (u64, u64, u64) {
    let net = margin as i64 + pnl - fee as i64; // what trader should walk away with

    if net > 0 {
        // trader is owed net back. If net > margin, pool must pay the difference.
        let payout = net as u64;
        if payout > margin { 
            let debit_from_pool = payout - margin;
            (payout, 0, debit_from_pool)
        } else {
            let credit_to_pool = margin - payout;
            (payout, credit_to_pool, 0)
        }
    } else {
        // trader loses everything, margin fully absorbed by pool
        (0, margin, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::assert_eq;

    #[test]
    fn test_calculate_pnl_long() {
        // Working with micro-USDC
        let entry_price: u64 = 73_485_100; // 73.4851 USDC
        let take_profit_exit_price = 75_000_000; // 75 USDC
        let stop_loss_exit_price = 71_000_000;
        let size = 40_000_000 - 40_000; // size - fees(0.04)
        let side1 = PositionType::Long;

        let expected_take_profit_pnl = 823777; // 823777 / 1000000 `10^6` = $0.82, roi=(+4.12%)
        let expected_loss_exit_price = -1351356;

        let pnl_take_profit = calculate_pnl(side1, entry_price, take_profit_exit_price, size);
        let pnl_stop_loss = calculate_pnl(side1, entry_price, stop_loss_exit_price, size);

        assert_eq!(pnl_take_profit, expected_take_profit_pnl, "Errror");
        assert_eq!(pnl_stop_loss, expected_loss_exit_price, "Should be correct");
    }

    #[test]
    fn test_calculate_pnl_short() {
        // Working with micro-USDC
        let entry_price: u64 = 73_485_100; // 73.4851 USDC
        let take_profit_exit_price = 71_000_000;
        let stop_loss_exit_price = 75_000_000;
        let size = 40_000_000 - 40_000;
        let side = PositionType::Short;

        let expected_take_profit_pnl = 1_351_356;
        let expected_stop_loss_pnl = -823_777;

        let pnl_take_profit = calculate_pnl(side, entry_price, take_profit_exit_price, size);
        let pnl_stop_loss = calculate_pnl(side, entry_price, stop_loss_exit_price, size);

        assert_eq!(
            pnl_take_profit, expected_take_profit_pnl,
            "Short take profit wrong"
        );
        assert_eq!(
            pnl_stop_loss, expected_stop_loss_pnl,
            "Short stop loss wrong"
        );
    }
}
