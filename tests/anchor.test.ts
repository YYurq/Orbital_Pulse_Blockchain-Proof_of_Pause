import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { OrbitalPulse } from "../target/types/orbital_pulse";
import { 
  getAssociatedTokenAddressSync, 
  TOKEN_PROGRAM_ID, 
  ASSOCIATED_TOKEN_PROGRAM_ID 
} from "@solana/spl-token";

describe("orbital-pulse-final", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.OrbitalPulse as Program<OrbitalPulse>;
  const signer = provider.wallet.publicKey;
  const stateAccount = anchor.web3.Keypair.generate();

  it("Запуск детерминированного ядра с расчетом LEN", async () => {
    const [mintPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("orbital-genesis")],
      program.programId
    );
    const tokenAccount = getAssociatedTokenAddressSync(mintPda, signer, false, TOKEN_PROGRAM_ID, ASSOCIATED_TOKEN_PROGRAM_ID);

    // Инициализация: эпсилон 10M, порог градиента 3%
    await program.methods
      .initialize(new anchor.BN(10000000), new anchor.BN(3))
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

    const slotHashes = new anchor.web3.PublicKey("SysvarS1otHashes111111111111111111111111111");

    console.log("--- МОНИТОРИНГ СИСТЕМЫ ---");
    for (let i = 1; i <= 5; i++) {
      await program.methods
        .tryTransition()
        .accounts({
          state: stateAccount.publicKey,
          mint: mintPda,
          tokenAccount: tokenAccount,
          slotHashes: slotHashes,
          signer: signer,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .rpc();

      const state = await program.account.pulseState.fetch(stateAccount.publicKey);
      
      console.log(`[PULSE ${i}] Fine: ${state.lastFineLog} | EMA: ${state.varianceIndex} | Depth: ${state.currentDepth} | Threshold: ${state.gradientThresholdPercent}%`);
    }
  });
});
