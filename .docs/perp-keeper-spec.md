# Perp Keeper — Spec

## Why

The perp program already exposes two permissionless instructions — `liquidate` and `update_funding` — but nothing calls them. Without an off-chain caller, underwater positions never get liquidated (bad debt accrues silently) and `cumulative_funding_index` never advances (funding stops correcting long/short skew). This spec defines a keeper service so third parties are incentivized to keep the protocol solvent and funding current, instead of relying on the team to trigger these by hand.

## What It Does

A single standalone Rust service, `kairos-keeper` (sibling to `kairos-indexer`/`kairos-api`), run as one long-lived binary with two independent async loops:

- **Liquidation loop** — scans open positions, computes equity off-chain, submits `liquidate` when a position breaches maintenance margin. Permissionless; the caller earns the on-chain liquidator reward.
- **Funding-tick loop** — per configured market, submits `update_funding` once `interval_seconds` has elapsed since `last_funding_time`. Permissionless, but **unrewarded on-chain today** (see Non-Goals).

Both loops share one RPC client, one keeper keypair/tx signer, and one config/logging layer.

## Requirements

### Liquidation loop

**Position discovery** — periodically enumerate all open positions for each configured market. A position opened after the loop starts must show up in a later scan without a restart.

**Off-chain equity computation** — for each position, compute equity as collateral adjusted for accrued funding, plus PnL, using a fresh oracle price — the same formula `liquidate` evaluates on-chain (equity = collateral_after_funding + pnl). Must match the on-chain result for the same inputs.

**Liquidation trigger** — submit `liquidate` when equity ≤ notional × maintenance_margin_bps / 10,000. Must not submit when equity is above that threshold.

**Permissionless operation** — any funded keypair can run the loop; no allowlist or registration. A never-before-seen keypair liquidating a genuinely breached position must succeed and receive the reward.

**Graceful race handling** — if another keeper's `liquidate` lands first for the same position, this bot's tx fails without crashing the process; the loop logs it and continues scanning the rest.

**Reward observability** — record `liquidator_reward` and `bad_debt` from the `PositionLiquidated` event on every successful liquidation.

### Funding-tick loop

**Interval-gated submission** — submit `update_funding` for a market only once `interval_seconds` has elapsed since its `last_funding_time`. Must not submit early.

**Independent multi-market coverage** — every configured market ticks on its own schedule, independent of the others.

**Permissionless, unrewarded operation** — any funded keypair can run the loop; no allowlist. A successful `update_funding` costs the tx fee and pays nothing back — this is expected behavior, not a bug (see Non-Goals).

**Graceful race handling** — if another keeper's `update_funding` lands first within the same interval, this bot's tx fails (on-chain `FundingTooEarly`) without crashing the process; the loop logs it and continues tracking the rest.

## Design

**Context.** `liquidate.rs` and `update_funding.rs` in `programs/perp` already implement both instructions as permissionless — any `Signer` can call them; nothing currently does. `Position` PDAs are seeded `[POSITION_SEED, trader, market]` — there's no on-chain index of "all open positions for market X," so discovery happens off-chain. `kairos-indexer` is the existing precedent for a standalone Rust service in this repo (tokio, `solana-sdk`, `.env` config via `dotenv`) — `kairos-keeper` follows the same conventions.

**Architecture decisions:**
- Single binary, two `tokio::spawn`ed tasks sharing one RPC client and one keeper `Keypair` — matches "handles both processes in one service," avoids funding two hot wallets.
- Position discovery via polling `getProgramAccounts` with a `market` memcmp filter, on a configurable interval — simpler than websocket `accountSubscribe`, works against any RPC provider, no subscription limits. Can switch later without changing the requirements above (they only constrain *when* a tx is submitted, not *how* positions are found).
- Off-chain equity math mirrors on-chain math exactly (`apply_funding` + `calculate_pnl` + the same MMR threshold) to avoid submitting liquidations that would fail on-chain and waste the fee.
- Config via `.env`: `RPC_URL`, keeper keypair (path or base58 secret), list of markets to watch — matching `kairos-indexer`'s pattern. No on-chain market registry lookup.
- Per-item failures (lost races, stale positions, RPC hiccups) are logged and swallowed, not fatal — one loop's failure never stops the other.

**Risks / trade-offs:**
- Polling creates a reaction-time window before price moves are noticed → already priced into `maintenance_margin_bps` sizing guidance (`programs/perp/.docs/risk_mechanism/Liquidation.md`: `MMR_bps ≈ expected_price_move_during_reaction_time × safety_multiplier`); keep poll interval well within that window.
- `getProgramAccounts` over many positions can be slow on public RPC as position count grows → filter by market via memcmp; use a dedicated RPC endpoint in production.
- Racing keepers both pay a tx fee, only one succeeds → inherent cost of permissionless keeper design, not something to prevent, just handle gracefully (see requirements above).
- Single keeper keypair funds both loops — if it runs low on SOL, both stop → log a low-balance warning.
- `update_funding` stays unrewarded, so third-party adoption of that loop specifically may be low → accepted; the team can run its own instance for that loop.

## Non-Goals

- No on-chain program changes. The bot calls `liquidate` and `update_funding` exactly as they exist today.
- No on-chain reward added to `update_funding` — funding ticks remain a free/altruistic action until a future change adds one. (Decided explicitly: keep as-is.)
- No on-chain market discovery/registry — markets to watch are supplied via config.
- No MEV/priority-fee strategy for winning liquidation races in v1 — tunable later via config without changing behavior.
- No metrics/Prometheus endpoint in v1 — structured logs only.

## Implementation Tasks

### 1. Crate scaffolding
- [ ] Create `kairos-keeper/` crate at repo root (`tokio`, `solana-sdk`, `dotenv`, `borsh`, matching `kairos-indexer`'s versions); `cargo build` succeeds with an empty `main.rs`
- [ ] Add `.env.example` (RPC URL, keeper keypair, market list), following `kairos-indexer/.env.example`'s format; app loads it without panicking

### 2. Shared infrastructure
- [ ] Config loading with validation (unit test rejects a config missing a required field)
- [ ] RPC client wrapper for `SynteticMarket`/`Position` accounts (deserializes a known devnet `SynteticMarket` correctly)
- [ ] Tx builder/signer using the keeper keypair (produces a signed, submittable tx for a stub instruction)
- [ ] Structured logging for loop health (last scan/tick time, error counts)

### 3. Liquidation loop
- [ ] Position discovery via `getProgramAccounts` + market memcmp filter (returns all open positions for a seeded devnet market)
- [ ] Off-chain equity computation matching `apply_funding`/`calculate_pnl`/the MMR threshold (unit test matches known on-chain values at multiple prices)
- [ ] `liquidate` submission for breached positions (integration test against a manually-created underwater devnet position succeeds)
- [ ] Graceful handling of failed/raced submissions (unit test simulates failure without the task exiting)
- [ ] Record `liquidator_reward`/`bad_debt` from `PositionLiquidated` on success

### 4. Funding-tick loop
- [ ] Per-market interval tracking (`last_funding_time` + `interval_seconds`) — unit test picks the correct due market among two with different intervals
- [ ] `update_funding` submission gated on interval (integration test against a short-interval devnet market succeeds once due)
- [ ] No premature submission before interval elapses (test confirms no tx sent)
- [ ] Graceful handling of failed/raced submissions (`FundingTooEarly`) without exiting

### 5. Process wiring
- [ ] Both loops as independent `tokio::spawn`ed tasks sharing one RPC client + keypair; local smoke test shows both logging activity within their intervals
- [ ] A fatal error in one loop doesn't stop the other (forced RPC error in funding loop, liquidation loop keeps scanning)

### 6. Devnet verification
- [ ] Run against the devnet perp deployment (`.docs/Deployments.md`); manually create an underwater position; bot liquidates it and logs the reward
- [ ] Bot ticks funding on a short-interval test market; `cumulative_funding_index_bps` advances on-chain as expected
