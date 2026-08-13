# Market Vault

- When the Trader Position is open, the margin sits in `market_vault` as escrow/collateral. The LP pool is the actual counterparty to the trade, it's on the other side, exposed to the position's unrealized PnL the whole time it's open, its just liability the pool is carrying on paper.


## Settlement
The transfer happens at close (close_position, whenever you build it), and it's driven by PnL, not simply "loss → pool":
- Trader profitable → trader gets margin back + profit, and that profit has to come from the LP pool (pool → trader).
- Trader at a loss → trader gets margin back minus the loss, and that lost portion moves from the vault into the pool (vault → pool).