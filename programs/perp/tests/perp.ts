import * as anchor from "@anchor-lang/core";
import { Program } from "@anchor-lang/core";
import { Perp } from "../target/types/perp";
import { expect } from "chai";

describe("perp", () => {
  anchor.setProvider(anchor.AnchorProvider.env());
  const provider = anchor.AnchorProvider.env();
  const program = anchor.workspace.Perp as Program<Perp>;

  it("connects to the local surfnet", async () => {
    const version = await provider.connection.getVersion();
    console.log("connected to validator, version:", version);
    expect(version).to.have.property("solana-core");
  });

  it("sees the deployed program on-chain", async () => {
    const info = await provider.connection.getAccountInfo(program.programId);
    console.log("program account owner:", info?.owner.toBase58());
    expect(info).to.not.be.null;
    expect(info!.executable).to.be.true;
  });
});