# What each module contains

## src/main.rs
- Loads config
- Creates the Postgres pool
- Runs pending migrations
- Starts the stream subscriber loop
- Wires: subscriber -> parser -> dispatcher -> db
- Top-level error handling / restart

## src/config.rs
- Struct `Config { rpc_ws_url, program_id, database_url, start_slot }`
- Loads from env (.env)
- Validates required vars are present

## src/stream/subscriber.rs
- Opens WebSocket connection to Solana RPC
- Sends `logsSubscribe` for your program id
- Yields raw log messages (a stream/channel of `RawLog { signature, slot, logs: Vec<String> }`)
- Reconnect + backoff logic on disconnect
- Reads last checkpoint (slot/signature) on startup to resume

## src/parser/decoder.rs
- Takes a `RawLog`
- Matches Anchor event discriminators / parses `Program data:` log lines
- Decodes bytes via Anchor IDL types
- Returns a typed `Event` (from events.rs) or `None` if irrelevant

## src/events.rs
- `enum Event { PositionOpened(..), PositionClosed(..), Liquidation(..), FundingUpdate(..) }`
- Each variant holds the decoded fields (owner, market, size, price, pnl, etc.)
- Shared contract between parser (produces) and handlers (consume)

## src/handlers/mod.rs
- `dispatch(event: Event, db: &Db)` — match on event, call the right handler

## src/handlers/position_opened.rs
- Insert a new row into `positions` (status = open)
- Fields: owner, market, side, size, entry_price, margin, opened_at, tx_sig

## src/handlers/position_closed.rs
- Update the matching `positions` row (status = closed, exit_price, pnl, closed_at)

## src/handlers/liquidation.rs
- Update the matching `positions` row (status = liquidated)
- Optionally insert into a `liquidations` table (liquidator, remaining_margin, tx_sig)

## src/handlers/funding_update.rs
- Insert into `funding_history` (market, funding_rate, cumulative_index, timestamp)

## src/db/pool.rs
- `Db` struct wrapping `sqlx::PgPool`
- `Db::connect(url)` constructor

## src/db/models.rs
- Rust structs mirroring each table (`PositionRow`, `LiquidationRow`, `FundingRow`)
- `#[derive(sqlx::FromRow)]`

## src/db/queries.rs
- The actual SQL: `insert_position`, `close_position`, `mark_liquidated`, `insert_funding`
- Also: `save_checkpoint(slot)` / `load_checkpoint()` for resume-after-restart

## migrations/*.sql
- `CREATE TABLE positions (...)`, `liquidations`, `funding_history`, `indexer_checkpoint`
