import * as anchor from '@coral-xyz/anchor';
import { Program, BN, EventParser } from '@coral-xyz/anchor';
import { PublicKey, Keypair, SystemProgram, ComputeBudgetProgram } from '@solana/web3.js';
import { TOKEN_PROGRAM_ID, createMint, getAssociatedTokenAddress, mintTo, createAssociatedTokenAccount, NATIVE_MINT } from '@solana/spl-token';
import { assert } from 'chai';
import { Bridge } from '../target/types/bridge';
import { TokenPool } from '../target/types/token_pool';

describe('bridge', () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.Bridge as Program<Bridge>;
  const tokenPoolProgram = anchor.workspace.TokenPool as Program<TokenPool>;

  const admin = provider.wallet as anchor.Wallet;
  const ramp = Keypair.generate();
  const user = Keypair.generate();
  let usdcMint: Keypair;
  let bridgeConfigPDA: PublicKey;
  let bridgePDA: PublicKey;
  let receiptPDA: PublicKey;
  let rampPDA: PublicKey;
  let dailyLimitPDA: PublicKey;
  let rateLimitPDA: PublicKey;
  let upgradePDA: PublicKey;

  let poolConfigPDA: PublicKey;
  let usdcPoolPDA: PublicKey;

  let usdcPoolTokenAccount: PublicKey;
  let userUsdcTokenAccount: PublicKey;

  console.log(`   admin's public key: ${admin.publicKey}`);
  console.log(`   ramp's public key: ${ramp.publicKey}`);
  console.log(`   user's public key: ${user.publicKey}`);

  before(async () => {
    await provider.connection.requestAirdrop(admin.publicKey, 1e9 * 2);
    await provider.connection.requestAirdrop(user.publicKey, 1e9 * 2);
    await provider.connection.requestAirdrop(ramp.publicKey, 1e9 * 2);

    [bridgeConfigPDA] = await PublicKey.findProgramAddressSync(
        [Buffer.from('bridge_config')],
        program.programId
      );
    console.log(`   before start bridgeConfigPDA:${bridgeConfigPDA}`);

    [upgradePDA] = PublicKey.findProgramAddressSync(
        [Buffer.from("upgrade")],
        program.programId
    );
    console.log(`   before start upgradePDA:${upgradePDA}`);

    usdcMint = Keypair.generate();
    await createMint(
        provider.connection,
        admin.payer,
        admin.publicKey,
        null,
        6,
        usdcMint
    );
    console.log(`   before start usdcMint:${usdcMint.publicKey}`);

    [bridgePDA] = await PublicKey.findProgramAddressSync(
        [Buffer.from('bridge'), usdcMint.publicKey.toBuffer()],
        program.programId
      );
    console.log(`   before start bridgePDA:${bridgePDA}`);

    [poolConfigPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from("pool_config")],
        tokenPoolProgram.programId
    );
    console.log(`   before start poolConfigPDA:${poolConfigPDA}`);

    [usdcPoolPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from("pool"), usdcMint.publicKey.toBuffer()],
        tokenPoolProgram.programId
    );
    console.log(`   before start usdcPoolPDA:${usdcPoolPDA}`);

    [rampPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from("ramp"), usdcMint.publicKey.toBuffer()],
        program.programId
    );
    console.log(`   before start rampPDA:${rampPDA}`);

    usdcPoolTokenAccount = await getAssociatedTokenAddress(
        usdcMint.publicKey,
        usdcPoolPDA,
        true
      );
    console.log(`   before start usdcPoolTokenAccount:${usdcPoolTokenAccount}`);

    userUsdcTokenAccount = await createAssociatedTokenAccount(
      provider.connection,
      admin.payer,
      usdcMint.publicKey,
      user.publicKey
    );

    console.log(`   before start userUsdcTokenAccount:${userUsdcTokenAccount}`);

    await mintTo(
      provider.connection,
      admin.payer,
      usdcMint.publicKey,
      userUsdcTokenAccount,
      admin.publicKey,
      1000000000 // 1000 USDC
    );

      const tokenBalanceInfo = await provider.connection.getTokenAccountBalance(userUsdcTokenAccount);
      console.log(`   before start userUsdcTokenAccount balance:${tokenBalanceInfo.value.uiAmount}`);
      const balance = await provider.connection.getBalance(userUsdcTokenAccount);
      console.log("   before start userUsdcTokenAccount SOL balance:", balance);
  });

  it('Initialize success', async () => {
    await program.methods.initialize(
      admin.publicKey,
      admin.publicKey,
      tokenPoolProgram.programId,
      program.programId)
    .accounts({
      bridgeConfig: bridgeConfigPDA,
      admin: admin.publicKey,
      systemProgram: SystemProgram.programId
    })
    .signers([admin.payer])
    .rpc();

    const config = await program.account.bridgeConfig.fetch(bridgeConfigPDA);
    assert.isTrue(config.isInitialized);
    console.log(`   Initialize success: ${config.admin.toString()},${admin.publicKey.toString()}`);

  });

  it('Initialize fail', async () => {
    try {
      await program.methods.initialize(
          admin.publicKey,
          admin.publicKey,
          tokenPoolProgram.programId,
          ramp.publicKey,)
      .accounts({
          bridgeConfig: bridgeConfigPDA,
          admin: admin.publicKey,
          systemProgram: SystemProgram.programId
      })
      .signers([admin.payer])
      .rpc();
    } catch (e) {
      //assert.include(e.message, 'AlreadyInitialized');
      console.log(`   Initialize fail: ${e.message}`);
    }
  });

  function stringToFixedBytes(str: string, length: number): number[] {
    const buffer = Buffer.alloc(length, 0);
    const bytes = Buffer.from(str);
    bytes.copy(buffer, 0, 0, Math.min(bytes.length, length));
    return Array.from(buffer);
  }

  it('SetTokenWhitelist', async () => {
    const tokens = [
        {
          symbol: stringToFixedBytes("USDC", 16),
          chainId: 1,
        },
        {
          symbol: stringToFixedBytes("USDC", 16),
          chainId: 2,
        },
      ];

    await program.methods.setTokenWhitelist(tokens)
      .accounts({ 
        bridgeConfig: bridgeConfigPDA,
        admin: admin.publicKey,
        systemProgram: SystemProgram.programId
      })
      .signers([admin.payer])
      .rpc();

    const config = await program.account.bridgeConfig.fetch(bridgeConfigPDA);
    // assert.isTrue(config.tokenWhitelist.some(t => 
    //   t.symbol.equals(tokenInfo.symbol) && t.chainId === tokenInfo.chainId
    // ));
    console.log(`   SetTokenWhitelist: ${Buffer.from(config.tokenWhitelist[0].symbol).toString('utf8').replace(/\0/g, '')}, ${config.tokenWhitelist[0].chainId}`);
  });

  it('SetBridge', async () => {
    await program.methods.setBridge(usdcMint.publicKey)
      .accounts({ 
        bridgeConfig: bridgeConfigPDA,
        bridge: bridgePDA,
        admin: admin.publicKey,
        mint: usdcMint.publicKey,
        systemProgram: SystemProgram.programId
      })
      .signers([admin.payer])
      .rpc();

    const config = await program.account.bridge.fetch(bridgePDA);
    console.log(`   SetBridge: ${config.isInitialized}`);
  });

  let message: string;
  it('CreateReceipt success', async () => {
    const targetChainId = 2;
    [receiptPDA] = await PublicKey.findProgramAddressSync(
        [
            Buffer.from('receipt_info'),
            usdcMint.publicKey.toBuffer(),
            new BN(targetChainId).toArrayLike(Buffer, 'le', 2)
        ],
        program.programId
    );

    [dailyLimitPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from("daily_limit_config"), 
         usdcMint.publicKey.toBuffer(),
         new Uint8Array([0]),
         new BN(2).toArrayLike(Buffer, 'le', 2)],
        tokenPoolProgram.programId
    );

    [rateLimitPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from("rate_limit_config"), 
         usdcMint.publicKey.toBuffer(),
         new Uint8Array([0]),
         new BN(2).toArrayLike(Buffer, 'le', 2)],
        tokenPoolProgram.programId
      );

    console.log(`   CreateReceipt receiptPDA:${receiptPDA}`);
    console.log(`   CreateReceipt dailyLimitPDA:${dailyLimitPDA}`);
    console.log(`   CreateReceipt rateLimitPDA:${rateLimitPDA}`);
    console.log(`   CreateReceipt program.programId:${program.programId}`);
    console.log(`   CreateReceipt associatedTokenProgram:${anchor.utils.token.ASSOCIATED_PROGRAM_ID}`);
    console.log(`   CreateReceipt TOKEN_PROGRAM_ID:${TOKEN_PROGRAM_ID}`);
    console.log(`   CreateReceipt tokenPoolProgram.programId:${tokenPoolProgram.programId}`);
    console.log(`   CreateReceipt SystemProgram.programId:${SystemProgram.programId}`);

    await tokenPoolProgram.methods.initialize(admin.publicKey, program.programId)
        .accounts({
            poolConfig: poolConfigPDA,
            admin: admin.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
        })
        .signers([admin.payer])
        .rpc();

    const chainId = 2;
    const limitType = 0;
    const now = new Date();
    const today = Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate());
    const todaySec = today / 1000;
    console.log("   CreateReceipt Set Daily Limit Config:", todaySec);
    await tokenPoolProgram.methods.setDailyLimitConfig(
            chainId,
            limitType,
            new BN(todaySec),
            new BN(100000000) // 100 USDC daily limit
        )
        .accounts({
            poolConfig: poolConfigPDA,
            dailyLimitConfig: dailyLimitPDA,
            admin: admin.publicKey,
            mint: usdcMint.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
        })
        .signers([admin.payer])
        .rpc();

    await tokenPoolProgram.methods.setRateLimitConfig(
            chainId,
            limitType,
            true,
            new BN(10000000000), // 10000 USDC token_capacity
            new BN(10) // 10 rate
        )
        .accounts({
            poolConfig: poolConfigPDA,
            rateLimitConfig: rateLimitPDA,
            admin: admin.publicKey,
            mint: usdcMint.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
        })
        .signers([admin.payer])
        .rpc();
    await tokenPoolProgram.methods.setTokenPool(usdcMint.publicKey)
        .accounts({
            poolConfig: poolConfigPDA,
            pool: usdcPoolPDA,
            poolTokenAccount: usdcPoolTokenAccount,
            admin: admin.publicKey,
            mint: usdcMint.publicKey,
            associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID,
            tokenProgram: TOKEN_PROGRAM_ID,
            systemProgram: anchor.web3.SystemProgram.programId
        })
        .signers([admin.payer])
        .rpc();

    console.log("   CreateReceipt initialize token pool end");

    const tx = await program.methods.createReceipt(
        targetChainId,
        stringToFixedBytes("USDC", 16),
        "aaaa",
        new BN(1000000)
    ).accounts({
        bridgeConfig: bridgeConfigPDA,
        bridge: bridgePDA,
        receipt: receiptPDA,
        sender: user.publicKey,
        mint: usdcMint.publicKey,
        pool: usdcPoolPDA,
        poolConfig: poolConfigPDA,
        dailyLimitConfig: dailyLimitPDA,
        rateLimitConfig: rateLimitPDA,
        userTokenAccount: userUsdcTokenAccount,
        poolTokenAccount: usdcPoolTokenAccount,
        associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID,
        tokenProgram: TOKEN_PROGRAM_ID,
        poolProgram: tokenPoolProgram.programId,
        systemProgram: SystemProgram.programId
    })
    .signers([user])
    .rpc();

    const receipt = await program.account.crossChainReceipt.fetch(receiptPDA);
    // assert.equal(receipt.count.toNumber(), 1);
    console.log(`   CreateReceipt: ${receipt.count}`);
    console.log(`   CreateReceipt tx: ${tx}`);

    const tokenBalanceInfo = await provider.connection.getTokenAccountBalance(usdcPoolTokenAccount);
    console.log(`   CreateReceipt usdcPoolTokenAccount balance:${tokenBalanceInfo.value.uiAmount}`);
    const balance = await provider.connection.getBalance(usdcPoolTokenAccount);
    console.log("   CreateReceipt usdcPoolTokenAccount SOL balance:", balance);

    //parse event
    try {
        async function getTransactionWithRetry(signature: string, retries = 10) {
            for (let i = 0; i < retries; i++) {
              const response = await provider.connection.getTransaction(signature, {
                commitment: "confirmed",
                maxSupportedTransactionVersion: 0,
              });
              console.log(`   ⏳ Waiting for event (attempt ${i})...`);
              if (response) return response;
              await new Promise(resolve => setTimeout(resolve, 2000));
            }
            throw new Error("Transaction not found after retries");
        }

        interface TokenAmountEvent {
            targetChainId: number;
            targetContractAddress: string;
            tokenAddress: string;
            symbol: string;
            amount: BN;
        }

        interface RequestSendEvent {
            name: "requestSend";
            data: {
              targetChainId: number; 
              receiver: string;
              message: string;
              tokenAmount: TokenAmountEvent;
            };
        }

        const txResponse = await getTransactionWithRetry(tx);

        if (!txResponse) {
          throw new Error("Transaction not found");
        }

        const eventParser = new EventParser(program.programId, program.coder);
        const events = Array.from(eventParser.parseLogs(txResponse.meta.logMessages));
        const parsedEvents: RequestSendEvent[] = events
            .filter((e): e is RequestSendEvent => e.name === "requestSend")
            .map(event => {
              //const hexMessage = bytesToHex(event.data.message);
              message = event.data.message;
              const tokenAmount = event.data.tokenAmount as unknown as TokenAmountEvent;
              return {
                ...event,
                data: {
                ...event.data,
                message: bytesToHex(event.data.message as unknown as Uint8Array),
                tokenAmount: {
                    ...tokenAmount,
                    targetContractAddress: new PublicKey(tokenAmount.targetContractAddress).toBase58(),
                    tokenAddress: new PublicKey(tokenAmount.tokenAddress).toBase58(),
                },
              },
            };
        });

        function bytesToHex(bytes: Uint8Array): string {
            return Array.from(bytes)
              .map(byte => byte.toString(16).padStart(2, '0'))
              .join('');
        }

        console.log(
            "RequestSend Event Data:",
            JSON.stringify(parsedEvents, (key, value) => {
              if (typeof value === 'bigint' || BN.isBN(value)) {
                return value.toString(10);
              }
              return value;
            }, 2)
          );

      } catch (error) {
        console.log("Error fetching transaction:", error);
      }
  });

  it('CreateNativeReceipt success', async () => {
    const targetChainId = 2;
    const [nativeReceiptPDA] = await PublicKey.findProgramAddressSync(
        [
            Buffer.from('native_receipt_info'),
            Buffer.from("SOL"),
            new BN(targetChainId).toArrayLike(Buffer, 'le', 2)
        ],
        program.programId
    );

    const [nativeDailyLimitPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from("daily_limit_config"), 
        NATIVE_MINT.toBuffer(),
         new Uint8Array([0]),
         new BN(2).toArrayLike(Buffer, 'le', 2)],
        tokenPoolProgram.programId
    );

    const [nativeRateLimitPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from("rate_limit_config"), 
        NATIVE_MINT.toBuffer(),
         new Uint8Array([0]),
         new BN(2).toArrayLike(Buffer, 'le', 2)],
        tokenPoolProgram.programId
      );

    const [nativeBridgePDA] = await PublicKey.findProgramAddressSync(
        [Buffer.from('native_bridge'), Buffer.from("SOL")],
        program.programId
      );

    const [nativePoolPDA] = await PublicKey.findProgramAddressSync(
        [Buffer.from('native_pool')],
        tokenPoolProgram.programId
      );

    await tokenPoolProgram.methods.setNativePool()
        .accounts({
            poolConfig: poolConfigPDA,
            nativePool: nativePoolPDA,
            admin: admin.publicKey,
            systemProgram: SystemProgram.programId
        })
        .signers([admin.payer])
        .rpc();

    console.log(`   CreateNativeReceipt nativeReceiptPDA:${nativeReceiptPDA}`);
    console.log(`   CreateNativeReceipt nativeDailyLimitPDA:${nativeDailyLimitPDA}`);
    console.log(`   CreateNativeReceipt nativeRateLimitPDA:${nativeRateLimitPDA}`);
    console.log(`   CreateNativeReceipt nativeBridgePDA:${nativeBridgePDA}`);
    console.log(`   CreateNativeReceipt nativePoolPDA:${nativePoolPDA}`);
    const balance0 = await provider.connection.getBalance(nativePoolPDA);
    console.log("   CreateReceipt nativePoolPDA SOL balance:", balance0);

    const chainId = 2;
    const limitType = 0;
    const now = new Date();
    const today = Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate());
    const todaySec = today / 1000;
    console.log("   CreateNativeReceipt Set Daily Limit Config:", todaySec);
    await tokenPoolProgram.methods.setDailyLimitConfig(
            chainId,
            limitType,
            new BN(todaySec),
            new BN(100000000000) // 100 SOL daily limit
        )
        .accounts({
            poolConfig: poolConfigPDA,
            dailyLimitConfig: nativeDailyLimitPDA,
            admin: admin.publicKey,
            mint: NATIVE_MINT,
            systemProgram: anchor.web3.SystemProgram.programId,
        })
        .signers([admin.payer])
        .rpc();

    await tokenPoolProgram.methods.setRateLimitConfig(
            chainId,
            limitType,
            true,
            new BN(100000000000), // 100 SOL token_capacity
            new BN(10) // 10 rate
        )
        .accounts({
            poolConfig: poolConfigPDA,
            rateLimitConfig: nativeRateLimitPDA,
            admin: admin.publicKey,
            mint: NATIVE_MINT,
            systemProgram: anchor.web3.SystemProgram.programId,
        })
        .signers([admin.payer])
        .rpc();

        console.log(`   CreateNativeReceipt set limit over!`);

    const tx = await program.methods.createNativeReceipt(
        targetChainId,
        "aaaa",
        new BN(1000000000)
    ).accounts({
        bridgeConfig: bridgeConfigPDA,
        bridge: nativeBridgePDA,
        receipt: nativeReceiptPDA,
        sender: user.publicKey,
        mint: NATIVE_MINT,
        // mint: usdcMint.publicKey,
        nativePool: nativePoolPDA,
        poolConfig: poolConfigPDA,
        dailyLimitConfig: nativeDailyLimitPDA,
        rateLimitConfig: nativeRateLimitPDA,
        poolProgram: tokenPoolProgram.programId,
        systemProgram: SystemProgram.programId
    })
    .signers([user])
    .rpc();

    const receipt = await program.account.crossChainReceipt.fetch(nativeReceiptPDA);
    // assert.equal(receipt.count.toNumber(), 1);
    console.log(`   CreateReceipt: ${receipt.count}`);
    console.log(`   CreateReceipt tx: ${tx}`);

    const balance = await provider.connection.getBalance(nativePoolPDA);
    console.log("   CreateReceipt nativePoolPDA SOL balance:", balance);

    //parse event
    try {
        async function getTransactionWithRetry(signature: string, retries = 10) {
            for (let i = 0; i < retries; i++) {
              const response = await provider.connection.getTransaction(signature, {
                commitment: "confirmed",
                maxSupportedTransactionVersion: 0,
              });
              console.log(`   ⏳ Waiting for event (attempt ${i})...`);
              if (response) return response;
              await new Promise(resolve => setTimeout(resolve, 2000));
            }
            throw new Error("Transaction not found after retries");
        }

        interface TokenAmountEvent {
            targetChainId: number;
            targetContractAddress: string;
            tokenAddress: string;
            symbol: string;
            amount: BN;
        }

        interface RequestSendEvent {
            name: "requestSend";
            data: {
              targetChainId: number; 
              receiver: string;
              message: string;
              tokenAmount: TokenAmountEvent;
            };
        }

        const txResponse = await getTransactionWithRetry(tx);

        if (!txResponse) {
          throw new Error("Transaction not found");
        }

        const eventParser = new EventParser(program.programId, program.coder);
        const events = Array.from(eventParser.parseLogs(txResponse.meta.logMessages));
        const parsedEvents: RequestSendEvent[] = events
            .filter((e): e is RequestSendEvent => e.name === "requestSend")
            .map(event => {
              //const hexMessage = bytesToHex(event.data.message);
              const tokenAmount = event.data.tokenAmount as unknown as TokenAmountEvent;
              return {
                ...event,
                data: {
                ...event.data,
                message: bytesToHex(event.data.message as unknown as Uint8Array),
                tokenAmount: {
                    ...tokenAmount,
                    targetContractAddress: new PublicKey(tokenAmount.targetContractAddress).toBase58(),
                    tokenAddress: new PublicKey(tokenAmount.tokenAddress).toBase58(),
                },
              },
            };
        });

        function bytesToHex(bytes: Uint8Array): string {
            return Array.from(bytes)
              .map(byte => byte.toString(16).padStart(2, '0'))
              .join('');
        }

        console.log(
            "RequestSend Event Data:",
            JSON.stringify(parsedEvents, (key, value) => {
              if (typeof value === 'bigint' || BN.isBN(value)) {
                return value.toString(10);
              }
              return value;
            }, 2)
          );

      } catch (error) {
        console.log("Error fetching transaction:", error);
      }
  });

  it('CreateReceipt fail', async () => {
    try {
        await program.methods.createReceipt(
            3,
            stringToFixedBytes("USDC", 16),
            user.publicKey.toString(),
            new BN(1000000)
        ).accounts({
            bridgeConfig: bridgeConfigPDA,
            bridge: bridgePDA,
            receipt: receiptPDA,
            sender: user.publicKey,
            mint: usdcMint.publicKey,
            pool: usdcPoolPDA,
            poolConfig: poolConfigPDA,
            dailyLimitConfig: dailyLimitPDA,
            rateLimitConfig: rateLimitPDA,
            userTokenAccount: userUsdcTokenAccount,
            poolTokenAccount: usdcPoolTokenAccount,
            associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID,
            tokenProgram: TOKEN_PROGRAM_ID,
            poolProgram: tokenPoolProgram.programId,
            systemProgram: SystemProgram.programId
        })
        .signers([user])
        .rpc();
    } catch (e) {
        console.log(`   CreateReceipt fail: ${e.message}`);
    }
  });

  it('ForwardMessage success', async () => {
    await program.methods.setCrossChainConfig(1, program.programId.toString())
        .accounts({
            bridgeConfig: bridgeConfigPDA,
            admin: admin.publicKey,
            systemProgram: SystemProgram.programId
        })
        .signers([admin.payer])
        .rpc();

    console.log("   ForwardMessage setCrossChainConfig end");

    [dailyLimitPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from("daily_limit_config"), 
            usdcMint.publicKey.toBuffer(),
            new Uint8Array([1]),
            new BN(1).toArrayLike(Buffer, 'le', 2)],
        tokenPoolProgram.programId
    );

    [rateLimitPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from("rate_limit_config"), 
            usdcMint.publicKey.toBuffer(),
            new Uint8Array([1]),
            new BN(1).toArrayLike(Buffer, 'le', 2)],
        tokenPoolProgram.programId
        );
    console.log(`   ForwardMessage dailyLimitPDA:${dailyLimitPDA}`);
    console.log(`   ForwardMessage rateLimitPDA:${rateLimitPDA}`);

    const chainId = 1;
    const limitType = 1;
    const now = new Date();
    const today = Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate());
    const todaySec = today / 1000;
    console.log("   ForwardMessage Set Daily Limit Config:", todaySec);
    await tokenPoolProgram.methods.setDailyLimitConfig(
            chainId,
            limitType,
            new BN(todaySec),
            new BN(100000000) // 100 USDC daily limit
        )
        .accounts({
            poolConfig: poolConfigPDA,
            dailyLimitConfig: dailyLimitPDA,
            admin: admin.publicKey,
            mint: usdcMint.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
        })
        .signers([admin.payer])
        .rpc();

    await tokenPoolProgram.methods.setRateLimitConfig(
            chainId,
            limitType,
            true,
            new BN(10000000000), // 10000 USDC token_capacity
            new BN(10) // 10 rate
        )
        .accounts({
            poolConfig: poolConfigPDA,
            rateLimitConfig: rateLimitPDA,
            admin: admin.publicKey,
            mint: usdcMint.publicKey,
            systemProgram: anchor.web3.SystemProgram.programId,
        })
        .signers([admin.payer])
        .rpc();
    console.log("   ForwardMessage Set Limit Config end");

    const messageBuffer = Buffer.from(message, 'utf8');
    const fixed160Buffer = Buffer.alloc(160, 0); 
    messageBuffer.copy(fixed160Buffer, 0, 0, Math.min(messageBuffer.length, 160));
    const receiptHashBuffer = fixed160Buffer.subarray(128, 160);

    const [receiptRecordPDA] = PublicKey.findProgramAddressSync(
        [Buffer.from("receipt_record"), receiptHashBuffer],
        program.programId
    );
    console.log(`   ForwardMessage receiptRecordPDA: ${receiptRecordPDA}`);

    await program.methods.forwardMessage(
      1,
      Array.from(receiptHashBuffer),
      2,
      Buffer.from("aaaa"),
      Buffer.alloc(160).fill(message),
      {
        targetChainId: 1,
        targetContractAddress: program.programId,
        tokenAddress: usdcMint.publicKey,
        symbol: "USDC",
        amount: new BN(1000000)
      }
    ).accounts({
        bridgeConfig: bridgeConfigPDA,
        bridge: bridgePDA,
        //receipt: receiptPDA,
        receiptRecord: receiptRecordPDA,
        sender: ramp.publicKey,
        ramp: rampPDA,
        mint: usdcMint.publicKey,
        pool: usdcPoolPDA,
        poolConfig: poolConfigPDA,
        dailyLimitConfig: dailyLimitPDA,
        rateLimitConfig: rateLimitPDA,
        userTokenAccount: userUsdcTokenAccount,
        poolTokenAccount: usdcPoolTokenAccount,
        associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID,
        tokenProgram: TOKEN_PROGRAM_ID,
        poolProgram: tokenPoolProgram.programId,
        systemProgram: SystemProgram.programId
    })
    .signers([ramp])
    .rpc();

    const tokenBalanceInfo = await provider.connection.getTokenAccountBalance(usdcPoolTokenAccount);
    console.log(`   ForwardMessage usdcPoolTokenAccount balance:${tokenBalanceInfo.value.uiAmount}`);
    const balance = await provider.connection.getBalance(usdcPoolTokenAccount);
    console.log("   ForwardMessage usdcPoolTokenAccount SOL balance:", balance);

    const receiptRecord = await program.account.receiptRecord.fetch(receiptRecordPDA);
    console.log(`   ForwardMessage1: ${receiptRecord.isInitialized},${receiptRecord.receiptHash}`);

    try {
        await program.methods.forwardMessage(
            1,
            Array.from(receiptHashBuffer),
            2,
            Buffer.from("aaaa"),
            Buffer.alloc(160).fill(message),
            {
              targetChainId: 1,
              targetContractAddress: program.programId,
              tokenAddress: usdcMint.publicKey,
              symbol: "USDC",
              amount: new BN(1000000)
            }
          ).accounts({
              bridgeConfig: bridgeConfigPDA,
              bridge: bridgePDA,
              //receipt: receiptPDA,
              receiptRecord: receiptRecordPDA,
              sender: ramp.publicKey,
              ramp: rampPDA,
              mint: usdcMint.publicKey,
              pool: usdcPoolPDA,
              poolConfig: poolConfigPDA,
              dailyLimitConfig: dailyLimitPDA,
              rateLimitConfig: rateLimitPDA,
              userTokenAccount: userUsdcTokenAccount,
              poolTokenAccount: usdcPoolTokenAccount,
              associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID,
              tokenProgram: TOKEN_PROGRAM_ID,
              poolProgram: tokenPoolProgram.programId,
              systemProgram: SystemProgram.programId
          })
          .signers([ramp])
          .rpc();
    } catch (e) {
      console.log(`   ForwardMessage fail: ${e.message}`);
    }
  });

  it('Pause', async () => {
    try {
      await program.methods.pause()
        .accounts({ admin: user.publicKey })
        .signers([user])
        .rpc();
    } catch (e) {
      //assert.include(e.message, 'Unauthorized');
      console.log(`   Pause: ${e.message}`);
    }

    await program.methods.pause()
      .accounts({ admin: admin.publicKey })
      .signers([admin.payer])
      .rpc();
    const config = await program.account.bridgeConfig.fetch(bridgeConfigPDA);
    console.log(`   Pause: ${config.isContractPause}`);
  });

  it('Restart', async () => {
    await program.methods.restart()
      .accounts({ admin: admin.publicKey })
      .signers([admin.payer])
      .rpc();

    const config = await program.account.bridgeConfig.fetch(bridgeConfigPDA);
    assert.isFalse(config.isContractPause);
    console.log(`   Restart: ${config.isContractPause}`);
  });

});