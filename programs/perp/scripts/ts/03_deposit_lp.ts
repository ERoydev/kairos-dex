import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import { SystemProgram } from "@solana/web3.js";
import BN from "bn.js";
import {
  payer,
  lpProgram,
  lpPool,
  lpUsdcVault,
  lpMint,
  USDC_MINT,
  explorer,
} from "./common.js";

const DEPOSIT_AMOUNT = 2_000_000; // 2 USDC (6 decimals)

async function main() {
  const providerAta = getAssociatedTokenAddressSync(USDC_MINT, payer.publicKey);
  const providerLpAta = getAssociatedTokenAddressSync(lpMint, payer.publicKey);

  console.log("provider:", payer.publicKey.toBase58());
  console.log("provider_ata (usdc):", providerAta.toBase58());
  console.log("provider_lp_ata:", providerLpAta.toBase58());

  const sig = await lpProgram.methods
    .deposit(new BN(DEPOSIT_AMOUNT))
    .accounts({
      provider: payer.publicKey,
      pool: lpPool,
      providerAta,
      providerLpAta,
      usdcMint: USDC_MINT,
      usdcVault: lpUsdcVault,
      lpMint,
      tokenProgram: TOKEN_PROGRAM_ID,
      associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    })
    .rpc();

  console.log("tx signature:", sig);
  console.log(explorer(sig));

  const pool = await lpProgram.account.pool.fetch(lpPool);
  console.log("pool after deposit:", pool);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
