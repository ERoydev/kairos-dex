# Risk Management

## Minimum set for a stable perp
1. Fees (entry)
2. Funding OR borrow fee (holding)
3. Mark price (skew-adjusted)
    - Mark price is the price my protocol uses internally
4. Position/OI/skew caps (prevention)
    - max_position_notional, max_user_notional, max_OI_long, max_OI_short, max_skew. Enforced when a trade opens.
5. Oracle safety (guard)
    - reject trades when price fee is broken
    - Two checks on every trader (Staleness-price is fresh, Deviation-price hasn't jumped absurdly from recent history)
    - avoid traders to open positions at manipulated fake prices from oracle
6. Liquidation (per-position cleanup)
7. Insurance fund (bad debt absorber)
    - Pool that eats bad debt
    - Means when position gets liquidated with negative equity (loss bigger than collateral), someone has to cover the gap.
    - Funded by liquidation penalties(each liquidation pays into it). Grows over time from healthy operation, drained during debt events.
    - I could split it 40/40/20 - 40% goes to the liquidator invoker(bot), 40% goes to the insurance fund, 20% goes to LP pool
8. Market pause (emergency)
    - Just a property `market_status`
9. ADL (LP solvency backstop)
    - triggered from off-chain like bots, but execution (force-closing profitable positions) is in my contract.



## What i have
1. Fees -> `utils/fee_model.rs`:
    - I have `Trade Fees` utils/fee_model.rs, paid one-time on open/close. 
    - Those fees are calculated based on `skew` the imbalance between total long and short open interest, that means we add additional fees on top of `base_fee` as a penalty for joining the market on the dominant side at the moment.

2. Funding -> `.docs/risk_mechanism/Funding.md`, `update_funding.rs`:
    - Simple implementation for funding using `max_skew`, and `cumulative_funding_index` to establish a funding_rate.
    - There is Bot that handles this for all Positions and the market index
    - The funding is handled on settlement

3. Mark price
    - I have only Oracle Price(index) and Skew (imbalance derived from OI), we have no mark, no premium, no contract price.
    - This is because we have no orderbook, no organic trading price exists. Mark price would just be a formula derived from skew. Skip the middleman — drive funding directly from skew.

4. Position/OI/skew caps
    - Implemented for my `syntetic_market.rs`

5. Oracle safety (guard)
    - TODO

6. Liquidation
    - Handled by liquidation bot

7. Insurance Fund
    - TODO

8. Market Pause
    - implemented just a `market_pause` instruction for that.

9. ADL
    - TODO