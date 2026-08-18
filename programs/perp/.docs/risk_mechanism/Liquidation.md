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

```
`Equity` = collateral + PnL (signed — subtracts when PnL is negative, adds when positive).
equity = collateral + PnL - accrued_funding
```

```
Example: $100 collateral, 5x leverage, $500 LONG on ETH at $2000.

ETH price	pnl	                                equity ($100 + pnl)	  Status
$2000	    $500 × (2000-2000)/2000 =  $0	    $100	              Fine
$1950	    $500 × (1950-2000)/2000 = -$12.50	$87.50	              Fine
$1800	    $500 × (1800-2000)/2000 = -$50	    $50	                  Fine
$1650	    $500 × (1650-2000)/2000 = -$87.50	$12.50	              At threshold
$1649	    $500 × (1649-2000)/2000 = -$87.75	$12.25	              LIQUIDATED
```

Generally a good formula to set this property is -> `MMR_bps ≈ expected_price_move_during_liquidation × safety_multiplier`
- expected_price_move = how much price can move in the time it takes keepers to react (in bps).
- safety_multiplier = 1.5–2× for headroom.

```
maintenance_margin_bps: 250,  // 2.5% — good default for BTC/ETH
```

## Workflow

1. Bot calls liquidate (permissionless)
2. Inside liquidation i do:
    - Compute funding owed or owes `accrued_funding`
    - Compute pnl from oracle price
    - Compute equity
    - Compute threshold = notional * mmr_bps / 10_000
    - Reject if equity >= threshold (position is healthy), compute the equity with the mmr, to validate that this position is for liquidation, so if mmr is 2.5% that means equity must be at least 2.5% of the whole notional size.

3. If equity > 0 (normal liquidation):
    - Compute penalty
    - Split penalty: 50% liquidator, 50% insurance fund
    - Remaining equity -> LP pool

4. If equity <= 0 (bad debt):
// Note -> This can happen when the bot keeper was to slow, so there is nothing left for penalty.

    - insurance fund covers the negative gap -> LP pool made whole
    - Small keeper reward paid from insurance fund

    - We pay 0.10% of notional, paid from the insurance fund when there's no equity left to split as a penalty

5. Update market OI (subtract this position's notional from the correct side)

6. Close Position account, refund rent to owner

## Fees

The plan is to divide them into 50/50 -> 50% goes to the `keeper` the bot who called correct liquidate and 50% for `insurance fund`