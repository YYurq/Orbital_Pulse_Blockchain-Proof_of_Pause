import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { OrbitalPulse } from "../target/types/orbital_pulse";
import { getAssociatedTokenAddressSync, TOKEN_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID } from "@solana/spl-token";

describe("orbital-pulse-final", function () {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.OrbitalPulse as Program<OrbitalPulse>;
  const signer = provider.wallet.publicKey;
  const stateAccount = anchor.web3.Keypair.generate();

  this.timeout(120000);

  it("Full Initialization and Stabilization Protocol", async function () {
    const [mintPda] = anchor.web3.PublicKey.findProgramAddressSync([Buffer.from("orbital-genesis")], program.programId);
    const tokenAccount = getAssociatedTokenAddressSync(mintPda, signer, false, TOKEN_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID);

    await program.methods
      .initialize(new anchor.BN(3))
      .accounts({
        state: stateAccount.publicKey,
        mint: mintPda,
        tokenAccount: tokenAccount,
        signer: signer,
        systemProgram: anchor.web3.SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: ASSOCIATED_TOKEN_PROGRAM_ID,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      })
      .signers([stateAccount])
      .rpc();

    const modes = ["S0 (IDLE)", "S1 (CONTROL)", "S2 (EVOLVE)"];
    for (let i = 1; i <= 30; i++) {
      await program.methods.tryTransition().accounts({
        state: stateAccount.publicKey,
        mint: mintPda,
        tokenAccount: tokenAccount,
        slotHashes: new anchor.web3.PublicKey("SysvarS1otHashes111111111111111111111111111"),
        signer: signer,
        tokenProgram: TOKEN_PROGRAM_ID,
      }).rpc();

      const state = await program.account.pulseState.fetch(stateAccount.publicKey);
      
      if (!state.isBorn) {
        console.log(`[T=${i.toString().padStart(2, '0')}] CALIBRATION: Collecting entropy... ${state.calibCount}/16`);
      } else {
        const varIdx = BigInt(state.varianceIndex.toString());
        const eps = BigInt(state.epsilon.toString());
        const xVal = BigInt(state.xControl.toString());
        const xMax = varIdx > eps ? varIdx : (eps > 0n ? eps : 1n);
        const load = (Number(xVal * 1000n / xMax) / 10).toFixed(1);

        console.log(
          `[T=${i.toString().padStart(2, '0')}] ${modes[state.mode] || "S0"} | ` +
          `Vacuum: ${eps.toString().padEnd(20)} | ` +
          `Energy: ${varIdx.toString().padEnd(10)} | ` +
          `Control_X: ${load}%`
        );
      }
    }
  });
});
