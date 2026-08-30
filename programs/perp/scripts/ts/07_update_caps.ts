import BN from "bn.js";
import { payer, perpProgram, market, explorer } from "./common.js";

async function main() {
  const sig = await perpProgram.methods
    .updateCaps({
      maxPositionNotional: new BN(1_000_000), // 1 USDC
      maxUserNotional: new BN(2_000_000), // 2 USDC
      maxOiLong: null,
      maxOiShort: null,
      maxSkew: null,
    })
    .accounts({ authority: payer.publicKey, market })
    .rpc();

  console.log("tx signature:", sig);
  console.log(explorer(sig));

  const acc = await perpProgram.account.synteticMarket.fetch(market);
  console.log("caps after update:", acc.riskManagement.caps);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
