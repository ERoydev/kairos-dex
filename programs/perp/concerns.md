
# Concerns about pepr + LP pool as counterparty

Trader wants to create position:
    - he creates long position (he wants to profit from asset price increase)
    - he creates short position (he wants to profit from assed price decrease)
    - my or may not create take/stop risk management option (this can influence again whether i can or cannot aggree to allow him to take a position)

The problem here is that, before this trader takes position my pool already has plenty of other trades inside that can be profitable or losable at the moment. I have to implement a mechanism to allow the trader to take a position ONLY if the outcome can be covered by the funds in the LP pool.

The question is how to implement this, because if i allow him to take that position and it coverable at the moment, after 2 days for example other traders can profit enormously making my pool unable to pay to this trader position that we have approved 2 days ago.

---

## Risk management tools
Typical mechanisms are:

1. Max open interest
Don't allow unlimited exposure.
Example: max BTC long OI = $20M.

2. Max leverage
Prevent positions that are too risky.
Liquidity utilization limits
Don't let total exposure become too large relative to LP liquidity.

4. Borrow/funding fees
As risk increases, fees increase, discouraging further imbalance.
Per-market caps
Limit longs and shorts independently.

5. Liquidations
Close losing positions before they create bad debt.