# Perp Program 


surfpool run deployment is the right tool here — it doesn't start a validator, it just executes a runbook's deploy actions against whatever RPC endpoint you point it at (127.0.0.1:8899, same as your already-running surfnet). So the flow is:

# terminal 1 — start the one validator, auto-deploys perp
cd programs/perp
surfpool start

# terminal 2 — deploy liquidity-pool into that SAME running surfnet
cd programs/liquidity-pool
anchor build
surfpool run deployment --env localnet

Both programs end up live on the one surfnet. If you rebuild liquidi run deployment --env localnet again to redeploy — there's no --watch
surfpool run deployment --env localnet

Both programs end up live on the one surfnet. If you rebuild liquidi run deployment --env localnet again to redeploy — there's no --watch equivalent across two separate txtx.yml projects targeting the same validator.

# Run tests

cd programs/perp
anchor test