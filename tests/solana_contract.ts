import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { SolanaContract } from "../target/types/solana_contract";
import { SystemProgram } from "@solana/web3.js";

describe("solana_contract", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.solanaContract as Program<SolanaContract>;

  it("Initialize and Set Value on Devnet", async () => {
    const state = anchor.web3.Keypair.generate();

    const airdropSig = await provider.connection.requestAirdrop(
      state.publicKey,
      2 * anchor.web3.LAMPORTS_PER_SOL
    );
    await provider.connection.confirmTransaction(airdropSig);

  const txInit = await program.methods
  .initialize()
  .accounts({
    state: state.publicKey,
    user: provider.wallet.publicKey,
    systemProgram: SystemProgram.programId, // web3.js import 사용
  })
  .signers([state])
  .rpc();
    console.log("Initialize tx:", txInit);

    const txSet = await program.methods
      .setValue(new anchor.BN(123))
      .accounts({ state: state.publicKey })
      .rpc();
    console.log("SetValue tx:", txSet);

    const account = await program.account.stateAccount.fetch(state.publicKey);
    console.log("Stored value:", account.value.toNumber());
  });
});
