import * as anchor from "@anchor-lang/core";
import { Program, BN } from "@anchor-lang/core";
import {
  PublicKey,
  Keypair,
  LAMPORTS_PER_SOL,
} from "@solana/web3.js";
import { getAssociatedTokenAddressSync, TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { createHash } from "crypto";
import { expect } from "chai";
import { Perp } from "../target/types/perp";
// Sibling workspace — liquidity-pool is a separate Anchor program this one CPIs into.
import lpIdl from "../../liquidity-pool/target/idl/liquidity_pool.json";
import { LiquidityPool } from "../../liquidity-pool/target/types/liquidity_pool";

// Real mainnet USDC — the deployed liquidity-pool program was built WITHOUT
// `--features dev`, so it hardcodes this exact mint (see constants.rs `USDC_MINT`,
// `#[cfg(not(feature = "dev"))]`). Surfpool clones it lazily from its mainnet fork.
const USDC_MINT = new PublicKey("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");

// pyth-solana-receiver-sdk's non-"pro-compatible" program id (the one perp links
// against — see Cargo.toml, no `pro-compatible` feature enabled).
const PYTH_RECEIVER_PROGRAM_ID = new PublicKey(
  "rec5EKMGg6MxZYaMdyBfgwp4d5rB9T1VQH5pJv5LtFJ"
);

const GLOBAL_SEED = Buffer.from("global");
const MARKET_SEED = Buffer.from("market_seed");
const MARKET_VAULT_SEED = Buffer.from("market_vault");
const INSURANCE_FUND_VAULT_SEED = Buffer.from("insurance_fund_vault");
const POSITION_SEED = Buffer.from("position");
const LIQUIDITY_POOL_SEED = Buffer.from("liquidity_pool");
const USDC_VAULT_SEED = Buffer.from("usdc_vault");
const LP_MINT_SEED = Buffer.from("lp_mint");

function symbolToBytes(symbol: string): number[] {
  const buf = Buffer.alloc(16);
  buf.write(symbol, 0, "utf8");
  return Array.from(buf);
}

// Raw JSON-RPC call for Surfpool's `surfnet_*` cheatcodes — not exposed by
// @solana/web3.js's Connection, so we hit the endpoint directly.
async function rpcCall(rpcUrl: string, method: string, params: unknown[]) {
  const res = await fetch(rpcUrl, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ jsonrpc: "2.0", id: 1, method, params }),
  });
  const json = await res.json();
  if (json.error) {
    throw new Error(`${method} failed: ${JSON.stringify(json.error)}`);
  }
  return json.result;
}

// Mirrors `fabricate_price_update()` in the Rust LiteSVM tests
// (tests/common/mod.rs), but written directly against a live validator via the
// surfnet_setAccount cheatcode instead of svm.set_account().
//
// Anchor account layout: 8-byte discriminator + PriceUpdateV2 fields, per
// pyth-solana-receiver-sdk 2.0.0's `price_update.rs`:
//   write_authority: Pubkey (32)
//   verification_level: enum { Partial{u8}=0, Full=1 } -> 1 byte tag (Full, no payload)
//   price_message: PriceFeedMessage (pythnet-sdk messages.rs) — 84 bytes:
//     feed_id: [u8;32], price: i64, conf: u64, exponent: i32,
//     publish_time: i64, prev_publish_time: i64, ema_price: i64, ema_conf: u64
//   posted_slot: u64 (8)
function buildPriceUpdateV2Bytes(
  feedId: Buffer,
  price: bigint,
  conf: bigint,
  exponent: number,
  publishTime: bigint
): Buffer {
  const discriminator = createHash("sha256")
    .update("account:PriceUpdateV2")
    .digest()
    .subarray(0, 8);

  const writeAuthority = Buffer.alloc(32); // default Pubkey
  const verificationLevel = Buffer.from([1]); // VerificationLevel::Full

  const priceMessage = Buffer.alloc(84);
  let o = 0;
  feedId.copy(priceMessage, o); o += 32;
  priceMessage.writeBigInt64LE(price, o); o += 8; // price
  priceMessage.writeBigUInt64LE(conf, o); o += 8; // conf
  priceMessage.writeInt32LE(exponent, o); o += 4; // exponent
  priceMessage.writeBigInt64LE(publishTime, o); o += 8; // publish_time
  priceMessage.writeBigInt64LE(publishTime, o); o += 8; // prev_publish_time
  priceMessage.writeBigInt64LE(price, o); o += 8; // ema_price
  priceMessage.writeBigUInt64LE(conf, o); o += 8; // ema_conf

  const postedSlot = Buffer.alloc(8); // 0

  return Buffer.concat([
    discriminator,
    writeAuthority,
    verificationLevel,
    priceMessage,
    postedSlot,
  ]);
}

describe("perp: full integration flow (surfpool)", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const rpcUrl = provider.connection.rpcEndpoint;

  const perp = anchor.workspace.Perp as Program<Perp>;
  const lpPool = new Program(lpIdl as anchor.Idl, provider) as unknown as Program<LiquidityPool>;

  const admin = provider.wallet.publicKey; // global authority, must fund provider.wallet's keypair
  const feeReceiver = Keypair.generate().publicKey;
  const trader = Keypair.generate();
  const lpProvider = Keypair.generate();

  const symbol = "SMOKE-PERP";
  const feedIdBytes = Buffer.from(Array.from({ length: 32 }, (_, i) => i + 1));
  const feedIdHex = "0x" + feedIdBytes.toString("hex");
  const oraclePlaceholder = Keypair.generate().publicKey; // unchecked, only needs != default

  // Price fixture: $60,000 with exponent -8 (matches Pyth's typical scaling).
  // Kept identical at open and close so pnl == 0 and assertions don't need to
  // replicate the fee/funding formulas in TS to predict exact payouts.
  const ORACLE_PRICE = 6_000_000_000_000n;
  const ORACLE_CONF = 1n;
  const ORACLE_EXPO = -8;

  let priceUpdateAccount: PublicKey;

  async function plantOraclePrice() {
    priceUpdateAccount = Keypair.generate().publicKey;
    const data = buildPriceUpdateV2Bytes(
      feedIdBytes,
      ORACLE_PRICE,
      ORACLE_CONF,
      ORACLE_EXPO,
      BigInt(Math.floor(Date.now() / 1000))
    );
    await rpcCall(rpcUrl, "surfnet_setAccount", [
      priceUpdateAccount.toBase58(),
      {
        lamports: 1_000_000_000,
        data: [data.toString("base64"), "base64"],
        owner: PYTH_RECEIVER_PROGRAM_ID.toBase58(),
        executable: false,
      },
    ]);
  }

  async function setUsdcBalance(owner: PublicKey, amount: bigint) {
    // Cheatcode wants `amount` as a JSON number (u64), not a string — all our
    // fixture amounts are well under Number.MAX_SAFE_INTEGER so this is safe.
    await rpcCall(rpcUrl, "surfnet_setTokenAccount", [
      owner.toBase58(),
      USDC_MINT.toBase58(),
      { amount: Number(amount) },
    ]);
  }

  async function usdcBalanceOf(owner: PublicKey): Promise<bigint> {
    const ata = getAssociatedTokenAddressSync(USDC_MINT, owner, true);
    const bal = await provider.connection.getTokenAccountBalance(ata);
    return BigInt(bal.value.amount);
  }

  // --- PDAs ---
  const [globalConfigPda] = PublicKey.findProgramAddressSync(
    [GLOBAL_SEED],
    perp.programId
  );
  const [marketPda] = PublicKey.findProgramAddressSync(
    [MARKET_SEED, Buffer.from(symbolToBytes(symbol))],
    perp.programId
  );
  const [marketVaultPda] = PublicKey.findProgramAddressSync(
    [MARKET_VAULT_SEED, marketPda.toBuffer()],
    perp.programId
  );
  const [insuranceFundVaultPda] = PublicKey.findProgramAddressSync(
    [INSURANCE_FUND_VAULT_SEED, marketPda.toBuffer()],
    perp.programId
  );
  const [positionPda] = PublicKey.findProgramAddressSync(
    [POSITION_SEED, trader.publicKey.toBuffer(), marketPda.toBuffer()],
    perp.programId
  );
  const [lpPoolPda] = PublicKey.findProgramAddressSync(
    [LIQUIDITY_POOL_SEED],
    lpPool.programId
  );
  const [lpUsdcVaultPda] = PublicKey.findProgramAddressSync(
    [USDC_VAULT_SEED],
    lpPool.programId
  );
  const [lpMintPda] = PublicKey.findProgramAddressSync(
    [LP_MINT_SEED],
    lpPool.programId
  );

  const feeReceiverAta = getAssociatedTokenAddressSync(USDC_MINT, feeReceiver, true);
  const traderUsdcAta = getAssociatedTokenAddressSync(USDC_MINT, trader.publicKey);
  const lpProviderLpAta = getAssociatedTokenAddressSync(lpMintPda, lpProvider.publicKey);

  before(async () => {
    for (const kp of [trader, lpProvider]) {
      const sig = await provider.connection.requestAirdrop(
        kp.publicKey,
        2 * LAMPORTS_PER_SOL
      );
      const latest = await provider.connection.getLatestBlockhash();
      await provider.connection.confirmTransaction(
        { signature: sig, ...latest },
        "confirmed"
      );
    }

    // Seed USDC balances directly via cheatcode — real mainnet USDC has no
    // mint authority we control, so we can't just mintTo().
    await setUsdcBalance(trader.publicKey, 10_000_000_000n); // 10,000 USDC
    await setUsdcBalance(lpProvider.publicKey, 1_000_000_000n); // 1,000 USDC
    await setUsdcBalance(feeReceiver, 0n); // just to materialize the ATA
  });

  it("initializes global config", async () => {
    await perp.methods
      .initializeGlobal(feeReceiver, 10)
      .accounts({
        payer: admin,
      })
      .rpc();

    const global = await perp.account.globalConfig.fetch(globalConfigPda);
    expect(global.authority.toBase58()).to.equal(admin.toBase58());
    expect(global.feeReceiver.toBase58()).to.equal(feeReceiver.toBase58());
    expect(global.isPaused).to.be.false;
    expect(global.maxMarkets).to.equal(10);
  });

  it("initializes the LP pool", async () => {
    await lpPool.methods
      .initializePool()
      .accounts({
        authority: admin,
        perpProgram: perp.programId,
      })
      .rpc();

    const pool = await lpPool.account.pool.fetch(lpPoolPda);
    expect(pool.totalAssets.toNumber()).to.equal(0);
    expect(pool.totalShares.toNumber()).to.equal(0);
    expect(pool.perpProgram.toBase58()).to.equal(perp.programId.toBase58());
  });

  it("initializes the market", async () => {
    await perp.methods
      .initializeMarket(symbolToBytes(symbol), {
        maxLeverage: 20,
        mmrBps: 500, // 5%
        feedId: feedIdHex,
      })
      .accounts({
        payer: admin,
        oracle: oraclePlaceholder,
        usdcMint: USDC_MINT,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .rpc();

    const market = await perp.account.synteticMarket.fetch(marketPda);
    expect(market.isActive).to.be.true;
    expect(market.oiLong.toNumber()).to.equal(0);
    expect(market.oiShort.toNumber()).to.equal(0);
    expect(market.riskManagement.maxLeverage).to.equal(20);
    expect(market.feedId).to.equal(feedIdHex);

    // NOTE: initialize_market does `global_config.markets_count.checked_add(1)`
    // without assigning the result back — markets_count is never actually
    // incremented on-chain. Asserting current (buggy) behavior rather than
    // hiding it; flag to the program owner if this should be fixed.
    const global = await perp.account.globalConfig.fetch(globalConfigPda);
    expect(global.marketsCount).to.equal(0);
  });

  it("LP provider deposits into the pool", async () => {
    const depositAmount = 500_000_000n; // 500 USDC

    await lpPool.methods
      .deposit(new BN(depositAmount.toString()))
      .accounts({
        provider: lpProvider.publicKey,
      })
      .signers([lpProvider])
      .rpc();

    const pool = await lpPool.account.pool.fetch(lpPoolPda);
    expect(BigInt(pool.totalAssets.toString())).to.equal(depositAmount);
    expect(BigInt(pool.totalShares.toString())).to.equal(depositAmount); // 1:1, first deposit

    const lpShareBal = await provider.connection.getTokenAccountBalance(lpProviderLpAta);
    expect(BigInt(lpShareBal.value.amount)).to.equal(depositAmount);

    const providerUsdcAfter = await usdcBalanceOf(lpProvider.publicKey);
    expect(providerUsdcAfter).to.equal(1_000_000_000n - depositAmount);
  });

  it("trader opens a position", async () => {
    await plantOraclePrice();

    const traderUsdcBefore = await usdcBalanceOf(trader.publicKey);
    const vaultBefore = await provider.connection.getTokenAccountBalance(marketVaultPda);
    const feeReceiverBefore = await provider.connection.getTokenAccountBalance(feeReceiverAta);
    const lpVaultBefore = await provider.connection.getTokenAccountBalance(lpUsdcVaultPda);

    const margin = 1_000_000_000n; // 1,000 USDC

    await perp.methods
      .openPosition({
        leverage: 5,
        margin: new BN(margin.toString()),
        takeProfit: new BN(0),
        stopLoss: new BN(0),
        positionType: { long: {} },
      })
      .accounts({
        trader: trader.publicKey,
        priceUpdate: priceUpdateAccount,
        globalConfig: globalConfigPda,
        market: marketPda,
        lpPool: lpPoolPda,
        lpPoolUsdcVault: lpUsdcVaultPda,
        lpPoolProgram: lpPool.programId,
        traderUsdcAta: traderUsdcAta,
        usdcMint: USDC_MINT,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .signers([trader])
      .rpc();

    const position = await perp.account.position.fetch(positionPda);
    expect(position.owner.toBase58()).to.equal(trader.publicKey.toBase58());
    expect(position.market.toBase58()).to.equal(marketPda.toBase58());
    expect(position.side).to.deep.equal({ long: {} });
    expect(position.entryPrice.toNumber()).to.equal(Number(ORACLE_PRICE));
    // margin fully backs the position at 5x before fees; collateral is margin minus open fee.
    expect(position.collateral.toNumber()).to.be.greaterThan(0);
    expect(position.collateral.toNumber()).to.be.at.most(Number(margin));

    const market = await perp.account.synteticMarket.fetch(marketPda);
    expect(BigInt(market.oiLong.toString())).to.equal(BigInt(position.notional.toString()));
    expect(market.oiShort.toNumber()).to.equal(0);

    const traderUsdcAfter = await usdcBalanceOf(trader.publicKey);
    const vaultAfter = await provider.connection.getTokenAccountBalance(marketVaultPda);
    const feeReceiverAfter = await provider.connection.getTokenAccountBalance(feeReceiverAta);
    const lpVaultAfter = await provider.connection.getTokenAccountBalance(lpUsdcVaultPda);

    const traderDebited = traderUsdcBefore - traderUsdcAfter;
    expect(traderDebited).to.equal(margin); // trader always pays exactly o_params.margin

    const vaultDelta = BigInt(vaultAfter.value.amount) - BigInt(vaultBefore.value.amount);
    const feeReceiverDelta = BigInt(feeReceiverAfter.value.amount) - BigInt(feeReceiverBefore.value.amount);
    const lpVaultDelta = BigInt(lpVaultAfter.value.amount) - BigInt(lpVaultBefore.value.amount);

    // Accounting identity: everything debited from the trader ends up as
    // collateral in the vault, protocol fee to fee_receiver, or lp fee to the pool.
    expect(vaultDelta + feeReceiverDelta + lpVaultDelta).to.equal(traderDebited);
    expect(vaultDelta).to.equal(BigInt(position.collateral.toString()));
  });

  it("trader closes the position", async () => {
    await plantOraclePrice(); // refresh publish_time so the staleness guard doesn't reject it

    const positionBefore = await perp.account.position.fetch(positionPda);
    const traderUsdcBefore = await usdcBalanceOf(trader.publicKey);
    const vaultBefore = await provider.connection.getTokenAccountBalance(marketVaultPda);
    const feeReceiverBefore = await provider.connection.getTokenAccountBalance(feeReceiverAta);
    const lpVaultBefore = await provider.connection.getTokenAccountBalance(lpUsdcVaultPda);

    await perp.methods
      .closePosition()
      .accounts({
        trader: trader.publicKey,
        priceUpdate: priceUpdateAccount,
        globalConfig: globalConfigPda,
        market: marketPda,
        lpPool: lpPoolPda,
        lpPoolUsdcVault: lpUsdcVaultPda,
        lpPoolProgram: lpPool.programId,
        traderUsdcAta: traderUsdcAta,
        usdcMint: USDC_MINT,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .signers([trader])
      .rpc();

    // Position account is closed (`close = trader`) — it no longer exists.
    let positionStillExists = true;
    try {
      await perp.account.position.fetch(positionPda);
    } catch {
      positionStillExists = false;
    }
    expect(positionStillExists).to.be.false;

    const market = await perp.account.synteticMarket.fetch(marketPda);
    expect(market.oiLong.toNumber()).to.equal(0); // fully unwound, back to pre-open state

    const traderUsdcAfter = await usdcBalanceOf(trader.publicKey);
    const vaultAfter = await provider.connection.getTokenAccountBalance(marketVaultPda);
    const feeReceiverAfter = await provider.connection.getTokenAccountBalance(feeReceiverAta);
    const lpVaultAfter = await provider.connection.getTokenAccountBalance(lpUsdcVaultPda);

    const traderCredited = traderUsdcAfter - traderUsdcBefore;
    const vaultDelta = BigInt(vaultBefore.value.amount) - BigInt(vaultAfter.value.amount); // decrease
    const feeReceiverDelta = BigInt(feeReceiverAfter.value.amount) - BigInt(feeReceiverBefore.value.amount);
    const lpVaultDelta = BigInt(lpVaultAfter.value.amount) - BigInt(lpVaultBefore.value.amount);

    // Price didn't move between open and close, so pnl == 0: the trader gets
    // (roughly) their collateral back minus the close fee, entirely vault-funded
    // (pool untouched by the pnl settlement — see `settle()` in utils/pnl.rs).
    expect(traderCredited > 0n).to.be.true;
    expect(traderCredited <= BigInt(positionBefore.collateral.toString())).to.be.true;

    // Vault conservation: everything drained from the vault becomes the fee
    // split (fee_receiver + lp pool) plus whatever was paid out to the trader.
    expect(vaultDelta).to.equal(feeReceiverDelta + lpVaultDelta + traderCredited);
  });
});
