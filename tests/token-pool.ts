// import * as anchor from '@coral-xyz/anchor';
// import { Program, BN } from '@coral-xyz/anchor';
// import { TokenPool } from '../target/types/token_pool';
// import { Bridge } from '../target/types/bridge';
// import { PublicKey, Keypair, SystemProgram, SYSVAR_RENT_PUBKEY } from '@solana/web3.js';
// import { TOKEN_PROGRAM_ID, createMint, getAssociatedTokenAddress, mintTo, createAssociatedTokenAccount, getAccount } from '@solana/spl-token';
// import { assert, expect } from 'chai';
// import { readFileSync } from "fs";
// import { publicKey } from '@coral-xyz/anchor/dist/cjs/utils';

// describe('token-pool', () => {
//   const provider = anchor.AnchorProvider.env();
//   anchor.setProvider(provider);
//   const program = anchor.workspace.TokenPool as Program<TokenPool>;
//   const bridgeProgram = anchor.workspace.Bridge as Program<Bridge>;

//   const admin = provider.wallet as anchor.Wallet;
//   const user = admin;
//   const bridgeContract = Keypair.generate();
//   console.log(`   --------------------------------------`);
//   console.log(`   admin's public key: ${admin.publicKey}`);
//   console.log(`   user's public key: ${user.publicKey}`);
//   console.log(`   bridgeContract's public key: ${bridgeContract.publicKey}`);
//   let usdcMint: Keypair;
 
//   let poolConfigPDA: PublicKey;
//   let usdcPoolPDA: PublicKey;
//   let upgradePDA: PublicKey;

//   let bridgeConfigPDA: PublicKey;
//   let bridgePDA: PublicKey;
 
//   let usdcPoolTokenAccount: PublicKey;
//   let userUsdcTokenAccount: PublicKey;
//   let userUsdcTokenAccount2: PublicKey;

//   before(async () => {
//     console.log(`   before start`);
//     await provider.connection.requestAirdrop(admin.publicKey, 1e9 * 2);
//     await provider.connection.requestAirdrop(user.publicKey, 1e9 * 2);
   
//     usdcMint = Keypair.generate();
//     await createMint(
//       provider.connection,
//       admin.payer,
//       admin.publicKey, // mintAuthority
//       null, // freezeAuthority
//       6,
//       usdcMint
//     );
//     console.log(`   before start usdcMint:${usdcMint.publicKey}`);

//     [poolConfigPDA] = PublicKey.findProgramAddressSync(
//       [Buffer.from("pool_config")],
//       program.programId
//     );
//     console.log(`   before start poolConfigPDA:${poolConfigPDA}`);

//     [usdcPoolPDA] = PublicKey.findProgramAddressSync(
//       [Buffer.from("pool"), usdcMint.publicKey.toBuffer()],
//       program.programId
//     );
//     console.log(`   before start usdcPoolPDA:${usdcPoolPDA}`);

//     [upgradePDA] = PublicKey.findProgramAddressSync(
//         [Buffer.from("upgrade")],
//         program.programId
//       );
//     console.log(`   before start upgradePDA:${upgradePDA}`);

//     usdcPoolTokenAccount = await getAssociatedTokenAddress(
//         usdcMint.publicKey,
//         usdcPoolPDA,
//         true
//       );
//     console.log(`   before start usdcPoolTokenAccount:${usdcPoolTokenAccount}`);

//     userUsdcTokenAccount = await createAssociatedTokenAccount(
//       provider.connection,
//       admin.payer,
//       usdcMint.publicKey,
//       bridgeContract.publicKey
//     );

//     console.log(`   before start userUsdcTokenAccount:${userUsdcTokenAccount}`);
   
//     await mintTo(
//       provider.connection,
//       admin.payer,
//       usdcMint.publicKey,
//       userUsdcTokenAccount,
//       admin.publicKey,
//       1000000000 // 1000 USDC
//     );

//     userUsdcTokenAccount2 = await createAssociatedTokenAccount(
//         provider.connection,
//         admin.payer,
//         usdcMint.publicKey,
//         user.publicKey
//       );
 
//       console.log(`   before start userUsdcTokenAccount2:${userUsdcTokenAccount2}`);
     
//       await mintTo(
//         provider.connection,
//         admin.payer,
//         usdcMint.publicKey,
//         userUsdcTokenAccount2,
//         admin.publicKey,
//         1000000000 // 1000 USDC
//       );

//       const tokenBalanceInfo = await provider.connection.getTokenAccountBalance(userUsdcTokenAccount);
//       console.log(`   before start userUsdcTokenAccount balance:${tokenBalanceInfo.value.uiAmount}`);
//       const balance = await provider.connection.getBalance(userUsdcTokenAccount);
//       console.log("   before start userUsdcTokenAccount SOL balance:", balance);

//       const tokenBalanceInfo2 = await provider.connection.getTokenAccountBalance(userUsdcTokenAccount2);
//       console.log(`   before start userUsdcTokenAccount2 balance:${tokenBalanceInfo2.value.uiAmount}`);
//       const balance2 = await provider.connection.getBalance(userUsdcTokenAccount2);
//       console.log("   before start userUsdcTokenAccount2 SOL balance:", balance2);
//   });

//   it('Initialize', async () => {
//     await program.methods.initialize(admin.publicKey, bridgeContract.publicKey)
//       .accounts({
//         poolConfig: poolConfigPDA,
//         admin: admin.publicKey,
//         systemProgram: SystemProgram.programId
//       })
//       .signers([admin.payer])
//       .rpc();

//     const config = await program.account.poolConfig.fetch(poolConfigPDA);
//     assert.isTrue(config.isInitialized);
//     assert.equal(config.admin.toString(), admin.publicKey.toString());
//   });

//   it('Set Token Pool', async () => {
//     const tx = await program.methods.setTokenPool(usdcMint.publicKey)
//       .accounts({
//         poolConfig: poolConfigPDA,
//         pool: usdcPoolPDA,
//         poolTokenAccount: usdcPoolTokenAccount,
//         admin: admin.publicKey,
//         mint: usdcMint.publicKey,
//         associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID,
//         tokenProgram: TOKEN_PROGRAM_ID,
//         systemProgram: SystemProgram.programId
//       })
//       .signers([admin.payer])
//       .rpc();

//     const pool = await program.account.pool.fetch(usdcPoolPDA);
//     assert.isTrue(pool.isInitialized);
//     console.log(`   Set Token Pool:${pool.isInitialized}`);
   
//     console.log(`   Set Token Pool usdcPoolTokenAccount:${usdcPoolTokenAccount}`);
//     const tokenAccount = await getAccount(provider.connection, usdcPoolTokenAccount);
//     console.log(`   Set Token Pool:${tokenAccount.owner.toString()},${usdcPoolPDA.toString()}`);
//     assert.equal(tokenAccount.owner.toString(), usdcPoolPDA.toString());
//   });

//   it('Lock', async () => {
//     const amount = new BN(1000000); // 1 USDC
   
//     await provider.connection.requestAirdrop(userUsdcTokenAccount, 1e9 * 2);
//     await provider.connection.requestAirdrop(bridgeContract.publicKey, 1e9 * 2);

//     [bridgeConfigPDA] = PublicKey.findProgramAddressSync(
//       [Buffer.from('bridge_config')],
//       bridgeProgram.programId
//     );
//     [bridgePDA] = PublicKey.findProgramAddressSync(
//       [Buffer.from('bridge'), usdcMint.publicKey.toBuffer()],
//       bridgeProgram.programId
//     );

//     await bridgeProgram.methods.initialize(
//       admin.publicKey,
//       admin.publicKey,
//       program.programId,
//       bridgeProgram.programId)
//     .accounts({
//       bridgeConfig: bridgeConfigPDA,
//       admin: admin.publicKey,
//       systemProgram: SystemProgram.programId
//     })
//     .signers([admin.payer])
//     .rpc();

//     await bridgeProgram.methods.setBridge(usdcMint.publicKey)
//       .accounts({ 
//         bridgeConfig: bridgeConfigPDA,
//         bridge: bridgePDA,
//         admin: admin.publicKey,
//         mint: usdcMint.publicKey,
//         systemProgram: SystemProgram.programId
//       })
//       .signers([admin.payer])
//       .rpc();

//     // await program.methods.lock(amount)
//     //   .accounts({
//     //     poolConfig: poolConfigPDA,
//     //     pool: usdcPoolPDA,
//     //     poolTokenAccount: usdcPoolTokenAccount,
//     //     userTokenAccount: userUsdcTokenAccount,
//     //     owner: bridgeContract.publicKey,
//     //     authority: bridgePDA,
//     //     mint: usdcMint.publicKey,
//     //     associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID,
//     //     tokenProgram: TOKEN_PROGRAM_ID,
//     //     systemProgram: SystemProgram.programId
//     //   })
//     //   .signers([bridgeContract])
//     //   .rpc();

//     const pool = await program.account.pool.fetch(usdcPoolPDA);
//     //assert.equal(pool.totalAmount.toString(), amount.toString());

//     console.log(`   Lock amount:${pool.totalAmount.toString()},${amount.toString()}`);
//     console.log(`   Lock userUsdcTokenAccount:${userUsdcTokenAccount}`);
//     const tokenAccount = await getAccount(provider.connection, userUsdcTokenAccount);
//     console.log(`   Lock:${tokenAccount.owner.toString()},${bridgeContract.publicKey}`);

//     const tokenBalanceInfo = await provider.connection.getTokenAccountBalance(usdcPoolTokenAccount);
//     console.log(`   Lock usdcPoolTokenAccount balance:${tokenBalanceInfo.value.uiAmount}`);
//     const balance = await provider.connection.getBalance(usdcPoolTokenAccount);
//     console.log("   Lock usdcPoolTokenAccount SOL balance:", balance);

//     const tokenBalanceInfo2 = await provider.connection.getTokenAccountBalance(userUsdcTokenAccount);
//     console.log(`   Lock userUsdcTokenAccount balance:${tokenBalanceInfo2.value.uiAmount}`);
//     const balance2 = await provider.connection.getBalance(usdcPoolTokenAccount);
//     console.log("   Lock userUsdcTokenAccount SOL balance:", balance2);
//   });

//   // it('Release', async () => {
//   //   const amount = new BN(400000); // 0.4 USDC
   
//   //   await provider.connection.requestAirdrop(usdcPoolTokenAccount, 1e9 * 2);
//   //   await provider.connection.requestAirdrop(bridgeContract.publicKey, 1e9 * 2);

//   //   await program.methods.release(amount)
//   //     .accounts({
//   //       poolConfig: poolConfigPDA,
//   //       pool: usdcPoolPDA,
//   //       poolTokenAccount: usdcPoolTokenAccount,
//   //       userTokenAccount: userUsdcTokenAccount,
//   //       authority: bridgePDA,
//   //       mint: usdcMint.publicKey,
//   //       tokenProgram: TOKEN_PROGRAM_ID
//   //     })
//   //     .signers([bridgeContract])
//   //     .rpc();

//   //   const pool = await program.account.pool.fetch(usdcPoolPDA);
//   //   assert.equal(pool.totalAmount.toString(), "600000");

//   //   console.log(`   Release amount:${pool.totalAmount.toString()},${amount.toString()}`);
//   // });

//   it('Add Liquidity', async () => {
//     const amount = new BN(500000); // 0.5 USDC

//     await provider.connection.requestAirdrop(userUsdcTokenAccount2, 1e9 * 2);
//     await provider.connection.requestAirdrop(user.publicKey, 1e9 * 2);

//     const [userPoolPDA] = PublicKey.findProgramAddressSync(
//       [Buffer.from("user_pool"), user.publicKey.toBuffer(), usdcMint.publicKey.toBuffer()],
//       program.programId
//     );

//     const tokenBalanceInfo = await provider.connection.getTokenAccountBalance(userUsdcTokenAccount2);
//     console.log(`   Add Liquidity userUsdcTokenAccount2 balance:${tokenBalanceInfo.value.uiAmount}`);
//     const balance = await provider.connection.getBalance(userUsdcTokenAccount2);
//     console.log("   Add Liquidity userUsdcTokenAccount2 SOL balance:", balance);

//     await program.methods.addLiquidity(amount)
//       .accounts({
//         poolConfig: poolConfigPDA,
//         userPool: userPoolPDA,
//         pool: usdcPoolPDA,
//         poolTokenAccount: usdcPoolTokenAccount,
//         userTokenAccount: userUsdcTokenAccount2,
//         sender: user.publicKey,
//         mint: usdcMint.publicKey,
//         associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID,
//         tokenProgram: TOKEN_PROGRAM_ID,
//         systemProgram: SystemProgram.programId
//       })
//       .signers([user.payer])
//       .rpc();

//     const userPool = await program.account.userPool.fetch(userPoolPDA);
//     assert.equal(userPool.liquidity.toString(), amount.toString());
//     console.log(`   Add Liquidity amount:${userPool.liquidity.toString()},${amount.toString()}`);
//     console.log(`   Add Liquidity userUsdcTokenAccount2:${userUsdcTokenAccount2}`);
//     const tokenAccount = await getAccount(provider.connection, userUsdcTokenAccount2);
//     console.log(`   Add Liquidity:${tokenAccount.owner.toString()},${user.publicKey}`);

//     const tokenBalanceInfo3 = await provider.connection.getTokenAccountBalance(usdcPoolTokenAccount);
//     console.log(`   Add Liquidity usdcPoolTokenAccount balance:${tokenBalanceInfo3.value.uiAmount}`);
//     const balance3 = await provider.connection.getBalance(usdcPoolTokenAccount);
//     console.log("   Add Liquidity usdcPoolTokenAccount SOL balance:", balance3);

//     const tokenBalanceInfo4 = await provider.connection.getTokenAccountBalance(userUsdcTokenAccount2);
//     console.log(`   Add Liquidity userUsdcTokenAccount2 balance:${tokenBalanceInfo4.value.uiAmount}`);
//     const balance4 = await provider.connection.getBalance(userUsdcTokenAccount2);
//     console.log("   Add Liquidity userUsdcTokenAccount2 SOL balance:", balance4);
//   });

//   it('Add Native Liquidity', async () => {
//     const amount = new BN(200000000); // 0.2 SOL

//     await provider.connection.requestAirdrop(user.publicKey, 1e9 * 2);

//     const [userNativePoolPDA] = PublicKey.findProgramAddressSync(
//       [Buffer.from("user_native_pool"), user.publicKey.toBuffer()],
//       program.programId
//     );

//     const [nativePoolPDA] = PublicKey.findProgramAddressSync(
//       [Buffer.from("native_pool")],
//       program.programId
//     );

//     console.log(`   Add Native Liquidity nativePoolPDA:${nativePoolPDA}`);
//     const beforeBalance= await provider.connection.getBalance(nativePoolPDA);
//     console.log(`   Add Native Liquidity nativePool SOL before:${beforeBalance}`);

//     const beforeUserBalance = await provider.connection.getBalance(user.publicKey);
//     console.log("   Add Native Liquidity user SOL before:", beforeUserBalance);

//     await program.methods.setNativePool()
//       .accounts({
//         poolConfig: poolConfigPDA,
//         nativePool: nativePoolPDA,
//         admin: admin.publicKey,
//         systemProgram: SystemProgram.programId
//       })
//       .signers([admin.payer])
//       .rpc();

//     console.log("   Add Native Liquidity setNativePool");

//     await program.methods.addNativeLiquidity(amount)
//       .accounts({
//         poolConfig: poolConfigPDA,
//         userNativePool: userNativePoolPDA,
//         nativePool: nativePoolPDA,
//         sender: user.publicKey,
//         systemProgram: SystemProgram.programId
//       })
//       .signers([user.payer])
//       .rpc();

//     const userNativePool = await program.account.userNativePool.fetch(userNativePoolPDA);
//     console.log(`   Add Native Liquidity userNativePool:${userNativePool.liquidity.toString()},${amount.toString()}`);

//     const afterPoolBalance= await provider.connection.getBalance(nativePoolPDA);
//     console.log(`   Add Native Liquidity nativePool SOL after:${afterPoolBalance}`);

//     const afterUserBalance = await provider.connection.getBalance(user.publicKey);
//     console.log("   Add Native Liquidity user SOL after:", afterUserBalance);
//   });

//   it('Remove Liquidity', async () => {
//     const amount = new BN(200000); // 0.2 USDC

//     await provider.connection.requestAirdrop(usdcPoolTokenAccount, 1e9 * 2);
//     await provider.connection.requestAirdrop(user.publicKey, 1e9 * 2);

//     const [userPoolPDA] = PublicKey.findProgramAddressSync(
//       [Buffer.from("user_pool"), user.publicKey.toBuffer(), usdcMint.publicKey.toBuffer()],
//       program.programId
//     );
//     console.log(`   Remove Liquidity userPoolPDA:${userPoolPDA}`);

//     await program.methods.removeLiquidity(amount)
//       .accounts({
//         poolConfig: poolConfigPDA,
//         userPool: userPoolPDA,
//         pool: usdcPoolPDA,
//         poolTokenAccount: usdcPoolTokenAccount,
//         userTokenAccount: userUsdcTokenAccount2,
//         sender: user.publicKey,
//         tokenProgram: TOKEN_PROGRAM_ID,
//         systemProgram: SystemProgram.programId
//       })
//       .signers([user.payer])
//       .rpc();

//     const userPool = await program.account.userPool.fetch(userPoolPDA);
//     //assert.equal(userPool.liquidity.toString(), amount.toString());
//     console.log(`   Remove Liquidity amount:${userPool.liquidity.toString()},${amount.toString()}`);
//   });

//   async function fundPdaAccount(pda: PublicKey, lamports: number) {
//     const tx = new anchor.web3.Transaction().add(
//       SystemProgram.transfer({
//         fromPubkey: provider.wallet.publicKey,
//         toPubkey: pda,
//         lamports,
//       })
//     );
//     await provider.sendAndConfirm(tx);
//   }

//   it('Remove Native Liquidity', async () => {
//     const amount = new BN(100000000); // 0.1 SOL

//     await provider.connection.requestAirdrop(usdcPoolTokenAccount, 1e9 * 2);
//     await provider.connection.requestAirdrop(user.publicKey, 1e9 * 2);

//     const [userNativePoolPDA] = PublicKey.findProgramAddressSync(
//       [Buffer.from("user_native_pool"), user.publicKey.toBuffer()],
//       program.programId
//     );

//     const [nativePoolPDA] = PublicKey.findProgramAddressSync(
//       [Buffer.from("native_pool")],
//       program.programId
//     );

//     console.log(`   Remove Native Liquidity nativePoolPDA:${nativePoolPDA}`);
//     const beforeBalance= await provider.connection.getBalance(nativePoolPDA);
//     console.log(`   Remove Native Liquidity nativePool SOL before:${beforeBalance}`);

//     const beforeUserBalance = await provider.connection.getBalance(user.publicKey);
//     console.log("   Remove Native Liquidity user SOL before:", beforeUserBalance);

//     await program.methods.removeNativeLiquidity(amount)
//       .accounts({
//         poolConfig: poolConfigPDA,
//         userNativePool: userNativePoolPDA,
//         nativePool: nativePoolPDA,
//         sender: user.publicKey,
//         systemProgram: SystemProgram.programId
//       })
//       .signers([user.payer])
//       .rpc();

//     const userNativePool = await program.account.userNativePool.fetch(userNativePoolPDA);
//     console.log(`   Remove Native Liquidity userNativePool:${userNativePool.liquidity.toString()},${amount.toString()}`);

//     const afterPoolBalance= await provider.connection.getBalance(nativePoolPDA);
//     console.log(`   Remove Native Liquidity nativePool SOL after:${afterPoolBalance}`);

//     const afterUserBalance = await provider.connection.getBalance(user.publicKey);
//     console.log("   Remove Native Liquidity user SOL after:", afterUserBalance);
//   });

// //   it('Release Native Token', async () => {
// //     const amount = new BN(50000000); // 0.05 SOL
// //     const [nativePoolPDA] = PublicKey.findProgramAddressSync(
// //         [Buffer.from("native_pool")],
// //         program.programId
// //       );

// //     console.log(`   Release Native Liquidity nativePoolPDA:${nativePoolPDA}`);
// //     const beforeBalance= await provider.connection.getBalance(nativePoolPDA);
// //     console.log(`   Release Native Liquidity nativePool SOL before:${beforeBalance}`);

// //     const beforeUserBalance = await provider.connection.getBalance(user.publicKey);
// //     console.log("   Release Native Liquidity user SOL before:", beforeUserBalance);

// //     await program.methods.releaseNativeToken(amount)
// //       .accounts({
// //         poolConfig: poolConfigPDA,
// //         nativePool: nativePoolPDA,
// //         receiver: user.publicKey,
// //         authority: bridgeContract.publicKey,
// //         systemProgram: SystemProgram.programId
// //       })
// //       .signers([bridgeContract])
// //       .rpc();

// //       const afterPoolBalance= await provider.connection.getBalance(nativePoolPDA);
// //       console.log(`   Release Native Liquidity nativePool SOL after:${afterPoolBalance}`);
  
// //       const afterUserBalance = await provider.connection.getBalance(user.publicKey);
// //       console.log("   Release Native Liquidity user SOL after:", afterUserBalance);
// //   });

//   it('Set Daily Limit Config', async () => {
//     const chainId = 1;
//     const limitType = 0;
//     const [dailyLimitPDA] = PublicKey.findProgramAddressSync(
//       [Buffer.from("daily_limit_config"), 
//        usdcMint.publicKey.toBuffer(),
//        new Uint8Array([limitType]),
//        new BN(chainId).toArrayLike(Buffer, 'le', 2)],
//       program.programId
//     );
//     const now = new Date();
//     const today = Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate());
//     const todaySec = today / 1000;
//     console.log("   Set Daily Limit Config:", todaySec);
//     await program.methods.setDailyLimitConfig(
//       chainId,
//       limitType,
//       new BN(todaySec),
//       new BN(100000000) // 100 USDC daily limit
//     )
//     .accounts({
//       poolConfig: poolConfigPDA,
//       dailyLimitConfig: dailyLimitPDA,
//       admin: admin.publicKey,
//       mint: usdcMint.publicKey,
//       systemProgram: SystemProgram.programId
//     })
//     .signers([admin.payer])
//     .rpc();

//     const limitConfig = await program.account.dailyLimitConfig.fetch(dailyLimitPDA);
//     assert.equal(limitConfig.dailyLimit.toString(), '100000000');
//     console.log("   Set Daily Limit Config:", limitConfig.dailyLimit.toString(10), limitConfig.refreshTime.toString(10), limitConfig.remainTokenAmount.toString(10));
//   });

//   it('Set Rate Limit Config', async () => {
//     const chainId = 1;
//     const limitType = 0;
//     const [rateLimitPDA] = PublicKey.findProgramAddressSync(
//       [Buffer.from("rate_limit_config"), 
//        usdcMint.publicKey.toBuffer(),
//        new Uint8Array([limitType]),
//        new BN(chainId).toArrayLike(Buffer, 'le', 2)],
//       program.programId
//     );
   
//     await program.methods.setRateLimitConfig(
//       chainId,
//       limitType,
//       true,
//       new BN(10000000000), // 10000 USDC token_capacity
//       new BN(10) // 10 rate
//     )
//     .accounts({
//       poolConfig: poolConfigPDA,
//       rateLimitConfig: rateLimitPDA,
//       admin: admin.publicKey,
//       mint: usdcMint.publicKey,
//       systemProgram: SystemProgram.programId
//     })
//     .signers([admin.payer])
//     .rpc();

//     const limitConfig = await program.account.rateLimitConfig.fetch(rateLimitPDA);
//     assert.equal(limitConfig.tokenCapacity.toString(), '10000000000');
//     console.log("   Set Rate Limit Config:", limitConfig.tokenCapacity.toString(10), limitConfig.currentTokenAmount.toString(10), limitConfig.rate.toString(10), limitConfig.lastUpdatedTime.toString(10));
//   });

//   // it('Consume Limit', async () => {
//   //   const chainId = 1;
//   //   const limitType = 0;
//   //   const [dailyLimitPDA] = PublicKey.findProgramAddressSync(
//   //       [Buffer.from("daily_limit_config"), 
//   //        usdcMint.publicKey.toBuffer(),
//   //        new Uint8Array([limitType]),
//   //        new BN(chainId).toArrayLike(Buffer, 'le', 2)],
//   //       program.programId
//   //     );
//   //   const [rateLimitPDA] = PublicKey.findProgramAddressSync(
//   //     [Buffer.from("rate_limit_config"), 
//   //      usdcMint.publicKey.toBuffer(),
//   //      new Uint8Array([limitType]),
//   //      new BN(chainId).toArrayLike(Buffer, 'le', 2)],
//   //     program.programId
//   //   );
   
//   //   await program.methods.consumeLimit(
//   //     chainId,
//   //     limitType,
//   //     new BN(1000000) // 1 USDC
//   //   )
//   //   .accounts({
//   //     poolConfig: poolConfigPDA,
//   //     dailyLimitConfig: dailyLimitPDA,
//   //     rateLimitConfig: rateLimitPDA,
//   //     sender: bridgeContract.publicKey,
//   //     mint: usdcMint.publicKey,
//   //     systemProgram: SystemProgram.programId
//   //   })
//   //   .signers([bridgeContract])
//   //   .rpc();

//   //   const dailyLimitConfig = await program.account.dailyLimitConfig.fetch(dailyLimitPDA);
//   //   const rateLimitConfig = await program.account.rateLimitConfig.fetch(rateLimitPDA);
//   //   console.log("   Consume Limit:", dailyLimitConfig.remainTokenAmount.toString(10), rateLimitConfig.currentTokenAmount.toString(10));
//   // });

//   it('Init Code Upgrade', async () => {
//     await program.methods.initCodeUpgrade()
//       .accounts({
//         poolConfig: poolConfigPDA,
//         upgrade: upgradePDA,
//         admin: admin.publicKey,
//         systemProgram: SystemProgram.programId
//       })
//       .signers([admin.payer])
//       .rpc();

//     const up = await program.account.upgrade.fetch(upgradePDA);
//     console.log("   Init Code Upgrade:", up.endCode.toString(10));
//   });

//   it('Init Owner Upgrade', async () => {
//     await program.methods.initOwnerUpgrade()
//       .accounts({
//         poolConfig: poolConfigPDA,
//         upgrade: upgradePDA,
//         admin: admin.publicKey,
//         systemProgram: SystemProgram.programId
//       })
//       .signers([admin.payer])
//       .rpc();

//     const up = await program.account.upgrade.fetch(upgradePDA);
//     console.log("   Init Owner Upgrade:", up.endOwner.toString(10));
//   });

//   it('Init Admin Upgrade', async () => {
//     await program.methods.initAdminUpgrade()
//       .accounts({
//         poolConfig: poolConfigPDA,
//         upgrade: upgradePDA,
//         admin: admin.publicKey,
//         systemProgram: SystemProgram.programId
//       })
//       .signers([admin.payer])
//       .rpc();

//     const up = await program.account.upgrade.fetch(upgradePDA);
//     console.log("   Init Admin Upgrade:", up.endAdmin.toString(10));
//   });

//   it('Cancel Code Upgrade', async () => {
//     await program.methods.cancelCodeUpgrade()
//       .accounts({
//         poolConfig: poolConfigPDA,
//         upgrade: upgradePDA,
//         admin: admin.publicKey,
//         systemProgram: SystemProgram.programId
//       })
//       .signers([admin.payer])
//       .rpc();

//     const up = await program.account.upgrade.fetch(upgradePDA);
//     console.log("   Cancel Code Upgrade:", up.endCode.toString(10));
//   });

//   it('Cancel Owner Upgrade', async () => {
//     await program.methods.cancelOwnerUpgrade()
//       .accounts({
//         poolConfig: poolConfigPDA,
//         upgrade: upgradePDA,
//         admin: admin.publicKey,
//         systemProgram: SystemProgram.programId
//       })
//       .signers([admin.payer])
//       .rpc();

//     const up = await program.account.upgrade.fetch(upgradePDA);
//     console.log("   Cancel Owner Upgrade:", up.endOwner.toString(10));
//   });

//   it('finalize Upgrades', async () => {
//     await program.methods.finalizeUpgrades()
//       .accounts({
//         poolConfig: poolConfigPDA,
//         upgrade: upgradePDA,
//         admin: admin.publicKey,
//         systemProgram: SystemProgram.programId
//       })
//       .signers([admin.payer])
//       .rpc();

//     const up = await program.account.upgrade.fetch(upgradePDA);
//     console.log("   finalize Upgrades:", up.endCode.toString(10), up.endOwner.toString(10), up.endAdmin.toString(10), up.code.toBase58(), up.owner.toBase58(), up.admin.toBase58());
//   });

//   it('Cancel Admin Upgrade', async () => {
//     await program.methods.cancelAdminUpgrade()
//       .accounts({
//         poolConfig: poolConfigPDA,
//         upgrade: upgradePDA,
//         admin: admin.publicKey,
//         systemProgram: SystemProgram.programId
//       })
//       .signers([admin.payer])
//       .rpc();

//     const up = await program.account.upgrade.fetch(upgradePDA);
//     console.log("   Cancel Admin Upgrade:", up.endAdmin.toString(10));
//   });

//   it('Set Bridge Contract', async () => {
//     const newBridge = Keypair.generate();
//     await program.methods.setBridgeContract(newBridge.publicKey)
//       .accounts({
//         poolConfig: poolConfigPDA,
//         admin: admin.publicKey,
//         systemProgram: SystemProgram.programId
//       })
//       .signers([admin.payer])
//       .rpc();

//     const config = await program.account.poolConfig.fetch(poolConfigPDA);
//     assert.equal(config.bridgeContract.toString(), newBridge.publicKey.toString());
//   });

//   it('Set Multisig Contract', async () => {
//     const multisigContract = Keypair.generate();
//     await program.methods.setMultisigContract("new_multisig2",multisigContract.publicKey)
//         .accounts({ 
//             poolConfig: poolConfigPDA,
//             admin: admin.publicKey,
//             systemProgram: SystemProgram.programId
//         })
//         .signers([admin.payer])
//         .rpc();

//     const config = await program.account.poolConfig.fetch(poolConfigPDA);
//     assert.equal(config.multisigContract.toString(), multisigContract.publicKey.toString());
//   });

// //   it('Set Admin', async () => {
// //     const newAdmin = Keypair.generate();
// //     await program.methods.setAdmin(newAdmin.publicKey)
// //       .accounts({
// //         poolConfig: poolConfigPDA,
// //         admin: admin.publicKey,
// //         systemProgram: SystemProgram.programId
// //       })
// //       .signers([admin.payer])
// //       .rpc();

// //     const config = await program.account.poolConfig.fetch(poolConfigPDA);
// //     assert.equal(config.admin.toString(), newAdmin.publicKey.toString());
// //   });

//   it('Set Admin Invalid', async () => {
//     try {
//       await program.methods.setAdmin(Keypair.generate().publicKey)
//         .accounts({
//           poolConfig: poolConfigPDA,
//           admin: user.publicKey,
//           systemProgram: SystemProgram.programId
//         })
//         .signers([user.payer])
//         .rpc();
//       assert.fail('Should throw error');
//     } catch (err) {
//     //   expect(err.error.errorCode.code).to.equal('ConstraintAddress');
//         console.log(`   Set Admin Invalid: ${err.message}`);
//     }
//   });
// });