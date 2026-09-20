import {
  ASSOCIATED_TOKEN_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  createAssociatedTokenAccountInstruction,
  getAssociatedTokenAddressSync,
} from "@solana/spl-token";
import { Transaction } from "@solana/web3.js";
import {
  payer,
  connection,
  perpProgram,
  globalConfig,
  USDC_MINT,
  FEE_RECEIVER,
  explorer,
} from "./common.js";

async function main() {
  const sig1 = await perpProgram.methods
    .updateGlobal({
      authority: null,
      feeReceiver: FEE_RECEIVER,
      isPaused: null,
      maxMarkets: null,
    })
    .accounts({ authority: payer.publicKey, globalConfig })
    .rpc();
  console.log("update_global (fee_receiver) tx:", sig1);
  console.log(explorer(sig1));

  const feeReceiverAta = getAssociatedTokenAddressSync(USDC_MINT, FEE_RECEIVER);
  const info = await connection.getAccountInfo(feeReceiverAta);
  if (!info) {
    const tx = new Transaction().add(
      createAssociatedTokenAccountInstruction(
        payer.publicKey,
        feeReceiverAta,
        FEE_RECEIVER,
        USDC_MINT,
        TOKEN_PROGRAM_ID,
        ASSOCIATED_TOKEN_PROGRAM_ID,
      ),
    );
    const sig2 = await connection.sendTransaction(tx, [payer]);
    await connection.confirmTransaction(sig2, "confirmed");
    console.log("create fee_receiver_ata tx:", sig2);
    console.log(explorer(sig2));
  } else {
    console.log("fee_receiver_ata already exists:", feeReceiverAta.toBase58());
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
