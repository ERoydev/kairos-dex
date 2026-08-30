import { payer, perpProgram, market, explorer } from "./common.js";

async function main() {
  for (const isActive of [false, true]) {
    const sig = await perpProgram.methods
      .marketPause(isActive)
      .accounts({ authority: payer.publicKey, market })
      .rpc();
    console.log(`market_pause(${isActive}) tx:`, sig);
    console.log(explorer(sig));
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
