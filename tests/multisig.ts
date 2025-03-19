// import * as anchor from '@coral-xyz/anchor';
// import { Program, web3 } from '@coral-xyz/anchor';
// import { assert } from 'chai';
// import { Multisig } from '../target/types/multisig';
// import { TokenPool } from '../target/types/token_pool';

// describe('multisig', () => {
//   const provider = anchor.AnchorProvider.env();
//   anchor.setProvider(provider);

//   const program = anchor.workspace.Multisig as Program<Multisig>;
//   const tokenPoolProgram = anchor.workspace.TokenPool as Program<TokenPool>;

//   const admin = web3.Keypair.generate();
//   const owner1 = web3.Keypair.generate();
//   const owner2 = web3.Keypair.generate();
//   const owner3 = web3.Keypair.generate();

//   console.log(`   admin's public key: ${admin.publicKey}`);
//   console.log(`   owner1's public key: ${owner1.publicKey}`);
//   console.log(`   owner2's public key: ${owner2.publicKey}`);
//   console.log(`   owner3's public key: ${owner3.publicKey}`);

//   const configName1 = "name1";
//   const configName2 = "name2";
//   const configName3 = "name3";

//   // 预分配 SOL 给测试账户
//   before(async () => {
//     await provider.connection.confirmTransaction(
//       await provider.connection.requestAirdrop(admin.publicKey, 1e9),
//     );
//   });

//   const [multisigConfigPDA] = web3.PublicKey.findProgramAddressSync(
//     [Buffer.from('multisig_config')],
//     program.programId
//   );

//   it('Initialize', async () => {
//     await program.methods.initialize(admin.publicKey)
//       .accounts({
//         multisigConfig: multisigConfigPDA,
//         admin: admin.publicKey,
//         systemProgram: anchor.web3.SystemProgram.programId,
//       })
//       .signers([admin])
//       .rpc();

//     const config = await program.account.multisigConfig.fetch(multisigConfigPDA);
//     assert.isTrue(config.isInitialized);
//     assert.equal(config.admin.toString(), admin.publicKey.toString());
//   });

//   it('Create Multisig', async () => {
//     const [multisigPDA1] = web3.PublicKey.findProgramAddressSync(
//       [Buffer.from("multisig"), Buffer.from(configName1)],
//       program.programId
//     );
//     await program.methods.createMultisig(
//       configName1,
//       [owner1.publicKey, owner2.publicKey, owner3.publicKey,provider.wallet.publicKey],
//       3
//     )
//     .accounts({
//       multisigConfig: multisigConfigPDA,
//       multisig: multisigPDA1,
//       admin: admin.publicKey,
//       systemProgram: anchor.web3.SystemProgram.programId,
//     })
//     .signers([admin])
//     .rpc();

//     const [multisigPDA2] = web3.PublicKey.findProgramAddressSync(
//       [Buffer.from("multisig"), Buffer.from(configName2)],
//       program.programId
//     );
//     await program.methods.createMultisig(
//       configName2,
//       [admin.publicKey, owner1.publicKey],
//       1
//     )
//     .accounts({
//         multisigConfig: multisigConfigPDA,
//         multisig: multisigPDA2,
//         admin: admin.publicKey,
//         systemProgram: anchor.web3.SystemProgram.programId,
//     })
//     .signers([admin])
//     .rpc();
//   });

//   it('Update Multisig', async () => {
//     const [multisigPDA] = web3.PublicKey.findProgramAddressSync(
//       [Buffer.from("multisig"), Buffer.from(configName1)],
//       program.programId
//     );

//     const newOwners = [
//       owner1.publicKey, 
//       owner2.publicKey, 
//       owner3.publicKey
//     ];
//     await program.methods.updateMultisig(newOwners, 2)
//       .accounts({
//         multisigConfig: multisigConfigPDA,
//         multisig: multisigPDA,
//         admin: admin.publicKey,
//         systemProgram: anchor.web3.SystemProgram.programId,
//       })
//       .signers([admin])
//       .rpc();

//     try {
//       await program.methods.updateMultisig(newOwners, 2)
//         .accounts({
//             multisigConfig: multisigConfigPDA,
//             multisig: multisigPDA,
//             admin: owner1.publicKey,
//             systemProgram: anchor.web3.SystemProgram.programId,
//         })
//         .signers([owner1])
//         .rpc();
//       assert.fail("Should throw error");
//     } catch (err) {
//       //assert.include(err.message, "Unauthorized");
//       console.log("   Update Multisig:", err.message);
//     }
//   });

//   it('Create Proposal', async () => {
//     const [multisigPDA] = web3.PublicKey.findProgramAddressSync(
//       [Buffer.from("multisig"), Buffer.from(configName1)],
//       program.programId
//     );

//     const [proposalPDA] = web3.PublicKey.findProgramAddressSync(
//       [Buffer.from("proposal"), multisigPDA.toBuffer()],
//       program.programId
//     );

//     const transferIx = {
//         programId: anchor.web3.SystemProgram.programId,
//         accounts: [
//           { pubkey: provider.wallet.publicKey, isSigner: false, isWritable: true },
//           { pubkey: owner1.publicKey, isSigner: false, isWritable: true },
//         ],
//         data: Buffer.alloc(0),
//       };

//     await program.methods.createProposal([transferIx])
//       .accounts({
//         multisigConfig: multisigConfigPDA,
//         proposal: proposalPDA,
//         multisig: multisigPDA,
//         proposer: admin.publicKey,
//         systemProgram: anchor.web3.SystemProgram.programId,
//       })
//       .signers([admin])
//       .rpc();

//     const proposalAccount = await program.account.proposal.fetch(proposalPDA);
//     assert.equal(proposalAccount.signers.length, 3);
//     assert.isFalse(proposalAccount.executed);
//     console.log("   Create Proposal signers length:", proposalAccount.signers.length);
//     console.log("   Create Proposal executed:", proposalAccount.executed);
//   });

//   it('Approve and Execute Proposal', async () => {
//     const [multisigPDA] = web3.PublicKey.findProgramAddressSync(
//       [Buffer.from("multisig"), Buffer.from(configName1)],
//       program.programId
//     );
  
//     const [proposalPDA] = web3.PublicKey.findProgramAddressSync(
//       [Buffer.from("proposal"), multisigPDA.toBuffer()],
//       program.programId
//     );

//     await program.methods.approveProposal()
//       .accounts({
//         multisigConfig: multisigConfigPDA,
//         proposal: proposalPDA,
//         multisig: multisigPDA,
//         signer: owner1.publicKey,
//         systemProgram: anchor.web3.SystemProgram.programId,
//       })
//       .signers([owner1])
//       .rpc();

//     await program.methods.approveProposal()
//       .accounts({
//         multisigConfig: multisigConfigPDA,
//         proposal: proposalPDA,
//         multisig: multisigPDA,
//         signer: owner2.publicKey,
//         systemProgram: anchor.web3.SystemProgram.programId,
//       })
//       .signers([owner2])
//       .rpc();

//     // 执行提案
//     // await program.methods.executeProposal()
//     //   .accounts({
//     //     multisigConfig: multisigConfigPDA,
//     //     proposal: proposalPDA,
//     //     multisig: multisigPDA,
//     //     //targetProgram: anchor.web3.SystemProgram.programId,
//     //     systemProgram: anchor.web3.SystemProgram.programId,
//     //   })
//     //   .remainingAccounts([
//     //     { pubkey: provider.wallet.publicKey, isWritable: true, isSigner: false },
//     //     { pubkey: owner1.publicKey, isWritable: true, isSigner: false },
//     //   ])
//     //   .rpc();

//     // const proposalAccount = await program.account.proposal.fetch(proposalPDA);
//     // assert.isTrue(proposalAccount.executed);
//     // console.log("   Approve and Execute Proposal executed:", proposalAccount.executed);
//   });

//   it('Set Admin via multisig', async () => {
//     // const [tokenPoolPDA] = web3.PublicKey.findProgramAddressSync(
//     //     [Buffer.from("pool_config")],
//     //     tokenPoolProgram.programId
//     //   );
//     const [poolConfigPDA] = web3.PublicKey.findProgramAddressSync(
//         [Buffer.from("pool_config")],
//         tokenPoolProgram.programId
//     );
//     await tokenPoolProgram.methods.initialize(admin.publicKey, admin.publicKey)
//         .accounts({
//             poolConfig: poolConfigPDA,
//             admin: admin.publicKey,
//             systemProgram: anchor.web3.SystemProgram.programId,
//         })
//         .signers([admin])
//         .rpc();

//     await tokenPoolProgram.methods.setMultisigContract(configName3, program.programId)
//       .accounts({
//           poolConfig: poolConfigPDA,
//           admin: admin.publicKey,
//           systemProgram: anchor.web3.SystemProgram.programId,
//       })
//       .signers([admin])
//       .rpc();

//     const config1 = await tokenPoolProgram.account.poolConfig.fetch(poolConfigPDA);
//     console.log("   Set Admin via multisig:", config1.multisigName, config1.multisigContract.toString());

//     const [multisigPDA3] = web3.PublicKey.findProgramAddressSync(
//         [Buffer.from("multisig"), Buffer.from(configName3)],
//         program.programId
//         );
    
//     const [proposalPDA3] = web3.PublicKey.findProgramAddressSync(
//         [Buffer.from("proposal"), multisigPDA3.toBuffer()],
//         program.programId
//     );
    
//     await program.methods.createMultisig(
//         configName3,
//         [admin.publicKey, owner1.publicKey, owner2.publicKey],
//         2
//     )
//     .accounts({
//         multisigConfig: multisigConfigPDA,
//         multisig: multisigPDA3,
//         admin: admin.publicKey,
//         systemProgram: anchor.web3.SystemProgram.programId,
//     })
//     .signers([admin])
//     .rpc();

//     const setAdminIxData = {
//       programId: tokenPoolProgram.programId,
//       accounts: [
//         { pubkey: poolConfigPDA, isSigner: false, isWritable: true },
//         { pubkey: multisigPDA3, isSigner: true, isWritable: false }
//       ],
//       data: tokenPoolProgram.coder.instruction.encode("setAdmin", { admin: owner3.publicKey }),
//     };

//     await program.methods.createProposal([setAdminIxData])
//       .accounts({
//         multisigConfig: multisigConfigPDA,
//         proposal: proposalPDA3,
//         multisig: multisigPDA3,
//         proposer: admin.publicKey,
//         systemProgram: anchor.web3.SystemProgram.programId,
//       })
//       .signers([admin])
//       .rpc();

//     await program.methods.approveProposal()
//       .accounts({
//         multisigConfig: multisigConfigPDA,
//         proposal: proposalPDA3,
//         multisig: multisigPDA3,
//         signer: admin.publicKey,
//         systemProgram: anchor.web3.SystemProgram.programId,
//       })
//       .signers([admin])
//       .rpc();

//     await program.methods.approveProposal()
//       .accounts({
//         multisigConfig: multisigConfigPDA,
//         proposal: proposalPDA3,
//         multisig: multisigPDA3,
//         signer: owner1.publicKey,
//         systemProgram: anchor.web3.SystemProgram.programId,
//       })
//       .signers([owner1])
//       .rpc();

//       console.log("   Set Admin via multisig tokenPoolProgram.programId:", tokenPoolProgram.programId.toString());
//       console.log("   Set Admin via multisig programId:", anchor.web3.SystemProgram.programId.toString());
//       console.log("   Set Admin via multisig multisigConfigPDA:", multisigConfigPDA.toString());
//       console.log("   Set Admin via multisig proposalPDA3:", proposalPDA3.toString());
//       console.log("   Set Admin via multisig multisigPDA3:", multisigPDA3.toString());
//       console.log("   Set Admin via multisig poolConfigPDA:", poolConfigPDA.toString());
//     await program.methods.executeProposal()
//       .accounts({
//         multisigConfig: multisigConfigPDA,
//         proposal: proposalPDA3,
//         multisig: multisigPDA3,
//         multisigPda: multisigPDA3,
//         proposer: admin.publicKey,
//         poolConfig: poolConfigPDA,
//         poolProgram: tokenPoolProgram.programId,
//         systemProgram: anchor.web3.SystemProgram.programId,
//       })
//       .signers([admin])
//       .rpc();

//     const config = await tokenPoolProgram.account.poolConfig.fetch(poolConfigPDA);
//     //assert.isTrue(poolState.admin.equals(newAdmin.publicKey));
//     console.log("   Set Admin via multisig:", config.admin.toString(), owner3.publicKey.toString());

//     const proposalAccount = await program.account.proposal.fetch(proposalPDA3);
//     //assert.isTrue(proposalAccount.executed);
//     console.log("   Set Admin Proposal executed:", proposalAccount.executed);
//   });

//   it('Set Admin', async () => {
//     const newAdmin = web3.Keypair.generate();
//     await program.methods.setAdmin(newAdmin.publicKey)
//       .accounts({
//         multisigConfig: multisigConfigPDA,
//         admin: admin.publicKey,
//         systemProgram: anchor.web3.SystemProgram.programId,
//       })
//       .signers([admin])
//       .rpc();

//     const config = await program.account.multisigConfig.fetch(multisigConfigPDA);
//     assert.equal(config.admin.toString(), newAdmin.publicKey.toString());
//     console.log("   Set Admin:", config.admin.toString(), newAdmin.publicKey.toString());
//   });
// });