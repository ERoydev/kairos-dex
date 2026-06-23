# Solana Perpetuals DEX — Project Plan

## What We're Building

A **Perpetuals Trading Exchange** on Solana where users can trade crypto with leverage
(long/short positions), similar to Drift Protocol or GMX. The goal is a production-grade
portfolio project that showcases Rust, Solana, and microservices architecture skills.

---

## Why This Project

- Mirrors real, recognizable protocols (Drift, GMX, Mango) — recruiters know what it is
- Naturally decomposes into microservices — not forced, each service has a real purpose
- Covers Rust async systems, DB design, WebSockets, and blockchain in one project
- Each service is independently deployable (Docker)
- Fully buildable on free tiers — no mainnet or paid infra needed

---

## Architecture Overview

```
┌─────────────────────────────────────────────────────┐
│                  Solana Program (Anchor)              │
│  positions · collateral · settlement · funding rate  │
└──────────────────────┬──────────────────────────────┘
                       │
        ┌──────────────┼──────────────┐
        ▼              ▼              ▼
┌──────────────┐ ┌──────────────┐ ┌──────────────┐
│ Oracle Svc   │ │ Indexer Svc  │ │ Liquidator   │
│              │ │              │ │ Bot          │
│ Pyth devnet  │ │ Helius hooks │ │              │
│ → cranks     │ │ → Postgres   │ │ Monitors     │
│   price feed │ │   events DB  │ │ undercollat. │
└──────────────┘ └──────┬───────┘ └──────────────┘
                        │
              ┌─────────┼─────────┐
              ▼         ▼         ▼
        ┌──────────┐ ┌──────┐ ┌──────────────┐
        │ REST API │ │  WS  │ │ Risk Engine  │
        │ (Axum)   │ │Server│ │              │
        │          │ │      │ │ funding rate │
        │ positions│ │prices│ │ OI limits    │
        │ history  │ │ P&L  │ │ fee calc     │
        └──────────┘ └──────┘ └──────────────┘
                        │
                   ┌────▼────┐
                   │Frontend │
                   │Next.js  │
                   └─────────┘
```

---

## Microservices Breakdown

| Service | Language | Responsibility |
|---|---|---|
| **Anchor Program** | Rust | On-chain logic: positions, collateral, liquidation, funding |
| **Oracle Service** | Rust | Pull Pyth price feeds, crank them on-chain |
| **Indexer** | Rust | Receive Helius webhooks, parse events, write to Postgres |
| **Liquidator Bot** | Rust | Monitor positions, call liquidation instruction when undercollateralized |
| **Risk Engine** | Rust | Compute funding rates, open interest caps, fee tiers |
| **REST API** | Rust (Axum) | Serve position history, account data, leaderboard |
| **WebSocket Server** | Rust (Axum/tokio-tungstenite) | Stream real-time prices and P&L to frontend |
| **Frontend** | Next.js + TypeScript | Wallet adapter, trading UI, TradingView charts |

---

## Core On-Chain Mechanics

### Accounts (PDAs)
- `Market` — global state per trading pair (SOL-PERP, BTC-PERP)
- `Position` — per user per market (size, entry price, collateral, side)
- `Vault` — holds collateral (USDC)

### Instructions
- `initialize_market` — admin sets up a new trading pair
- `deposit_collateral` — user deposits USDC into their position vault
- `open_position` — user opens long/short with leverage
- `close_position` — user closes and settles PnL
- `liquidate` — anyone calls this on an undercollateralized position (earns a fee)
- `update_funding` — cranked periodically to settle funding between longs/shorts

### Key Concepts to Implement
- **Mark price vs index price** (Pyth feed)
- **Funding rate** — longs pay shorts or vice versa based on skew
- **Leverage limit** — e.g., max 10x
- **Liquidation threshold** — e.g., margin ratio below 5%
- **Liquidation fee** — incentive for liquidator bot

---

## Database Schema (Postgres)

```sql
-- Markets
CREATE TABLE markets (
  id TEXT PRIMARY KEY,           -- "SOL-PERP"
  base_mint TEXT NOT NULL,
  quote_mint TEXT NOT NULL,
  created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Positions (synced from indexer)
CREATE TABLE positions (
  pubkey TEXT PRIMARY KEY,
  owner TEXT NOT NULL,
  market TEXT NOT NULL,
  side TEXT NOT NULL,            -- "long" | "short"
  size NUMERIC NOT NULL,
  collateral NUMERIC NOT NULL,
  entry_price NUMERIC NOT NULL,
  liquidation_price NUMERIC NOT NULL,
  opened_at TIMESTAMPTZ NOT NULL,
  closed_at TIMESTAMPTZ,
  realized_pnl NUMERIC
);

-- Price history (from oracle service)
CREATE TABLE price_history (
  id BIGSERIAL PRIMARY KEY,
  market TEXT NOT NULL,
  price NUMERIC NOT NULL,
  timestamp TIMESTAMPTZ NOT NULL
);

-- Events (raw from indexer)
CREATE TABLE events (
  signature TEXT PRIMARY KEY,
  event_type TEXT NOT NULL,      -- "OpenPosition" | "Liquidate" etc.
  data JSONB NOT NULL,
  slot BIGINT NOT NULL,
  timestamp TIMESTAMPTZ NOT NULL
);
```

---

## API Endpoints (REST)

```
GET  /markets                        — list all markets
GET  /markets/:id/price              — current price + 24h change
GET  /markets/:id/positions          — all open positions for a market
GET  /accounts/:pubkey/positions     — all positions for a wallet
GET  /accounts/:pubkey/history       — closed positions + realized PnL
GET  /leaderboard                    — top traders by PnL
GET  /health                         — service health check
```

---

## WebSocket Events

```
subscribe: { type: "price", market: "SOL-PERP" }
subscribe: { type: "positions", market: "SOL-PERP" }
subscribe: { type: "account", pubkey: "..." }

emits:
  { type: "price_update", market, price, timestamp }
  { type: "position_opened", position }
  { type: "position_closed", position, pnl }
  { type: "liquidation", position, liquidator }
```

---

## Tech Stack

| Layer | Technology |
|---|---|
| Smart contract | Rust + Anchor |
| Off-chain services | Rust (tokio, Axum, sqlx) |
| Database | Postgres |
| Frontend | Next.js, TypeScript, TailwindCSS |
| Wallet | Solana Wallet Adapter |
| Charts | TradingView Lightweight Charts |
| Containerization | Docker + docker-compose |
| CI/CD | GitHub Actions |

---

## Free Infrastructure

| Service | Provider | Free Tier |
|---|---|---|
| Solana network | Devnet | Free + airdrop |
| RPC + Webhooks | Helius | 100k credits/month |
| Price oracle | Pyth devnet | Free |
| Postgres | Neon | 500MB free |
| Backend hosting | Railway or Render | Free tier |
| Frontend hosting | Vercel | Free |
| CI/CD | GitHub Actions | 2000 min/month |

---

## Build Order

### Phase 1 — Foundation
- [ ] Anchor program: `initialize_market`, `deposit_collateral`, `open_position`, `close_position`
- [ ] Anchor tests with `bankrun`
- [ ] Postgres schema + migrations

### Phase 2 — Off-chain Core
- [ ] Indexer service (Helius webhooks → Postgres)
- [ ] REST API (Axum) serving positions and history
- [ ] Basic Next.js frontend with wallet adapter

### Phase 3 — Live Data
- [ ] Oracle service (Pyth devnet → on-chain crank)
- [ ] WebSocket server (real-time prices + positions)
- [ ] TradingView chart in frontend

### Phase 4 — Advanced Mechanics
- [ ] Liquidator bot (monitor + call liquidate instruction)
- [ ] Risk engine (funding rate calculation + crank)
- [ ] Liquidation price display in frontend

### Phase 5 — Production Polish
- [ ] Docker + docker-compose for all services
- [ ] GitHub Actions CI (build, test, lint)
- [ ] Devnet deployment + live demo URL
- [ ] README with architecture diagram

---

## Folder Structure

```
solana-perps-dex/
├── program/                  # Anchor smart contract
│   ├── src/
│   │   ├── lib.rs
│   │   ├── instructions/
│   │   │   ├── initialize_market.rs
│   │   │   ├── open_position.rs
│   │   │   ├── close_position.rs
│   │   │   ├── liquidate.rs
│   │   │   └── update_funding.rs
│   │   └── state/
│   │       ├── market.rs
│   │       ├── position.rs
│   │       └── vault.rs
│   └── tests/
├── services/
│   ├── indexer/              # Rust — Helius webhooks → Postgres
│   ├── oracle/               # Rust — Pyth feeds → on-chain
│   ├── liquidator/           # Rust — monitors + liquidates
│   ├── risk-engine/          # Rust — funding rates + OI
│   ├── api/                  # Rust (Axum) — REST API
│   └── ws/                   # Rust (Axum) — WebSocket server
├── frontend/                 # Next.js
│   ├── app/
│   ├── components/
│   └── lib/
├── db/
│   └── migrations/
├── docker-compose.yml
├── .github/
│   └── workflows/
│       └── ci.yml
└── README.md
```

---

## What This Demonstrates to Recruiters

- **Rust** — smart contracts, async microservices, Axum APIs
- **Solana** — Anchor framework, PDAs, CPIs, account model
- **Systems thinking** — decomposed a complex domain into clean services
- **Database design** — event sourcing pattern, indexed queries
- **Real-time systems** — WebSocket streaming
- **DeFi mechanics** — funding rates, liquidations, leverage — domain knowledge
- **DevOps** — Docker, CI/CD, multi-service orchestration
- **Frontend** — Web3 wallet integration, real-time UI
