import * as anchor from "@anchor-lang/core";
import { Program } from "@anchor-lang/core";
import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import fs from "fs";
import os from "os";
import { createRequire } from "module";
import type { Perp } from "../../target/types/perp";

const require = createRequire(import.meta.url);
const idl = require("../../target/idl/perp.json");

const DEVNET_URL = "https://api.devnet.solana.com";
const walletPath = process.env.ANCHOR_WALLET ?? `${os.homedir()}/.config/solana/id.json`;

const secret = JSON.parse(fs.readFileSync(walletPath, "utf-8"));
const payer = Keypair.fromSecretKey(new Uint8Array(secret));

const connection = new Connection(DEVNET_URL, "confirmed");
const wallet = new anchor.Wallet(payer);
const provider = new anchor.AnchorProvider(connection, wallet, { commitment: "confirmed" });
anchor.setProvider(provider);

const program = new Program(idl as anchor.Idl, provider) as unknown as Program<Perp>;

const [globalConfig] = PublicKey.findProgramAddressSync(
  [Buffer.from("global")],
  program.programId,
);

async function main() {
  console.log("payer:", payer.publicKey.toBase58());
  console.log("program:", program.programId.toBase58());
  console.log("global_config PDA:", globalConfig.toBase58());

  const sig = await program.methods
    .initializeGlobal(payer.publicKey, 10) // fee_receiver = payer, max_markets = 10
    .accounts({
      payer: payer.publicKey,
      globalConfig,
      systemProgram: anchor.web3.SystemProgram.programId,
    })
    .rpc();

  console.log("tx signature:", sig);
  console.log(`https://explorer.solana.com/tx/${sig}?cluster=devnet`);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
