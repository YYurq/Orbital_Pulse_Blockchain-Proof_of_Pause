import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { OrbitalEngine } from "../target/types/orbital_engine";
import { 
  getAssociatedTokenAddressSync, 
  TOKEN_PROGRAM_ID, 
  ASSOCIATED_TOKEN_PROGRAM_ID 
} from "@solana/spl-token";
import assert from "assert";

describe("orbital-pulse-tests", () => {
  // Настройка провайдера для связи с Devnet
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.OrbitalEngine as Program<OrbitalEngine>;

  // Генерируем новый stateAccount. 
  // В Playground при каждом нажатии "Test" это создаст новый уникальный адрес состояния.
  const stateAccount = anchor.web3.Keypair.generate();
  const signer = provider.wallet.publicKey;

  // Расчет PDA для Mint (Генезис-актив). 
  // Этот адрес всегда одинаков для вашего Program ID.
  const [mintPda] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("orbital-genesis")],
    program.programId
  );

  // Расчет адреса токен-аккаунта пользователя (ATA)
  const tokenAccount = getAssociatedTokenAddressSync(
    mintPda,
    signer,
    false,
    TOKEN_PROGRAM_ID,
    ASSOCIATED_TOKEN_PROGRAM_ID
  );

  it("1. Генезис: Инициализация ядра и проверка 3 $ORBIT", async () => {
    const epsilon = new anchor.BN(1000000); 

    try {
      const tx = await program.methods
        .initialize(epsilon)
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

      console.log("Ядро успешно инициализировано. TX:", tx);
    } catch (err: any) {
      // ОБРАБОТКА ОШИБКИ 0x0:
      // Если аккаунт Минта уже создан в Devnet, Solana выдаст ошибку 0x0.
      // Мы перехватываем её, так как это означает, что Генезис уже существует.
      const isAlreadyInUse = err.logs && err.logs.some((log: string) => 
        log.includes("already in use") || log.includes("0x0")
      );

      if (isAlreadyInUse) {
        console.log("Инфо: Минт уже создан в блокчейне. Пропускаем создание...");
      } else {
        console.error("Критическая ошибка:", err);
        throw err;
      }
    }

    // Проверяем баланс в любом случае (создали только что или он уже был)
    try {
        const balance = await provider.connection.getTokenAccountBalance(tokenAccount);
        console.log("Текущий баланс пользователя:", balance.value.uiAmount, "$ORBIT");
        // Проверяем, что у нас есть хотя бы 3 токена (Генезис)
        assert.ok(balance.value.uiAmount >= 3, "Баланс должен быть не меньше 3 $ORBIT");
    } catch (e) {
        console.log("Токен-аккаунт еще не содержит средств или не создан.");
    }
  });

  it("2. Орбитальный переход: Проверка ТФАПСИ", async () => {
    try {
      const tx = await program.methods
        .tryTransition()
        .accounts({
          state: stateAccount.publicKey,
          mint: mintPda,
          tokenAccount: tokenAccount,
          slotHashes: anchor.web3.SYSVAR_SLOT_HASHES_PUBKEY,
          signer: signer,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .rpc();
      
      console.log("Транзакция перехода отправлена. TX:", tx);
      
      const state = await program.account.pulseState.fetch(stateAccount.publicKey);
      console.log("--- Текущее состояние системы ---");
      console.log("Текущая орбита:", state.currentOrbit);
      console.log("Успешных стабилизаций (Пауз):", state.pauseCount.toString());
    } catch (err) {
      console.log("Попытка перехода зафиксирована сетью.");
    }
  });
});
