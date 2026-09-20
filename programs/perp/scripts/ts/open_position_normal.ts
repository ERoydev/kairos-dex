import { TOKEN_PROGRAM_ID, getAssociatedTokenAddressSync } from "@solana/spl-token";
import { SystemProgram, SYSVAR_INSTRUCTIONS_PUBKEY, PublicKey } from "@solana/web3.js";
import BN from "bn.js";
import {
  payer,
  perpProgram,
  globalConfig,
  lpPoolPda,
  lpUsdcVault,
  lpProgram,
  USDC_MINT,
  FEE_RECEIVER,
  positionPda,
  explorer,
  symbolToBytes16,
  marketPda,
  marketVaultPda,
  waitForFreshOracle
} from "./common.js";

const market_symbol: number[] = symbolToBytes16("SOL-PERP");
export const market = marketPda(Buffer.from(market_symbol));
export const marketVault = marketVaultPda(market);

export const PYTH_PRICE_FEED = new PublicKey("7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE");

export const position = positionPda(payer.publicKey, market);


async function main() {
  // Compute ATA's from wallet pubkeys
  const traderAta = getAssociatedTokenAddressSync(USDC_MINT, payer.publicKey);
  const feeReceiverAta = getAssociatedTokenAddressSync(USDC_MINT, FEE_RECEIVER);
  

  // ===== Prerequisities
  // initialize fee receiver ATA, run in bash `make initialize-fee-receiver`

  // Logs for debugging purposes
  console.log("Trader:", payer.publicKey.toBase58());
  console.log("Position PDA:", position.toBase58());
  console.log("Market: ", market.toBase58());
  console.log("Market Vault: ", marketVault.toBase58());
  console.log("Lp Pool: ", lpPoolPda.toBase58());
  console.log("Lp Pool Usdc Vault ", lpUsdcVault.toBase58());
  console.log("Lp Pool Program ", lpProgram.programId);
  console.log()

  // Avoid `OracleGuardReadFailed` program error and wait till oracle is fresh
  await waitForFreshOracle(PYTH_PRICE_FEED);

  let openPositionParams = {
    "leverage": 2,
    "margin": new BN(5000), // 0.005 USDC before fee
    "takeProfit": new BN(0),
    "stopLoss": new BN(0),
    "positionType": { long: {} },
  }

  const sig = await perpProgram.methods
    .openPosition(openPositionParams)
    .accounts({
      trader: payer.publicKey,
      priceUpdate: PYTH_PRICE_FEED,
      globalConfig,
      position,
      marketVault,
      market,
      lpPool: lpPoolPda,
      lpPoolUsdcVault: lpUsdcVault,
      lpPoolProgram: lpProgram.programId,
      feeReceiverAta,
      traderUsdcAta: traderAta,
      usdcMint: USDC_MINT,
      tokenProgram: TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
      instructionsSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
    })
    .rpc();

  console.log("tx signature:", sig);
  console.log(explorer(sig));

  const acc = await perpProgram.account.position.fetch(position);
  console.log("position after open:", acc);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
