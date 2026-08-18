# SynteticMarket

We call it syntetic, because the market doesn't hold the actual asset. There's no real BTC in BTC-PERP market for example. Users just bet on BTC's price, collateralized in USDC. The "BTC" exposure is synthesized from oracle prices and math.

## Contrast with:
- Spot market = holds real BTC.
- Synthetic market = holds only USDC, references BTC's price.


## Vaults
SynteticMarket owns two PDA vaults (derived on-demand, not stored):

1. Market vault:    seeds = [MARKET_VAULT_SEED, market.key()]
    - Stores the traders collateral, that he have created `Position` for.
2. Insurance fund vault: seeds = [INSURANCE_VAULT_SEED, market.key()]
    - Bad debt mechanism used when bot keeper was to slow to call `liquidate` for a position and we have equity lower than `0`, we need to give reward for the bot keeper from this fund.
    - This fund grows on liquidation, 50% of penalty goes to it, currently.