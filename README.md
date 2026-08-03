# kairos-indexer

Write-only background service for Kairos DEX. Listens for on-chain events from the Perp Program and LP Pool Program (via Helius webhooks) and mirrors them into Postgres.

**This service does not serve the frontend.** Reads are handled by `kairos-api`. This service only writes.

## What it does

- Exposes a webhook endpoint that Helius calls when a matching transaction occurs on our programs
- Decodes the event/instruction data
- Writes/updates rows in Postgres (positions, deposits, withdrawals, liquidations)

## What it does NOT do

- Serve any public API — no reads, no frontend traffic
- Trigger or execute anything on-chain — purely passive, reacts after the fact
- Act as source of truth — Postgres here is a mirror of on-chain state, not authoritative

## Architecture

```
Perp Program / LP Pool Program → tx on-chain
        ↓
Helius watches program addresses
        ↓
Helius webhook → kairos-indexer
        ↓
Decode → write to Postgres
```

## Stack

- Rust, Axum (single webhook route), Tokio, sqlx, serde

## Running locally

```bash
cp .env.example .env   # DATABASE_URL, HELIUS_WEBHOOK_SECRET
cargo run
```

## Related services

- `kairos-api` — reads from the same Postgres DB
- `kairos-perp-program` / `kairos-lp-pool-program` — source of the events