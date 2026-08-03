

/// fee_bps: fee in basis points (1 bps = 0.01%). e.g. 10 = 0.10%
/// size: notional position size in USDC base units
/// returns: fee amount in USDC base units
pub fn calculate_fee(size: u64, fee_bps: u64) -> u64 {
    // fee = size * fee_bps / 10_000
    (size as u128 * fee_bps as u128 / 10_000) as u64
}
 