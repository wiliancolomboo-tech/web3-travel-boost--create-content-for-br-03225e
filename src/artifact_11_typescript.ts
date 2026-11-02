import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { BreakpointTravelboost } from "../target/types/breakpoint_travelboost";
import { Keypair, LAMPORTS_PER_SOL, PublicKey, SystemProgram } from "@solana/web3.js";
import { assert, expect } from "chai";
import * as crypto from "crypto";

describe("Breakpoint 2026 TravelBoost Program Suite", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.BreakpointTravelboost as Program<BreakpointTravelboost>;

  const campaignId = `breakpoint_2026_${Date.now()}`;
  const grantAmount = new anchor.BN(1.5 * LAMPORTS_PER_SOL);
  const minScoreThreshold = 750; // out of 1000

  let campaignPda: PublicKey;
  let vaultPda: PublicKey;

  const builder = Keypair.generate();
  const reviewer1 = Keypair.generate();
  const reviewer2 = Keypair.generate();
  const rogueReviewer = Keypair.generate();

  let submissionPda: PublicKey;

  before(async () => {
    // Air-drop funds to participants
    const airdropSigners = [builder, reviewer1, reviewer2, rogueReviewer];
    for (const signer of airdropSigners) {
      const sig = await provider.connection.requestAirdrop(
        signer.publicKey,
        5 * LAMPORTS_PER_SOL
      );
      const latestBlockhash = await provider.connection.getLatestBlockhash();
      await provider.connection.confirmTransaction({
        signature: sig,
        blockhash: latestBlockhash.blockhash,
        lastValidBlockHeight: latestBlockhash.lastValidBlockHeight,
      });
    }

    [campaignPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("campaign"), Buffer.from(campaignId)],
      program.programId
    );

    [vaultPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("campaign_vault"), Buffer.from(campaignId)],
      program.programId
    );

    [submissionPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("submission"), campaignPda.toBuffer(), builder.publicKey.toBuffer()],
      program.programId
    );
  });

  it("1. Initializes the Breakpoint 2026 Campaign and Vault PDA", async () => {
    await program.methods
      .initializeCampaign(campaignId, minScoreThreshold, grantAmount)
      .accounts({
        campaign: campaignPda,
        vault: vaultPda,
        authority: provider.wallet.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    const campaignAccount = await program.account.campaign.fetch(campaignPda);
    assert.equal(campaignAccount.campaignId, campaignId);
    assert.equal(campaignAccount.minScoreThreshold, minScoreThreshold);
    assert.isTrue(campaignAccount.grantAmountPerBuilder.eq(grantAmount));
    assert.isTrue(campaignAccount.active);
  });

  it("2. Funds the Campaign Treasury Vault with 10 SOL", async () => {
    const depositAmount = new anchor.BN(10 * LAMPORTS_PER_SOL);

    await program.methods
      .fundVault(depositAmount)
      .accounts({
        campaign: campaignPda,
        vault: vaultPda,
        funder: provider.wallet.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    const vaultBalance = await provider.connection.getBalance(vaultPda);
    assert.isAtLeast(vaultBalance, 10 * LAMPORTS_PER_SOL);
  });

  it("3. Builder submits technical deep-dive for Breakpoint 2026", async () => {
    const title = "Zero-Copy SVM Deserialization & SIMD Optimization";
    const contentUrl = "https://github.com/solana-developers/breakpoint-2026-simd-guide";
    const contentHash = crypto.createHash("sha256").update(title).digest();
    const hashArray = Array.from(contentHash);

    await program.methods
      .submitContent(title, contentUrl, hashArray, 2) // Track 2: Infrastructure
      .accounts({
        submission: submissionPda,
        campaign: campaignPda,
        builder: builder.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([builder])
      .rpc();

    const submission = await program.account.contentSubmission.fetch(submissionPda);
    assert.equal(submission.title, title);
    assert.equal(submission.contentUrl, contentUrl);
    assert.equal(submission.evaluatorCount, 0);
  });

  it("4. Rejects early disbursement prior to reviewer quorum", async () => {
    try {
      await program.methods
        .verifyAndDisburse()
        .accounts({
          campaign: campaignPda,
          vault: vaultPda,
          submission: submissionPda,
          builder: builder.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();
      assert.fail("Should have thrown QuorumNotReached error");
    } catch (err: any) {
      expect(err.toString()).to.include("QuorumNotReached");
    }
  });

  it("5. Reviewer 1 scores submission (850/1000)", async () => {
    await program.methods
      .evaluateSubmission(850)
      .accounts({
        submission: submissionPda,
        campaign: campaignPda,
        evaluator: reviewer1.publicKey,
      })
      .signers([reviewer1])
      .rpc();

    const submission = await program.account.contentSubmission.fetch(submissionPda);
    assert.equal(submission.evaluatorCount, 1);
    assert.equal(submission.evaluationScore, 850);
  });

  it("6. Prevents duplicate scoring by the same reviewer", async () => {
    try {
      await program.methods
        .evaluateSubmission(900)
        .accounts({
          submission: submissionPda,
          campaign: campaignPda,
          evaluator: reviewer1.publicKey,
        })
        .signers([reviewer1])
        .rpc();
      assert.fail("Should have rejected duplicate review");
    } catch (err: any) {
      expect(err.toString()).to.include("DuplicateEvaluator");
    }
  });

  it("7. Reviewer 2 scores submission (890/1000) meeting quorum", async () => {
    await program.methods
      .evaluateSubmission(890)
      .accounts({
        submission: submissionPda,
        campaign: campaignPda,
        evaluator: reviewer2.publicKey,
      })
      .signers([reviewer2])
      .rpc();

    const submission = await program.account.contentSubmission.fetch(submissionPda);
    assert.equal(submission.evaluatorCount, 2);
    // (850 + 890) / 2 = 870
    assert.equal(submission.evaluationScore, 870);
  });

  it("8. Verifies quorum and automatically disburses 1.5 SOL grant", async () => {
    const builderBalanceBefore = await provider.connection.getBalance(builder.publicKey);

    await program.methods
      .verifyAndDisburse()
      .accounts({
        campaign: campaignPda,
        vault: vaultPda,
        submission: submissionPda,
        builder: builder.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .rpc();

    const builderBalanceAfter = await provider.connection.getBalance(builder.publicKey);
    const diff = builderBalanceAfter - builderBalanceBefore;

    assert.equal(diff, 1.5 * LAMPORTS_PER_SOL);

    const submission = await program.account.contentSubmission.fetch(submissionPda);
    assert.deepEqual(submission.status, { disbursed: {} });
  });

  it("9. Prevents double disbursement replay attack", async () => {
    try {
      await program.methods
        .verifyAndDisburse()
        .accounts({
          campaign: campaignPda,
          vault: vaultPda,
          submission: submissionPda,
          builder: builder.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .rpc();
      assert.fail("Replay attack succeeded when it must fail");
    } catch (err: any) {
      expect(err.toString()).to.include("InvalidSubmissionState");
    }
  });
});