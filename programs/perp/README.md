# Perp Program 

# Run tests

Automated (recommended) — builds both programs, starts surfpool, deploys
liquidity-pool into it, runs `anchor test`, then tears surfpool down again:

cd programs/perp
make integration-test

# or directly: ./test-integration.sh

Manual — same flow as above but run by hand, useful if you want the runbook
supervisor UI (localhost:8488) open to review transactions before they're
signed and broadcast:

cd programs/perp
anchor test

# Devnet deployment

## Clean stale program, if you want to reseted during development

Fresh program ID so init-global / init-market run against empty state.

```bash
cd programs/perp
solana program close <OLD_PROGRAM_ID> -ud --bypass-warning
rm target/deploy/perp-keypair.json   # keys sync alone won't mint a new ID
anchor build && anchor keys sync && anchor build
```

Then `grep -rn "<OLD_PROGRAM_ID>" .` from repo root and fix the leftovers
(`deployed_programs.json`, runbook inputs, keeper/indexer/api configs).

Closed IDs are dead forever, and the old PDAs keep their rent — nothing can
sign to close them once the program is gone.

## Deploy

```bash
anchor build
surfpool run deployment --env devnet -u
```

# Troubleshooting

Have in mind that `liquidity-pool` is a dependency of perp, so that means you have to first deploy liquidity pool and make sure everything is set correctly, then build that in `perp` and deploy it. After those steps you can go with initialization, since they depend on account and program ID's constaints that may return confusing errors if something is messed up.