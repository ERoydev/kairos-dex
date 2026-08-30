# Start on clear

1. solana-keygen new -o target/deploy/perp-keypair.json --force
2. anchor keys sync                     (updates declare_id! in lib.rs + Anchor.toml)
3. anchor build
4. surfpool run deployment --env devnet
5. update PERP program ID in your indexer + scripts/ts/common.ts
6. re-run: initialize_global → initialize_market → ...   (fresh PDAs, empty state)


# Workflow

```bash
NO_DNA=1 node_modules/.bin/tsx scripts/ts/invoke_initialize_global.ts
NO_DNA=1 node_modules/.bin/tsx scripts/ts/07b_set_fee_receiver.ts
NO_DNA=1 node_modules/.bin/tsx scripts/ts/02_init_lp_pool.ts
NO_DNA=1 node_modules/.bin/tsx scripts/ts/03_deposit_lp.ts
NO_DNA=1 node_modules/.bin/tsx scripts/ts/05_initialize_market.ts
NO_DNA=1 node_modules/.bin/tsx scripts/ts/06_market_pause.ts   
NO_DNA=1 node_modules/.bin/tsx scripts/ts/07_update_caps.ts    
NO_DNA=1 node_modules/.bin/tsx scripts/ts/10_trade_flow.ts     
```