# kairos-api

Read-only backend service for Kairos DEX. Serves indexed/historical data to the frontend — position history, PnL, pool stats, deposit/withdraw history.

**This service does not execute trades or move funds.** All fund-moving actions (open/close position, deposit/withdraw, liquidate) happen directly on-chain via the Perp Program and LP Pool Program, signed by the user's wallet. `kairos-api` only reads from Postgres, which is kept up to date by the `kairos-indexer` service.

## What this service is for

- Position history (open, closed, liquidated)
- LP deposit/withdraw history
- Pool stats (TVL, share price over time)
- Aggregated/derived data too expensive to query on-chain directly

## What this service is NOT for

- Executing transactions (trading, depositing, liquidating) — those go directly from the frontend to the Solana programs
- Source of truth for current on-chain state — Postgres here is a mirror, not authoritative
- Live/authoritative pricing — price used for actual trades is read on-chain by the Perp Program itself, not served here

## Architecture

```
Frontend → kairos-api → Postgres (read-only)
                            ↑
                      kairos-indexer (writes, listens to on-chain events)
```

## Stack

- Rust
- Axum (web framework)
- sqlx (Postgres client, compile-time checked queries)
- Tokio (async runtime)

## Running locally

```bash
cp .env.example .env   # set DATABASE_URL
cargo run
```

## Endpoints

| Method | Path | Description |
|---|---|---|
| GET | `/positions/:owner` | Position history for a wallet |
| GET | `/pools/:market` | Pool stats for a market |
| GET | `/pools/:market/history` | Share price / TVL over time |
| GET | `/deposits/:owner` | LP deposit/withdraw history |
| GET | `/stats` | Global platform stats |

## Related services

- `kairos-indexer` — writes to the same Postgres DB
- `kairos-perp-program` — on-chain Perp contract
- `kairos-lp-pool-program` — on-chain LP vault contract