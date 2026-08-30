import * as anchor from "@anchor-lang/core";
import { Program } from "@anchor-lang/core";
import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import fs from "fs";
import os from "os";
import { createRequire } from "module";
import type { Perp } from "../../target/types/perp";
import type { LiquidityPool } from "../../../liquidity-pool/target/types/liquidity_pool";

const require = createRequire(import.meta.url);

if (fs.existsSync(new URL("../../.env", import.meta.url))) {
  process.loadEnvFile(new URL("../../.env", import.meta.url));
}

// All overridable via .env (see .env.example) so redeploying to a different
// program ID / market / wallet doesn't require editing this file.
export const RPC_URL = process.env.RPC_URL ?? "https://api.devnet.solana.com";
export const USDC_MINT = new PublicKey(
  process.env.USDC_MINT ?? "4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU",
);
// Feed used in the program's own open_position.rs test (read_live_oracle_price) —
// already validated to publish with exponent -8, matching PYTH_PRICE_EXPONENT.
export const FEED_ID_HEX =
  process.env.PYTH_FEED_ID_HEX ??
  "e62df6c8b4a85fe1a67db44dc12de5db330f7ac66b72dc658afedf0f4a415b43";
// Pyth's own keeper continuously updates this canonical price-feed PDA on devnet
// (shard 0) for the feed above, so we can read it directly without posting updates
// ourselves. Derived via PythSolanaReceiver.getPriceFeedAccountAddress(0, FEED_ID_HEX).
export const PYTH_PRICE_FEED = new PublicKey(
  process.env.PYTH_PRICE_FEED_ACCOUNT ?? "4cSM2e6rvbGQUFiJbqytoVMi5GgghSMr8LwVrT9VPSPo",
);
// Separate from the trader/payer wallet — open_position/close_position pass
// fee_receiver_ata and trader_usdc_ata as distinct accounts, and Anchor rejects
// the same mutable account being passed under two different names.
export const FEE_RECEIVER = new PublicKey(
  process.env.FEE_RECEIVER ?? "8ab1ytf1MV5h7UF23ntqrSbHpFh6FY5ma3pLHghCJCwz",
);
export const SYMBOL = process.env.MARKET_SYMBOL ?? "TESTUSD";

const walletPath = process.env.ANCHOR_WALLET ?? `${os.homedir()}/.config/solana/id.json`;
const secret = JSON.parse(fs.readFileSync(walletPath, "utf-8"));
export const payer = Keypair.fromSecretKey(new Uint8Array(secret));

export const connection = new Connection(RPC_URL, "confirmed");
export const wallet = new anchor.Wallet(payer);
export const provider = new anchor.AnchorProvider(connection, wallet, { commitment: "confirmed" });
anchor.setProvider(provider);

const perpIdl = require("../../target/idl/perp.json");
const lpIdl = require("../../../liquidity-pool/target/idl/liquidity_pool.json");

export const perpProgram = new Program(perpIdl as anchor.Idl, provider) as unknown as Program<Perp>;
export const lpProgram = new Program(lpIdl as anchor.Idl, provider) as unknown as Program<LiquidityPool>;

export const [globalConfig] = PublicKey.findProgramAddressSync(
  [Buffer.from("global")],
  perpProgram.programId,
);
export const [lpPool] = PublicKey.findProgramAddressSync(
  [Buffer.from("liquidity_pool")],
  lpProgram.programId,
);
export const [lpUsdcVault] = PublicKey.findProgramAddressSync(
  [Buffer.from("usdc_vault")],
  lpProgram.programId,
);
export const [lpMint] = PublicKey.findProgramAddressSync(
  [Buffer.from("lp_mint")],
  lpProgram.programId,
);

export function marketPda(symbol: Buffer) {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("market_seed"), symbol],
    perpProgram.programId,
  )[0];
}
export function marketVaultPda(market: PublicKey) {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("market_vault"), market.toBuffer()],
    perpProgram.programId,
  )[0];
}
export function insuranceFundVaultPda(market: PublicKey) {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("insurance_fund_vault"), market.toBuffer()],
    perpProgram.programId,
  )[0];
}
export function positionPda(trader: PublicKey, market: PublicKey) {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("position"), trader.toBuffer(), market.toBuffer()],
    perpProgram.programId,
  )[0];
}

export function symbolToBytes16(symbol: string): number[] {
  const buf = Buffer.alloc(16);
  buf.write(symbol, "utf-8");
  return Array.from(buf);
}

export const symbolBytes = symbolToBytes16(SYMBOL);
export const market = marketPda(Buffer.from(symbolBytes));
export const marketVault = marketVaultPda(market);
export const insuranceFundVault = insuranceFundVaultPda(market);

export function explorer(sig: string) {
  return `https://explorer.solana.com/tx/${sig}?cluster=devnet`;
}

// PYTH_PRICE_FEED is refreshed by Pyth's own devnet infra every ~15-30min, but
// the program rejects prices older than 60s (see OracleConfig::max_staleness_secs
// in oracle.rs). Poll until it's fresh enough before firing price-dependent
// instructions (open_position/close_position/liquidate), instead of guessing.
const PRICE_UPDATE_PUBLISH_TIME_OFFSET = 8 + 32 + 1 + 32 + 8 + 8 + 4; // disc + write_authority + verification_level + feed_id + price + conf + exponent

export async function pythPriceAgeSeconds(): Promise<number> {
  const info = await connection.getAccountInfo(PYTH_PRICE_FEED);
  if (!info) throw new Error(`PYTH_PRICE_FEED account not found: ${PYTH_PRICE_FEED.toBase58()}`);
  const publishTime = info.data.readBigInt64LE(PRICE_UPDATE_PUBLISH_TIME_OFFSET);
  return Math.floor(Date.now() / 1000) - Number(publishTime);
}

export async function waitForFreshOracle(
  maxAgeSecs = 45,
  pollIntervalMs = 15_000,
  maxWaitMs = 40 * 60_000,
): Promise<void> {
  const deadline = Date.now() + maxWaitMs;
  for (;;) {
    const age = await pythPriceAgeSeconds();
    if (age <= maxAgeSecs) {
      console.log(`oracle fresh (${age}s old), proceeding`);
      return;
    }
    if (Date.now() > deadline) {
      throw new Error(`oracle still stale (${age}s old) after waiting ${maxWaitMs / 60_000}min`);
    }
    console.log(`oracle stale (${age}s old), waiting for Pyth's devnet keeper to refresh it...`);
    await new Promise((r) => setTimeout(r, pollIntervalMs));
  }
}
