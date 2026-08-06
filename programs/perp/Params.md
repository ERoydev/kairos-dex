Per-Market Parameters
max_leverage

Ceiling on position leverage at open.

Initial value: look at what mature venues offer for the same asset. BTC/ETH 20–50x, mid-cap alts 5–20x, illiquid 2–5x. Start conservative — loosening later is easier than tightening.
Who moves it: governance sets the outer band, risk-keeper moves within it.
Automated? Yes. Drop when volatility spikes, restore when it calms.

initial_margin_ratio (IMR)

Inverse of max_leverage. Pick one as canonical or they'll drift out of sync.

maintenance_margin_ratio (MMR)

Liquidation threshold. Gap between IMR and MMR is the trader's buffer.

Initial value: roughly half of IMR. Too tight → noise liquidations. Too wide → bad debt.
Who moves it: governance only.
Automated? No. Keep static. React to volatility via max_leverage on new positions, not by moving MMR on existing ones.

max_position_notional

Cap on any single position.

Initial value: 0.5–2% of LP TVL.
Who moves it: risk-keeper within envelope.
Automated? Yes. Scales with LP TVL, tightens under stress.

max_user_notional

Cap on one user's total notional in one market.

Initial value: 3–5× max_position_notional.
Who moves it / automated? Same as max_position_notional.

max_OI_long / max_OI_short

Gross OI caps per side per market.

Initial value: each side ≤ 20–40% of LP TVL. Sum across markets shouldn't blow past LP TVL.
Who moves it: risk-keeper within envelope.
Automated? Yes. Grow slowly (weekly steps), shrink fast on stress.

max_skew

Cap on net directional exposure |OI_long − OI_short|. The most important per-market parameter — every dollar of skew is a real unhedged LP bet.

Initial value: ≤ 0.5× max_OI_side.
Who moves it: risk-keeper within envelope.
Automated? Yes, and the most reactive. Tighten immediately on solvency degradation or vol spikes. Grow slowly.

market_status

ACTIVE / PAUSED / SETTLE_ONLY.

Who moves it: multisig / emergency role, fast path.
Automated? Hard triggers only (oracle stale, oracle deviation). Soft signals (skew high, solvency low) go through multisig — you don't want a monitoring blip to freeze the market.

fee_schedule

Base fees + skew-responsive curve.

Initial value: base taker 5–10 bps. Skew curve near zero at balanced OI, ramps up near max_skew.
Who moves it: governance for base, risk-keeper for skew curve.
Automated? Curve parameters are static; the fee charged at trade time is computed live from OI.

funding_rate_config

Base curve, sensitivity to premium, clamps, interval.

Initial value: the hardest to get right. Common start: proportional to (mark − index) / index, clamped ±0.75%/8h. Copy a mature protocol for the same asset class.
Who moves it: governance sets the curve; risk-keeper tunes sensitivity within bounds.
Automated? Curve static, rate paid computed live. If you're changing curve parameters often, the curve is wrong.