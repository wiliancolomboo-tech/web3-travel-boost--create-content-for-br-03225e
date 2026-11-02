import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { TravelBoost } from "../target/types/travel_boost";
import { PublicKey, Keypair, SystemProgram, LAMPORTS_PER_SOL } from "@solana/web3.js";
import { expect } from "chai";
import crypto from "crypto";

describe("travel-boost-protocol", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.TravelBoost as Program<TravelBoost>;

  const authority = Keypair.generate();
  const oracle = Keypair.generate();
  const creator = Keypair.generate();

  const campaignId = Array.from(crypto.randomBytes(16));
  const rawContent = "https://github.com/solana-developers/breakpoint-2026-demo";
  const contentHash = Array.from(crypto.createHash("sha256").update(rawContent).digest());

  let campaignPda: PublicKey;
  let campaignBump: number;
  let vaultPda: PublicKey;
  let vaultBump: number;
  let submissionPda: PublicKey;

  before(async () => {
    // Airdrop funds to authority, oracle, and creator
    const airdropSigners = [authority, oracle, creator];
    for (const signer of airdropSigners) {
      const sig = await provider.connection.requestAirdrop(signer.publicKey, 10 * LAMPORTS_PER_SOL);
      await provider.connection.confirmTransaction(sig);
    }

    [campaignPda, campaignBump] = PublicKey.findProgramAddressSync(
      [Buffer.from("campaign"), Buffer.from(campaignId)],
      program.programId
    );

    [vaultPda, vaultBump] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault"), campaignPda.toBuffer()],
      program.programId
    );

    [submissionPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("submission"), campaignPda.toBuffer(), Buffer.from(contentHash)],
      program.programId
    );
  });

  it("1. Initializes the Breakpoint 2026 Travel Campaign with Escrow Vault", async () => {
    const totalBudget = new anchor.BN(50 * LAMPORTS_PER_SOL);
    const deadline = new anchor.BN(Math.floor(Date.now() / 1000) + 86400 * 30); // 30 days out

    await program.methods
      .initializeCampaign(campaignId, totalBudget, deadline)
      .accounts({
        campaign: campaignPda,
        vault: vaultPda,
        validatorOracle: oracle.publicKey,
        authority: authority.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([authority])
      .rpc();

    // Fund the PDA vault with native lamports
    const fundTx = new anchor.web3.Transaction().add(
      SystemProgram.transfer({
        fromPubkey: authority.publicKey,
        toPubkey: vaultPda,
        lamports: 10 * LAMPORTS_PER_SOL,
      })
    );
    await provider.sendAndConfirm(fundTx, [authority]);

    const campaignAccount = await program.account.campaign.fetch(campaignPda);
    expect(campaignAccount.authority.toBase58()).to.equal(authority.publicKey.toBase58());
    expect(campaignAccount.validatorOracle.toBase58()).to.equal(oracle.publicKey.toBase58());
    expect(campaignAccount.isActive).to.be.true;

    const vaultBalance = await provider.connection.getBalance(vaultPda);
    expect(vaultBalance).to.be.gte(10 * LAMPORTS_PER_SOL);
  });

  it("2. Registers Creator Content Submission with SHA-256 fingerprint", async () => {
    const metadataUri = "ipfs://QmXoypizjW3WknFiJnKLwHCnL72vedxjQkDDP1mXWo6uco";
    const grantTier = 2; // Silver Travel Grant ($1,500 eq)

    await program.methods
      .registerContent(contentHash, metadataUri, grantTier)
      .accounts({
        campaign: campaignPda,
        submission: submissionPda,
        creator: creator.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([creator])
      .rpc();

    const submissionAccount = await program.account.contentSubmission.fetch(submissionPda);
    expect(submissionAccount.creator.toBase58()).to.equal(creator.publicKey.toBase58());
    expect(submissionAccount.metadataUri).to.equal(metadataUri);
    expect(submissionAccount.milestonesApproved[0]).to.be.false;
  });

  it("3. Fails when unauthorized signer attempts milestone attestation", async () => {
    try {
      await program.methods
        .attestMilestone(0)
        .accounts({
          campaign: campaignPda,
          submission: submissionPda,
          validatorOracle: creator.publicKey, // Invalid: creator is not the registered oracle
        })
        .signers([creator])
        .rpc();
      expect.fail("Attestation should have failed with unauthorized signer.");
    } catch (err: any) {
      expect(err.error.errorCode.code).to.equal("UnauthorizedValidator");
    }
  });

  it("4. Oracle successfully attests Milestone 0 (Tier 1 Research)", async () => {
    await program.methods
      .attestMilestone(0)
      .accounts({
        campaign: campaignPda,
        submission: submissionPda,
        validatorOracle: oracle.publicKey,
      })
      .signers([oracle])
      .rpc();

    const submissionAccount = await program.account.contentSubmission.fetch(submissionPda);
    expect(submissionAccount.milestonesApproved[0]).to.be.true;
  });

  it("5. Creator claims Milestone 0 Travel Grant funds from Escrow Vault", async () => {
    const claimLamports = new anchor.BN(2 * LAMPORTS_PER_SOL);
    const initialCreatorBalance = await provider.connection.getBalance(creator.publicKey);

    await program.methods
      .claimTravelGrant(0, claimLamports)
      .accounts({
        campaign: campaignPda,
        vault: vaultPda,
        submission: submissionPda,
        creator: creator.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([creator])
      .rpc();

    const postCreatorBalance = await provider.connection.getBalance(creator.publicKey);
    expect(postCreatorBalance).to.be.greaterThan(initialCreatorBalance);

    const submissionAccount = await program.account.contentSubmission.fetch(submissionPda);
    expect(submissionAccount.milestonesClaimed[0]).to.be.true;
  });

  it("6. Rejects double-claim on already claimed milestone", async () => {
    const claimLamports = new anchor.BN(2 * LAMPORTS_PER_SOL);
    try {
      await program.methods
        .claimTravelGrant(0, claimLamports)
        .accounts({
          campaign: campaignPda,
          vault: vaultPda,
          submission: submissionPda,
          creator: creator.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([creator])
        .rpc();
      expect.fail("Should have prevented double claim.");
    } catch (err: any) {
      expect(err.error.errorCode.code).to.equal("MilestoneAlreadyApproved");
    }
  });
});