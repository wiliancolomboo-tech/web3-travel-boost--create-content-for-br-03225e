import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Breakboost } from "../target/types/breakboost";
import {
  TOKEN_PROGRAM_ID,
  createMint,
  createAccount,
  mintTo,
  getAccount,
} from "@solana/spl-token";
import { expect } from "chai";
import * as crypto from "crypto";

describe("breakboost core unit & integration test suite", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.Breakboost as Program<Breakboost>;

  const authority = provider.wallet as anchor.Wallet;
  const creator = anchor.web3.Keypair.generate();
  const unauthorizedUser = anchor.web3.Keypair.generate();

  let fundingMint: anchor.web3.PublicKey;
  let authorityTokenAccount: anchor.web3.PublicKey;
  let creatorTokenAccount: anchor.web3.PublicKey;
  let campaignPda: anchor.web3.PublicKey;
  let vaultPda: anchor.web3.PublicKey;

  const campaignName = "Breakpoint 2026 Travel Fund";
  const totalBudget = new anchor.BN(10_000_000_000); // 10,000 Tokens
  const maxStipend = new anchor.BN(2_000_000_000);   // 2,000 Tokens Max
  const requestedStipend = new anchor.BN(1_500_000_000); // 1,500 Tokens

  const sampleContent = "Solana SVM State Compression & Firedancer In-Depth Analysis";
  const contentHash = crypto.createHash("sha256").update(sampleContent).digest();

  before(async () => {
    // Airdrop SOL to creator and unauthorized user
    const tx1 = await provider.connection.requestAirdrop(creator.publicKey, 2 * anchor.web3.LAMPORTS_PER_SOL);
    const tx2 = await provider.connection.requestAirdrop(unauthorizedUser.publicKey, 2 * anchor.web3.LAMPORTS_PER_SOL);
    await provider.connection.confirmTransaction(tx1);
    await provider.connection.confirmTransaction(tx2);

    // Create Funding Token Mint (Simulating USDC)
    fundingMint = await createMint(
      provider.connection,
      (authority.payer as any),
      authority.publicKey,
      null,
      6
    );

    // Create Token Accounts
    authorityTokenAccount = await createAccount(
      provider.connection,
      (authority.payer as any),
      fundingMint,
      authority.publicKey
    );

    creatorTokenAccount = await createAccount(
      provider.connection,
      (authority.payer as any),
      fundingMint,
      creator.publicKey
    );

    // Mint funds to authority
    await mintTo(
      provider.connection,
      (authority.payer as any),
      fundingMint,
      authorityTokenAccount,
      authority.publicKey,
      20_000_000_000
    );

    // Derive PDAs
    [campaignPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("campaign"), authority.publicKey.toBuffer(), Buffer.from(campaignName)],
      program.programId
    );

    [vaultPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("vault"), campaignPda.toBuffer()],
      program.programId
    );
  });

  it("1. Successfully initializes the Breakpoint 2026 Campaign and Vault PDA", async () => {
    await program.methods
      .initializeCampaign(campaignName, totalBudget, maxStipend)
      .accounts({
        campaign: campaignPda,
        fundingMint: fundingMint,
        vault: vaultPda,
        authority: authority.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      })
      .rpc();

    const campaignState = await program.account.campaign.fetch(campaignPda);
    expect(campaignState.authority.toBase58()).to.equal(authority.publicKey.toBase58());
    expect(campaignState.totalBudget.toString()).to.equal(totalBudget.toString());
    expect(campaignState.maxStipend.toString()).to.equal(maxStipend.toString());
    expect(campaignState.proposalCount.toNumber()).to.equal(0);
  });

  it("2. Funds the Campaign Escrow Vault", async () => {
    await program.methods
      .fundCampaign(new anchor.BN(5_000_000_000))
      .accounts({
        campaign: campaignPda,
        vault: vaultPda,
        funderTokenAccount: authorityTokenAccount,
        funder: authority.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .rpc();

    const vaultAccount = await getAccount(provider.connection, vaultPda);
    expect(Number(vaultAccount.amount)).to.equal(5_000_000_000);
  });

  let proposalPda: anchor.web3.PublicKey;

  it("3. Content creator submits a Travel Boost proposal with SHA-256 hash", async () => {
    const campaignState = await program.account.campaign.fetch(campaignPda);
    const indexBuf = Buffer.alloc(8);
    indexBuf.writeBigUInt64LE(BigInt(campaignState.proposalCount.toNumber()));

    [proposalPda] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("proposal"), campaignPda.toBuffer(), indexBuf],
      program.programId
    );

    await program.methods
      .createProposal(
        "Firedancer Architecture Deep Dive",
        "https://github.com/solana-creators/firedancer-breakboost",
        Array.from(contentHash),
        requestedStipend
      )
      .accounts({
        campaign: campaignPda,
        proposal: proposalPda,
        creator: creator.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([creator])
      .rpc();

    const proposalState = await program.account.proposal.fetch(proposalPda);
    expect(proposalState.creator.toBase58()).to.equal(creator.publicKey.toBase58());
    expect(proposalState.requestedAmount.toString()).to.equal(requestedStipend.toString());
    expect(JSON.stringify(proposalState.status)).to.equal(JSON.stringify({ pending: {} }));
  });

  it("4. Prevents unauthorized users from reviewing proposals", async () => {
    try {
      await program.methods
        .reviewProposal(true)
        .accounts({
          campaign: campaignPda,
          proposal: proposalPda,
          authority: unauthorizedUser.publicKey,
        })
        .signers([unauthorizedUser])
        .rpc();
      expect.fail("Should have thrown unauthorized error");
    } catch (err: any) {
      expect(err.error.errorCode.code).to.equal("Unauthorized");
    }
  });

  it("5. Reviewer approves the proposal", async () => {
    await program.methods
      .reviewProposal(true)
      .accounts({
        campaign: campaignPda,
        proposal: proposalPda,
        authority: authority.publicKey,
      })
      .rpc();

    const proposalState = await program.account.proposal.fetch(proposalPda);
    expect(JSON.stringify(proposalState.status)).to.equal(JSON.stringify({ approved: {} }));
  });

  it("6. Rejects disbursement when proof hash fails integrity verification", async () => {
    const invalidHash = crypto.createHash("sha256").update("Tampered malicious submission").digest();

    try {
      await program.methods
        .verifyAndDisburseStipend(Array.from(invalidHash))
        .accounts({
          campaign: campaignPda,
          proposal: proposalPda,
          vault: vaultPda,
          creatorTokenAccount: creatorTokenAccount,
          authority: authority.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .rpc();
      expect.fail("Should have rejected invalid hash proof");
    } catch (err: any) {
      expect(err.error.errorCode.code).to.equal("ProofHashMismatch");
    }
  });

  it("7. Successfully verifies content proof and releases travel stipend", async () => {
    const creatorInitialBalance = await getAccount(provider.connection, creatorTokenAccount);
    expect(Number(creatorInitialBalance.amount)).to.equal(0);

    await program.methods
      .verifyAndDisburseStipend(Array.from(contentHash))
      .accounts({
        campaign: campaignPda,
        proposal: proposalPda,
        vault: vaultPda,
        creatorTokenAccount: creatorTokenAccount,
        authority: authority.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .rpc();

    const creatorFinalBalance = await getAccount(provider.connection, creatorTokenAccount);
    expect(Number(creatorFinalBalance.amount)).to.equal(requestedStipend.toNumber());

    const proposalState = await program.account.proposal.fetch(proposalPda);
    expect(JSON.stringify(proposalState.status)).to.equal(JSON.stringify({ disbursed: {} }));
    expect(proposalState.verifiedAt.toNumber()).to.be.greaterThan(0);
  });
});