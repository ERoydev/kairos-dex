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

### Dev database helpers

```bash
cargo db-fresh   # drop all tables and reapply migrations (clean slate)
cargo db-seed    # insert mock markets/positions so the keeper bot has something to scan
```

Both are aliases defined in `.cargo/config.toml`; `db-seed` runs `src/bin/seed.rs`, which seeds a few markets (some overdue for funding, some not) and a couple of open positions on the first one. `db-seed` clears its own tables before inserting, so it's safe to rerun on its own — no need to `db-fresh` in between.

## Related services

- `kairos-api` — reads from the same Postgres DB
- `kairos-perp-program` / `kairos-lp-pool-program` — source of the events

- `kairos-postgres` - A docker container i have locally for now, start it then connect to it with Dbeaver to inspect data. It also have persistent data with docker volumes.