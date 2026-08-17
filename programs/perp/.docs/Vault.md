# Market Vault

- When the Trader Position is open, the margin sits in `market_vault` as escrow/collateral. The LP pool is the actual counterparty to the trade, it's on the other side, exposed to the position's unrealized PnL the whole time it's open, its just liability the pool is carrying on paper.


## Settlement
The transfer happens at close (close_position), and it's driven by PnL, not simply "loss → pool":
- Trader profitable → trader gets margin back + profit, and that profit has to come from the LP pool (pool → trader).
- Trader at a loss → trader gets margin back minus the loss, and that lost portion moves from the vault into the pool (vault → pool).

## Why open_position deposits margin + lp_fees, not just margin
`credit_lp_pool`'s CPI (liquidity-pool's `credit` ix) only accepts a signer matching `pool.perp_program`, which is set to `market_vault`'s address — so the token account paying the LP its fee cut must be owned by `market_vault`, not the trader. lp_fees has to land in the vault before it can be forwarded to the pool.

If the vault only received `margin` and then forwarded `lp_fees` back out, it'd end up holding `margin - lp_fees` while `position.collateral` still records the full `margin` — an under-collateralized vault that fails to pay traders back at close. So `open_position` deposits `margin + lp_fees`, forwards `lp_fees` to the pool, and is left holding exactly `margin`.