# Liquidation

## Formulate to calculate Liquidation Price

C_usdc + pnl - f < m 

C_usdc = Collateral
pnl
f = is not the trade fee, instead it refers to `funding`
m = minimum USDC value for a position, so called maintenance margin -> MMR


## Maintenance margin ratio -> MMR
- The minimum % of position size you must keep as equity to stay alive (no liquidated).

Set per market. If BTC-PERP has MMR = 2.5%, then on a $1000 position you need at least $25 of equity at all times. Drop below → liquidated.