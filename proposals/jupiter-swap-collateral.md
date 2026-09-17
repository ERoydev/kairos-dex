# Proposal: Jupiter Swap for Arbitrary-Token Collateral

Status: idea / not started
Owner: Emil

## Motivation

Right now, depositing into the LP vault or posting margin on the perp program
requires the user to already hold the collateral asset (USDC). Most wallets
hold a mix of SOL and other SPL tokens. Requiring USDC up front is friction,
and it's also the point in the stack where an integration with an external
protocol (rather than more from-scratch program code) is the most valuable
thing to practice.

Goal: let a user deposit/margin with *any* SPL token, swap it to USDC via
Jupiter, and hand the resulting USDC to the existing `perp` / `liquidity-pool`
programs unchanged. Internally, nothing about the on-chain accounting
changes — this is purely a pre-processing step in front of instructions that
already exist.

## Non-goals

- No changes to `programs/perp` or `programs/liquidity-pool` account
  structures, math, or CPI flow.
- Not a general-purpose swap feature — scoped to "swap into the collateral
  asset as part of a deposit/open-position action."
- Not replacing the price oracle used for funding/liquidation — Jupiter is
  only in the deposit path, not the pricing path.

## High-level flow

```
User (wallet: BONK / JUP / SOL / ...)
        │
        │ 1. request quote: <token> → USDC, amount
        ▼
  Jupiter Quote API (v6) ──▶ route + expected USDC out + price impact
        │
        │ 2. request swap instructions for that route
        ▼
  Jupiter Swap API ──▶ serialized swap instruction(s) / versioned tx pieces
        │
        │ 3. compose: [swap ix(s)] + [existing deposit/open_position ix]
        ▼
  Single versioned transaction, one signature from user
        │
        ▼
  perp / liquidity-pool programs receive USDC exactly as they do today
```

Two composition options for step 3:

1. **Single atomic transaction** (preferred): swap instruction(s) + your
   program's `deposit`/`open_position` instruction in one versioned
   transaction. Either both succeed or both fail — no state where funds are
   swapped but never deposited.
2. **Two sequential transactions**: swap first, confirm, then deposit. Simpler
   to build, but leaves a window where the user has USDC in their wallet but
   no position — acceptable as a v1 fallback if composing instructions proves
   annoying, but the atomic version is the real target.

## Where this lives

Doesn't require new on-chain code. Candidates for where the integration
logic sits:

- A new module/route in `kairos-api` (e.g. `POST /deposit/quote`,
  `POST /deposit/build-tx`) that wraps the Jupiter Quote/Swap calls and
  returns a ready-to-sign transaction to the frontend. Keeps `kairos-api`'s
  "read-only" framing slightly stretched, so may want a new small service
  instead (`kairos-swap-gateway` or similar) if that framing matters.
- Alternatively, do it client-side only (frontend calls Jupiter directly,
  builds the combined tx, sends to `perp` program) — no new backend service
  at all, at the cost of putting Jupiter API keys/rate-limit handling in the
  frontend.

Recommendation when implementing: start client-side (fewer moving parts to
practice the integration itself), move server-side later if you want to
practice building a backend service around a third-party API.

## Key risks / things to get right

- **Slippage / stale quotes**: quote is fetched, then some time passes before
  the tx lands. Need a slippage tolerance (`slippageBps`) and a max
  acceptable price-impact check before building the tx, otherwise a moved
  market can eat the user's deposit.
- **Atomicity**: if using two sequential transactions, a failure between them
  leaves USDC sitting in the user's wallet with no position opened — needs to
  be communicated clearly in the UI, not a silent bad state.
- **Transaction size**: composing a Jupiter swap route (which can span
  multiple hops/AMMs) with your own instruction may hit the transaction size
  limit. Jupiter's API supports requesting simpler/fewer-hop routes
  (`onlyDirectRoutes` or route restriction) as a fallback.
- **Failure UX**: Jupiter route not found, quote expired, insufficient
  liquidity for the requested size — all need explicit handling and a clear
  error back to the user rather than a generic failure.
- **Idempotency**: if the swap+deposit tx fails after the swap leg lands but
  before program logic (only relevant in the two-tx design), a retry path is
  needed so USDC doesn't get stranded.

## Implementation sketch (when picked up)

1. Prototype against Jupiter's Quote + Swap API on devnet/mainnet-fork with a
   throwaway wallet, no program involved yet — confirm you can get a route
   and a working swap tx for `SOL → USDC`.
2. Confirm whether Jupiter's swap instructions can be merged into a single
   versioned transaction alongside an arbitrary custom instruction (check
   current API docs for "swap instructions" vs "swap transaction" endpoints —
   the instructions endpoint is the one designed for composability).
3. Wire the combined tx to call your existing `deposit` (LP) or
   `open_position` (perp) instruction with the resulting USDC amount as
   input.
4. Add slippage/price-impact guardrails before building the tx.
5. Add UI: token picker, quote preview (expected USDC out, price impact,
   slippage setting), single "Deposit" button that does swap+deposit in one
   signature.
6. Test with at least one low-liquidity token to exercise the "no route" /
   "high price impact" failure paths, not just SOL → USDC happy path.

## Open questions

- Devnet Jupiter support/liquidity is thin — may need to test primarily
  against a mainnet fork (surfpool?) rather than devnet.
- Whether to expose this only for LP deposits, only for opening perp
  positions, or both — perp margin top-ups might want the same flow later.
