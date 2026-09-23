# Ingestor Implementation Plan

## Purpose
Watch on-chain events for N Solana programs and push them as tasks to Redis for downstream indexers.

## Architecture

```
Solana RPC ──▶ Ingestor ──▶ Redis Stream ──▶ Indexers ──▶ Postgres
                                                    │
                                                    └─▶ writes cursor
```

## Components

| Component | Responsibility |
|---|---|
| **Cursor Manager** | Load cursor rows from Postgres on startup |
| **Catch-up Worker** | Fill the gap between last cursor and now via RPC |
| **Stream Subscriber** | Live WebSocket `logsSubscribe` per program |
| **Queue Publisher** | `XADD` signatures to Redis stream |

## Cursor Table

```sql
CREATE TABLE ingestor_cursor (
  program_id      TEXT PRIMARY KEY,
  last_signature  TEXT NOT NULL,
  last_slot       BIGINT NOT NULL,
  updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

One row per tracked program (`perp`, `lp`, etc.).

## Cursor Ownership

- **Ingestor**: reads cursor on startup only.
- **Indexer**: writes cursor after successful DB insert.

Cursor advances **only forward** (`WHERE last_slot < EXCLUDED.last_slot`).

## Redis Message Shape

```
XADD events:raw * program_id <perp|lp> signature <sig> slot <slot>
```

Include `program_id` so indexers can dispatch to the right parser.

## Startup Flow

```
1. Load cursors for all tracked programs from Postgres
2. For each program:
     if cursor exists:
         sigs = getSignaturesForAddress(program, until=cursor.last_signature)
     else:
         apply first-run policy (start-from-now recommended)
     publish each sig to Redis (oldest first)
3. Open WebSocket, logsSubscribe per program
4. On each new log → publish to Redis (indexer will advance cursor)
```

## Runtime Flow

```
loop forever:
   receive log notification
   XADD {program_id, signature, slot} to Redis
```

That's it — no cursor updates, no DB writes. Ingestor stays thin.

## Guarantees

- **No gaps**: catch-up covers any downtime before streaming starts.
- **No lost events**: cursor advances only after indexer's DB write succeeds.
- **At-least-once delivery**: duplicates possible; indexer handles via `ON CONFLICT DO NOTHING`.
- **Parallel-safe**: multiple indexers via Redis consumer group; cursor uses forward-only advance.

## Tech Stack

- Language: **Rust** (Tokio async runtime)
- Crates: `solana-client`, `solana-sdk`, `tokio`, `redis`, `sqlx`, `serde`
- Config: list of program IDs, RPC URL, WS URL, Redis URL, Postgres URL

## First-Run Policy

Recommended: **start from now** (`cursor = current slot`, no backfill). Add historical backfill later only if needed.

## Failure Modes

| Failure | Recovery |
|---|---|
| Ingestor crash | Restart → catch-up fills gap from cursor |
| WebSocket disconnect | Reconnect + re-run catch-up |
| Indexer crash mid-processing | Redis redelivers unacked message; cursor unchanged |
| Redis flushed | Ingestor restart → catch-up rebuilds queue from cursor |
| Postgres down | Ingestor still buffers to Redis; indexers retry writes |