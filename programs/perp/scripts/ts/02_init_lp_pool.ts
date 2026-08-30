import { TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { SystemProgram } from "@solana/web3.js";
import {
  payer,
  perpProgram,
  lpProgram,
  lpPool,
  lpUsdcVault,
  lpMint,
  USDC_MINT,
  explorer,
} from "./common.js";

async function main() {
  console.log("authority:", payer.publicKey.toBase58());
  console.log("lp_pool PDA:", lpPool.toBase58());
  console.log("usdc_vault PDA:", lpUsdcVault.toBase58());
  console.log("lp_mint PDA:", lpMint.toBase58());

  const sig = await lpProgram.methods
    .initializePool()
    .accounts({
      authority: payer.publicKey,
      pool: lpPool,
      usdcMint: USDC_MINT,
      usdcVault: lpUsdcVault,
      lpMint,
      perpProgram: perpProgram.programId,
      tokenProgram: TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    })
    .rpc();

  console.log("tx signature:", sig);
  console.log(explorer(sig));
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
