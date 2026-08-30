import { TOKEN_PROGRAM_ID, getAssociatedTokenAddressSync } from "@solana/spl-token";
import { SystemProgram, SYSVAR_INSTRUCTIONS_PUBKEY } from "@solana/web3.js";
import BN from "bn.js";
import {
  payer,
  connection,
  perpProgram,
  globalConfig,
  market,
  marketVault,
  insuranceFundVault,
  lpPool,
  lpUsdcVault,
  lpProgram,
  USDC_MINT,
  FEE_RECEIVER,
  PYTH_PRICE_FEED,
  positionPda,
  explorer,
  waitForFreshOracle,
} from "./common.js";

const position = positionPda(payer.publicKey, market);

async function openPosition(leverage: number, margin: number) {
  const traderAta = getAssociatedTokenAddressSync(USDC_MINT, payer.publicKey);
  const feeReceiverAta = getAssociatedTokenAddressSync(USDC_MINT, FEE_RECEIVER);

  const sig = await perpProgram.methods
    .openPosition({
      leverage,
      margin: new BN(margin),
      takeProfit: new BN(0),
      stopLoss: new BN(0),
      positionType: { long: {} },
    })
    .accounts({
      trader: payer.publicKey,
      priceUpdate: PYTH_PRICE_FEED,
      globalConfig,
      position,
      marketVault,
      market,
      lpPool,
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
  console.log(`open_position(leverage=${leverage}) tx:`, sig, explorer(sig));
}

async function closePosition() {
  const traderAta = getAssociatedTokenAddressSync(USDC_MINT, payer.publicKey);
  const feeReceiverAta = getAssociatedTokenAddressSync(USDC_MINT, FEE_RECEIVER);

  const sig = await perpProgram.methods
    .closePosition()
    .accounts({
      trader: payer.publicKey,
      priceUpdate: PYTH_PRICE_FEED,
      globalConfig,
      position,
      marketVault,
      market,
      lpPool,
      lpPoolUsdcVault: lpUsdcVault,
      lpPoolProgram: lpProgram.programId,
      feeReceiverAta,
      traderUsdcAta: traderAta,
      usdcMint: USDC_MINT,
      tokenProgram: TOKEN_PROGRAM_ID,
      instructionsSysvar: SYSVAR_INSTRUCTIONS_PUBKEY,
    })
    .rpc();
  console.log("close_position tx:", sig, explorer(sig));
}

async function liquidate() {
  const liquidatorAta = getAssociatedTokenAddressSync(USDC_MINT, payer.publicKey);

  const sig = await perpProgram.methods
    .liquidate()
    .accounts({
      liquidator: payer.publicKey,
      trader: payer.publicKey,
      priceUpdate: PYTH_PRICE_FEED,
      position,
      marketVault,
      insuranceFundVault,
      market,
      liquidatorUsdcAta: liquidatorAta,
      usdcMint: USDC_MINT,
      tokenProgram: TOKEN_PROGRAM_ID,
    })
    .rpc();
  console.log("liquidate tx:", sig, explorer(sig));
}

async function main() {
  await waitForFreshOracle();

  const existing = await connection.getAccountInfo(position);
  if (existing) {
    console.log("closing leftover open position first...");
    await closePosition();
  }

  await openPosition(2, 5000);
  await closePosition();

  await openPosition(50, 400);
  await liquidate();
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
