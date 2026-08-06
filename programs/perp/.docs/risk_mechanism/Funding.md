# Funding
reference: https://hyperliquid.gitbook.io/hyperliquid-docs/trading/funding#technical-details
https://medium.com/@compasslabs/a-guide-to-perpetual-contracts-and-gmx-v2-a4770cbc25e3

I need funding mechanism in order to balance what counter poistion my LP Pool takes in order to balance profin and losses.

Firstly the whole point of `Funding` is trader to trader, not me:
- Longs crowded → longs pay shorts. Money moves from long positions' collateral to short positions' collateral.
- Shorts crowded → shorts pay longs. Opposite direction.

**You (the protocol / LP) don't touch funding. It's just a redistribution mechanism between the two sides to keep the market balanced.**

Generally we have two places where we settle funding:
1. Bot side (off-chain, read-only):

- Reads all positions + market's cumulative index.
- For each position, computes what the funding-adjusted collateral would be.
- Uses that to decide if the position is liquidatable.
- Bot never writes anything — just triggers instructions.

2. On-chain side (actual settlement):
Funding is actually applied to collateral only when the position is touched:

- User closes → settle funding, refund whatever's left.
- Liquidator triggers → settle funding, then liquidate.
- Any other position interaction → settle funding first.


## Funding Fee Math

```
skew = oi_long - oi_short
skew_ratio = skew / max_skew        // between -1 and +1
funding_rate = skew_ratio × sensitivity_bps
funding_rate = clamp(funding_rate, ±max_rate_bps)
```


1. Cumulative index tick (bot calls this per interval)

market.cumulative_funding_index += funding_rate
market.last_funding_time = now

2. Per-position accrued funding (computed on settlement)

delta = market.cumulative_funding_index - position.entry_funding_index
accrued = notional × delta

for long:  collateral -= accrued  // positive delta = long pays
for short: collateral += accrued  // positive delta = short receives

3. Position setup (on open)

position.entry_funding_index = market.cumulative_funding_index  // snapshot at open

### Parameters

`sensitivity_bps` -> replaces the premium calculation(diff between spot price and contract price). Instead of "how far mark is from spot," you use "how far skew is from balanced."
`max_rate_bps` -> safety cap on the final funding rate.
`interval_seconds` -> bot invokes update_funding() in this interval

`cumulative_funding_index_bps` -> updated by update_funding()
`max_skew` -> hard cap on |oi_long - oi_short|. Represents the maximum net directional exposure the LP is willing to take. Default ~20% of LP TVL. Used both as a hard reject threshold on new trades and as the denominator for skew_ratio in funding calculation.


### Order of implementation
Add cumulative_funding_index and last_funding_time to the market struct.
Add entry_funding_index to the position struct.
Write update_funding instruction (bot calls this) — does step 1.
In open_position: snapshot entry_funding_index (step 3).
In close_position / liquidate_position / health check: settle funding (step 2) before doing anything else.

Then run a bot that calls update_funding every interval_seconds. Done.