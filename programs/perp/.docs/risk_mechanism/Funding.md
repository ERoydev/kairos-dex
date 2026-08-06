# Funding
reference: https://hyperliquid.gitbook.io/hyperliquid-docs/trading/funding#technical-details

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

I will keep track of two cumulative funding fees.
- the current_cumulative_funding_fee
- the entry_cumulative_funding_fee

Then i keep track of another cumulative funding fees.
- long_cumulative_funding_fee
- short_cumulative_funding_fee

1. funding fee per position size, that this user have to pay, because he took some positions from t2 to t5 for example
F_i = funding_fee_at_time_i/long_open_interest_at_time_i

2. side that is going to claim this funding fee, this results in amount of funding fee that shorts can claim per position size at time i
C_i = funding_fee_at_time_i/short_open_interest_at_time_i