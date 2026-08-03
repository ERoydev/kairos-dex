# LP - Perp exchange counterparty

The LP pool is the house in a perp exchange. Every time a trader opens a position, they're effectively trading against the pool. So:
    - Trader loses → the pool collects their loss as profit → credit the pool (money flows IN)
    - Trader wins → the pool pays out their profit → debit the pool (money flows OUT)

These two instructions are called by the perp program (not by traders directly) as CPIs when a position is settled or liquidated. The perp program calculates the PnL, then calls either:
    - credit(amount) — trader was liquidated / hit stop loss, pool takes the funds
    - debit(amount) — trader closed in profit, pool sends them USDC

The LP depositors are the passive counterparty — they collect fees and trader losses, but they also absorb trader profits. That's why the LP share price fluctuates based on how traders perform overall.


1. Credit happens at any time the trader loses money:
    - Position closed at a loss
    - Liquidation
    - Funding rate payments (if trader is on the losing side)
All of these result in USDC flowing into the pool → credit.

2. debit is the mirror — any time the trader makes money, the pool pays them out.

The perp program is the one that knows the PnL, so it decides which to call and with what amount. The LP pool just blindly accepts or sends USDC when the authorized caller (perp program) tells it to.