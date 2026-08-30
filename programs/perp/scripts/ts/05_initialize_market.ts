import { TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { SystemProgram } from "@solana/web3.js";
import {
  payer,
  perpProgram,
  globalConfig,
  lpPool,
  symbolBytes,
  market,
  marketVault,
  insuranceFundVault,
  USDC_MINT,
  FEED_ID_HEX,
  PYTH_PRICE_FEED,
  explorer,
} from "./common.js";

// max_leverage=50, mmr_bps=500 (5%) so that:
//  - a low-leverage position (e.g. 2x) stays comfortably above maintenance margin -> closeable normally
//  - a max-leverage (50x) position is liquidatable immediately after opening
//    (equity == collateral <= notional * mmr_bps/10_000 == collateral * 50 * 0.05 = collateral * 2.5)

async function main() {
  console.log("payer:", payer.publicKey.toBase58());
  console.log("market PDA:", market.toBase58());
  console.log("market_vault PDA:", marketVault.toBase58());
  console.log("insurance_fund_vault PDA:", insuranceFundVault.toBase58());

  const sig = await perpProgram.methods
    .initializeMarket(symbolBytes, {
      maxLeverage: 50,
      mmrBps: 500,
      feedId: `0x${FEED_ID_HEX}`,
    })
    .accounts({
      payer: payer.publicKey,
      globalConfig,
      market,
      vault: marketVault,
      insuranceFundVault,
      lpPool,
      oracle: PYTH_PRICE_FEED,
      usdcMint: USDC_MINT,
      tokenProgram: TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
    })
    .rpc();

  console.log("tx signature:", sig);
  console.log(explorer(sig));

  const acc = await perpProgram.account.synteticMarket.fetch(market);
  console.log("market after init:", acc);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
