
// TODO
pub enum Side {
    Long,
    Short,
}

/// Computes PnL in USDC (same precision as `size`/`margin`) for a position.
/// `size` is the notional exposure in USDC, entry/exit prices share the same decimals.
/// long -> (exit_price - entry_price) * size / entry_price;
/// short -> (entry_price - exit_price) * size / entry_price;
pub fn calculate_pnl(
    side: Side,
    entry_price: u64,
    exit_price: u64,
    size: u64,
) -> i64 {
    let entry = entry_price as i128;
    let exit = exit_price as i128;
    let size = size as i128;
 
    let price_diff = match side {
        Side::Long => exit - entry,
        Side::Short => entry - exit,
    };
 
    // pnl = price_diff * size / entry_price
    let pnl = price_diff * size / entry;
    pnl as i64
}

/// Called from close_position / liquidate after calculate_pnl().
/// Returns (amount_to_pay_trader, credit_amount_to_pool, debit_amount_from_pool)
pub fn settle(margin: u64, pnl: i64, fee: u64) -> (u64, u64, u64) {
    let net = margin as i64 + pnl - fee as i64;
 
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