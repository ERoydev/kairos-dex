# Kairos DEX

A perpetuals exchange on Solana: traders take leveraged long/short positions against a shared LP vault, which acts as the counterparty (the "house").

## Components

| Component | Role |
|---|---|
| `programs/perp` | On-chain perp program — markets, positions, funding, liquidation |
| `programs/liquidity-pool` | On-chain LP vault — holds depositor funds, is credited/debited by the perp program via CPI |
| `kairos-indexer` | Write-only service: listens to on-chain events (via Helius webhooks) and mirrors them into Postgres |
| `kairos-api` | Read-only service: serves indexed data (position history, PnL, pool stats) to the frontend |
| `kairos-keeper` | Permissionless bot: calls `liquidate` and `update_funding` so underwater positions get closed and funding keeps advancing |
| `kairos-db` | Postgres schema/migrations shared by the indexer and api |
| `kairos-rpc` | Shared Rust RPC client helpers |

## How it fits together

```
Trader / LP wallet
        │  (sign & send directly)
        ▼
Perp Program ──CPI──▶ LP Pool Program
        │
        │ on-chain events
        ▼
     Helius webhook
        │
        ▼
  kairos-indexer ──writes──▶ Postgres ◀──reads── kairos-api ──▶ Frontend
        ▲
        │
  kairos-keeper (liquidate / update_funding, polls the programs directly)
```

Trading, depositing, and liquidating all happen directly on-chain, signed by the caller's wallet — `kairos-api`/`kairos-indexer`/Postgres are a read mirror, never the source of truth.

## Docs & diagrams

- [`.docs/`](.docs) — deployment notes, keeper spec (`perp-keeper-spec.md`), keeper architecture diagram (`perp-keeper.png`)
- [`.research/diagrams/kairos/`](.research/diagrams/kairos) — C4 diagrams (open with [excalidraw.com](https://excalidraw.com)):
  - `c4_container_context.excalidraw` — system context / container view
  - `c4_perp_component.excalidraw` — perp program component breakdown
  - `c4_indexer_component.excalidraw` — indexer component breakdown
  - `c4_keeper_component.excalidraw` — keeper component breakdown
  - `trader.excalidraw` / `lp_provider.excalidraw` — user flows
- Per-component READMEs and `.docs/` folders under `programs/perp`, `programs/liquidity-pool`, `kairos-indexer`, `kairos-keeper`, `kairos-api` for deeper detail
- [`.research/`](.research) — background research (e.g. GMX v2 study)

## Running things

Each service has its own README with setup/run instructions. Typical order for local dev:

1. Build & deploy `programs/liquidity-pool`, then `programs/perp` (perp depends on the LP pool's program ID/accounts — see `programs/perp/README.md` troubleshooting section)
2. Start Postgres, run `kairos-indexer` (writer) and `kairos-api` (reader)
3. Run `kairos-keeper` against the deployed programs
