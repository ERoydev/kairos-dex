
# Market Parameters for risk management

## Automated modify (Scales with TVL)
Scales with LP TVL (auto-adjust), TVL (Total Value Locked) USDC in LP Pool.

These are just fractions of TVL, they move as the pool grows or shrinks:

- `max_OI_long`
- `max_OI_short`
- `max_position_notional`
- `max_user_notional`
- `max_skew`

Store as fractions, compute live from current TVL. No manual work.

---

## Automated on-signals (bot watches, updates on-chain) -> Coulde be left manuall, since automating these means to have 100% trust in bot

Adjust based on market conditions

These react to volatility, solvency, oracle health — not just pool size:

- max_leverage — drop when volatility spikes.
- market_status — pause on oracle failure or emergency.
- exposure_multiplier — tighten when LP solvency degrades.

---

## Rarely touched

- `maintenance_margin_ratio`
- `fee_schedule (base fees)`
- `max_oracle_staleness`
- `max_oracle_deviation_bps`

---

## Explanations for parameters

### max_skew
`skew` — the difference between long and short open interest. Measures how balanced (or imbalanced) the market is.
```
skew = (oi_long - oi_short)

- Positive: long-heavy (longs > shorts). LP is short exposed.
- Negative: short-heavy (shorts > longs). LP is long exposed.
- Zero: balanced. LP is flat.
```

Cap on |OI_long − OI_short|. Prevents the LP from taking on too much net directional exposure.

**Default:** ~20% of LP pool USDC. Scales with LP TVL over time (updated by risk-keeper or stored as fraction and computed live).

**Check on new trade:**
1. Compute projected OI after the trade.
2. Compute projected skew.
3. Accept if projected_skew ≤ max_skew. Reject otherwise.

**Formulas:**
- New long:  |curr_oi_long + notional − curr_oi_short| ≤ max_skew
- New short: |curr_oi_long − (curr_oi_short + notional)| ≤ max_skew

notional = margin × leverage (position size in USDC)

Skew-reducing trades bypass this check (still subject to all other checks).

