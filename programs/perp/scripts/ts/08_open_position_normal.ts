import { TOKEN_PROGRAM_ID, getAssociatedTokenAddressSync } from "@solana/spl-token";
import { SystemProgram, SYSVAR_INSTRUCTIONS_PUBKEY } from "@solana/web3.js";
import BN from "bn.js";
import {
  payer,
  perpProgram,
  globalConfig,
  market,
  marketVault,
  lpPool,
  lpUsdcVault,
  lpProgram,
  USDC_MINT,
  PYTH_PRICE_FEED,
  FEE_RECEIVER,
  positionPda,
  explorer,
} from "./common.js";

export const position = positionPda(payer.publicKey, market);

async function main() {
  const traderAta = getAssociatedTokenAddressSync(USDC_MINT, payer.publicKey);
  const feeReceiverAta = getAssociatedTokenAddressSync(USDC_MINT, FEE_RECEIVER);

  console.log("trader:", payer.publicKey.toBase58());
  console.log("position PDA:", position.toBase58());

  const sig = await perpProgram.methods
    .openPosition({
      leverage: 2,
      margin: new BN(5000), // 0.005 USDC before fee
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

  console.log("tx signature:", sig);
  console.log(explorer(sig));

  const acc = await perpProgram.account.position.fetch(position);
  console.log("position after open:", acc);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
