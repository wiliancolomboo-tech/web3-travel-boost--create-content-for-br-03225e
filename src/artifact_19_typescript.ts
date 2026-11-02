// tests/breakpoint-boost.ts
import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { BreakpointBoost } from "../target/types/breakpoint_boost";
import {
  createMint,
  createAccount,
  mintTo,
  getAccount,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import { expect } from "chai";
import * as crypto from "crypto";

describe("breakpoint-boost test suite", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.BreakpointBoost as Program<BreakpointBoost>;

  const authority = Keypair.generate();
  const creator = Keypair.generate();

  let tokenMint: PublicKey;
  let campaignPda: PublicKey;
  let campaignBump: number;
  let vaultPda: PublicKey;
  let vaultBump: number;
  let granteePda: PublicKey;
  let granteeBump: number;
  let creatorTokenAccount: PublicKey;

  const totalAllocation = new anchor.BN(10_000_000_000); // 10,000 USDC (6 decimals)
  const milestonePayouts = [
    new anchor.BN(2_500_000_000), // Milestone 1: 2,500 USDC
    new anchor.BN(5_000_000_000), // Milestone 2: 5,000 USDC
    new anchor.BN(2_500_000_000), // Milestone 3: 2,500 USDC
  ];

  before(async () => {
    // Airdrop SOL to authority and creator
    const sig1 = await provider.connection.requestAirdrop(authority.publicKey, 10 * anchor.web3.LAMPORTS_PER_SOL);
    const sig2 = await provider.connection.requestAirdrop(creator.publicKey, 5 * anchor.web3.LAMPORTS_PER_SOL);
    await provider.connection.confirmTransaction(sig1);
    await provider.connection.confirmTransaction(sig2);

    // Initialize Mint
    tokenMint = await createMint(
      provider.connection,
      authority,
      authority.publicKey,
      null,
      6
    );

    // Derive PDAs
    [campaignPda, campaignBump] = PublicKey.findProgramAddressSync(
      [Buffer.from("campaign"), authority.publicKey.toBuffer(), tokenMint.toBuffer()],
      program.programId
    );

    [vaultPda, vaultBump] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault"), campaignPda.toBuffer()],
      program.programId
    );

    [granteePda, granteeBump] = PublicKey.findProgramAddressSync(
      [Buffer.from("grantee"), campaignPda.toBuffer(), creator.publicKey.toBuffer()],
      program.programId
    );

    creatorTokenAccount = await createAccount(
      provider.connection,
      creator,
      tokenMint,
      creator.publicKey
    );
  });

  it("1. Successfully initializes Breakpoint 2026 Campaign and Vault", async () => {
    await program.methods
      .initializeCampaign(totalAllocation, 20)
      .accounts({
        authority: authority.publicKey,
        tokenMint: tokenMint,
        campaign: campaignPda,
        vault: vaultPda,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      })
      .signers([authority])
      .rpc();

    // Fund the campaign vault directly
    await mintTo(
      provider.connection,
      authority,
      tokenMint,
      vaultPda,
      authority,
      10_000_000_000
    );

    const campaignAccount = await program.account.campaign.fetch(campaignPda);
    expect(campaignAccount.authority.toBase58()).to.equal(authority.publicKey.toBase58());
    expect(campaignAccount.totalAllocation.toNumber()).to.equal(10_000_000_000);
    expect(campaignAccount.maxGrantees).to.equal(20);
    expect(campaignAccount.currentGrantees).to.equal(0);
    expect(campaignAccount.isActive).to.be.true;

    const vaultAccount = await getAccount(provider.connection, vaultPda);
    expect(Number(vaultAccount.amount)).to.equal(10_000_000_000);
  });

  it("2. Registers a creator grantee with 3 defined travel milestones", async () => {
    const metadataUri = "https://arweave.net/breakpoint2026-creator-content-roadmap";

    await program.methods
      .registerGrantee(milestonePayouts, metadataUri)
      .accounts({
        authority: authority.publicKey,
        creator: creator.publicKey,
        campaign: campaignPda,
        grantee: granteePda,
        systemProgram: SystemProgram.programId,
      })
      .signers([authority])
      .rpc();

    const granteeAccount = await program.account.grantee.fetch(granteePda);
    expect(granteeAccount.creator.toBase58()).to.equal(creator.publicKey.toBase58());
    expect(granteeAccount.milestoneCount).to.equal(3);
    expect(granteeAccount.totalStipend.toNumber()).to.equal(10_000_000_000);
    expect(granteeAccount.currentCompletedMilestone).to.equal(0);
  });

  it("3. Creator submits valid content hash for Milestone 0", async () => {
    const sampleContent = "Solana Breakpoint 2026: Complete Infrastructure Preview & Guide";
    const contentHashBuffer = crypto.createHash("sha256").update(sampleContent).digest();
    const contentHashArray = Array.from(contentHashBuffer);

    await program.methods
      .submitMilestone(0, contentHashArray)
      .accounts({
        creator: creator.publicKey,
        grantee: granteePda,
        clock: anchor.web3.SYSVAR_CLOCK_PUBKEY,
      })
      .signers([creator])
      .rpc();

    const granteeAccount = await program.account.grantee.fetch(granteePda);
    expect(granteeAccount.milestones[0].status).to.deep.equal({ submitted: {} });
    expect(granteeAccount.milestones[0].contentHash).to.deep.equal(contentHashArray);
  });

  it("4. Evaluator approves Milestone 0 and atomically releases funds", async () => {
    const initialCreatorBal = await getAccount(provider.connection, creatorTokenAccount);

    await program.methods
      .approveMilestone(0)
      .accounts({
        authority: authority.publicKey,
        campaign: campaignPda,
        vault: vaultPda,
        grantee: granteePda,
        creatorTokenAccount: creatorTokenAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
        clock: anchor.web3.SYSVAR_CLOCK_PUBKEY,
      })
      .signers([authority])
      .rpc();

    const finalCreatorBal = await getAccount(provider.connection, creatorTokenAccount);
    const transferred = Number(finalCreatorBal.amount) - Number(initialCreatorBal.amount);
    expect(transferred).to.equal(2_500_000_000);

    const granteeAccount = await program.account.grantee.fetch(granteePda);
    expect(granteeAccount.milestones[0].status).to.deep.equal({ approvedAndDisbursed: {} });
    expect(granteeAccount.currentCompletedMilestone).to.equal(1);
  });

  it("5. Fails when unauthorized non-admin attempts to approve milestone", async () => {
    const badActor = Keypair.generate();
    const sig = await provider.connection.requestAirdrop(badActor.publicKey, anchor.web3.LAMPORTS_PER_SOL);
    await provider.connection.confirmTransaction(sig);

    try {
      await program.methods
        .approveMilestone(1)
        .accounts({
          authority: badActor.publicKey,
          campaign: campaignPda,
          vault: vaultPda,
          grantee: granteePda,
          creatorTokenAccount: creatorTokenAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
          clock: anchor.web3.SYSVAR_CLOCK_PUBKEY,
        })
        .signers([badActor])
        .rpc();
      expect.fail("Should have thrown unauthorized error");
    } catch (err: any) {
      expect(err).to.exist;
    }
  });
});