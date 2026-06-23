# Perpetuals DEX — Research Phase

## Goal

Understand what a perpetuals DEX is, how it works financially and mechanically,
and how existing protocols implement it on-chain — before writing any code.

---

## Step 1 — Understand Perpetual Futures (The Financial Concept)

**What to search:** "perpetual futures explained" on Investopedia

**What to learn:**
- How perps differ from spot trading and traditional futures
- What leverage is and how it amplifies gains/losses
- What margin and collateral mean
- What liquidation is and when it happens
- What funding rate is and why it exists

**Questions to answer before moving on:**
- [ ] If I open a 10x long on SOL at $100, what happens if SOL drops to $91?
- [ ] What is the difference between isolated margin and cross margin?
- [ ] Why do longs pay shorts (or vice versa) in a funding rate?

**Time estimate:** 1–2 hours

---

## Step 2 — Understand How DEXs Implement Perps

Read docs/whitepapers of existing protocols. Don't read everything — focus on
architecture and mechanics.

### GMX (search: "GMX V2 docs")
- Simpler model, good starting point
- Focus on: how the GLP liquidity pool works, how positions are opened/closed,
  how the protocol acts as counterparty

### Drift Protocol (search: "Drift Protocol docs")
- Solana-native — closest to what you're building
- Focus on: their vAMM design, how funding rate is calculated, account structure

### dYdX (search: "dYdX documentation")
- More complex, but very well documented
- Focus on: order book model vs AMM model — understand the tradeoff

**Questions to answer before moving on:**
- [ ] Where does the counterparty come from when a user opens a long?
- [ ] How does the protocol stay solvent if many positions are underwater?
- [ ] What is the difference between a vAMM and an order book perp DEX?

**Time estimate:** 2–4 hours

---

## Step 3 — Understand the Core Math

You don't need to derive everything, but you should understand these formulas.

### PnL Calculation
```
Long PnL  = (current_price - entry_price) * size
Short PnL = (entry_price - current_price) * size
```

### Leverage
```
leverage = position_size / collateral
```

### Liquidation Price (Long)
```
liquidation_price = entry_price * (1 - 1/leverage + maintenance_margin)
```

### Funding Rate
```
funding_rate = (mark_price - index_price) / index_price
```
Paid every N hours. Longs pay shorts when mark > index (market is overbought).

### Margin Ratio
```
margin_ratio = (collateral + unrealized_pnl) / position_size
```
Liquidation triggers when margin_ratio falls below maintenance threshold (e.g. 5%).

**Questions to answer before moving on:**
- [ ] At 10x leverage, how much does the price need to move against you to get liquidated?
- [ ] If mark price = $102 and index price = $100, who pays whom in funding?

**Time estimate:** 1–2 hours

---

## Step 4 — Understand Solana / Anchor Basics

Skip this step if you already know Anchor.

**Resources:**
- Search: "Anchor book Solana" (official framework docs)
- Search: "Solana cookbook"

**Key concepts to understand:**
- [ ] What is a PDA (Program Derived Address) and why is it used?
- [ ] How does Solana's account model differ from Ethereum?
- [ ] What is a CPI (Cross-Program Invocation)?
- [ ] What is rent and why do accounts need a minimum balance?
- [ ] How do Anchor's `#[account]` and `#[derive(Accounts)]` macros work?

**Time estimate:** 2–4 hours (more if Solana is new to you)

---

## Step 5 — Read Real On-Chain Perps Code

Look at open-source Anchor implementations — don't try to understand everything,
just get familiar with how accounts and instructions are structured.

**What to search:**
- "drift-program GitHub" — Drift's on-chain Anchor program (production-grade reference)
- "perpetuals Solana GitHub" — several open-source examples to compare

**What to look for:**
- How is the `Position` account struct defined? What fields does it have?
- How does `open_position` instruction validate inputs?
- How are PDAs seeded for markets and positions?

**Time estimate:** 1–2 hours

---

## Checklist — Ready to Code?

Before starting Phase 1 of the build, you should be able to answer all of these:

### Financial Mechanics
- [ ] What is a perpetual future and how does it differ from a spot trade?
- [ ] What is funding rate and why does it exist?
- [ ] What triggers a liquidation?
- [ ] What is the difference between mark price and index price?
- [ ] Who is the counterparty when a user opens a long on a DEX?

### Protocol Design
- [ ] What does a `Position` account need to store?
- [ ] What does a `Market` account need to store?
- [ ] What is the difference between a vAMM-based and order book-based perp DEX?
- [ ] How does the protocol ensure it can pay out winning positions?

### Solana / Anchor
- [ ] What is a PDA and how would you derive one for a user's position?
- [ ] What is the purpose of the `Vault` account in this project?
- [ ] How does a liquidator call the `liquidate` instruction safely?

---

## Notes

Use this section to jot down anything surprising or unclear as you research.

---
