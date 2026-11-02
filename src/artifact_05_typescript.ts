import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { TravelBoost } from "../target/types/travel_boost";
import {
  createMint,
  createAccount,
  mintTo,
  getAccount,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import { expect } from "chai";

describe("TravelBoost Protocol: Breakpoint 2026 Test Suite", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.TravelBoost as Program<TravelBoost>;

  const authority = Keypair.generate();
  const funder = Keypair.generate();
  const creator = Keypair.generate();
  const unapprovedCreator = Keypair.generate();

  let tokenMint: PublicKey;
  let funderTokenAccount: PublicKey;
  let creatorTokenAccount: PublicKey;
  let authorityTokenAccount: PublicKey;

  let campaignPda: PublicKey;
  let vaultPda: PublicKey;
  let applicationPda: PublicKey;

  const campaignTitle = "Breakpoint 2026 Travel Boost";
  const campaignDescription = "Developer & Creator content grants for Breakpoint Singapore/Global 2026.";
  const tierAllowances = [
    new anchor.BN(500_000_000),   // Tier 0: 500 USDC
    new anchor.BN(1_200_000_000), // Tier 1: 1,200 USDC
    new anchor.BN(2_500_000_000), // Tier 2: 2,500 USDC
  ];

  before(async () => {
    // Airdrop SOL to test actors
    for (const actor of [authority, funder, creator, unapprovedCreator]) {
      const sig = await provider.connection.requestAirdrop(
        actor.publicKey,
        10 * anchor.web3.LAMPORTS_PER_SOL
      );
      await provider.connection.confirmTransaction(sig, "confirmed");
    }

    // Create mock USDC mint (6 decimals)
    tokenMint = await createMint(
      provider.connection,
      funder,
      funder.publicKey,
      null,
      6
    );

    // Create user token accounts
    funderTokenAccount = await createAccount(
      provider.connection,
      funder,
      tokenMint,
      funder.publicKey
    );

    creatorTokenAccount = await createAccount(
      provider.connection,
      creator,
      tokenMint,
      creator.publicKey
    );

    authorityTokenAccount = await createAccount(
      provider.connection,
      authority,
      tokenMint,
      authority.publicKey
    );

    // Mint 50,000 USDC into Funder account
    await mintTo(
      provider.connection,
      funder,
      tokenMint,
      funderTokenAccount,
      funder.publicKey,
      50_000_000_000
    );

    // Derive PDAs
    [campaignPda] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("travel-campaign"),
        authority.publicKey.toBuffer(),
        Buffer.from(campaignTitle),
      ],
      program.programId
    );

    [vaultPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("campaign-vault"), campaignPda.toBuffer()],
      program.programId
    );

    [applicationPda] = PublicKey.findProgramAddressSync(
      [
        Buffer.from("creator-application"),
        campaignPda.toBuffer(),
        creator.publicKey.toBuffer(),
      ],
      program.programId
    );
  });

  it("1. Successfully initializes Breakpoint 2026 Campaign with PDA Vault", async () => {
    const deadline = new anchor.BN(Math.floor(Date.now() / 1000) + 86400 * 30);
    const maxRecipients = 50;

    await program.methods
      .initializeCampaign(
        campaignTitle,
        campaignDescription,
        deadline,
        maxRecipients,
        tierAllowances
      )
      .accounts({
        campaign: campaignPda,
        tokenMint: tokenMint,
        vault: vaultPda,
        authority: authority.publicKey,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      })
      .signers([authority])
      .rpc();

    const campaign = await program.account.campaign.fetch(campaignPda);
    expect(campaign.title).to.equal(campaignTitle);
    expect(campaign.authority.toBase58()).to.equal(authority.publicKey.toBase58());
    expect(campaign.currentRecipients).to.equal(0);
    expect(campaign.maxRecipients).to.equal(50);
  });

  it("2. Escrows funds into campaign vault", async () => {
    const depositAmount = new anchor.BN(25_000_000_000); // 25,000 USDC

    await program.methods
      .depositFunds(depositAmount)
      .accounts({
        campaign: campaignPda,
        vault: vaultPda,
        funderTokenAccount: funderTokenAccount,
        funder: funder.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .signers([funder])
      .rpc();

    const vaultToken = await getAccount(provider.connection, vaultPda);
    expect(Number(vaultToken.amount)).to.equal(25_000_000_000);

    const campaign = await program.account.campaign.fetch(campaignPda);
    expect(campaign.totalFundsEscrowed.toNumber()).to.equal(25_000_000_000);
  });

  it("3. Allows creator to register profile for Tier 2 (Global)", async () => {
    await program.methods
      .registerCreator("solana_architect", 2)
      .accounts({
        campaign: campaignPda,
        application: applicationPda,
        creator: creator.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([creator])
      .rpc();

    const app = await program.account.creatorApplication.fetch(applicationPda);
    expect(app.handle).to.equal("solana_architect");
    expect(app.requestedTier).to.equal(2);
    expect(app.status).to.deep.equal({ submitted: {} });
  });

  it("4. Prevents registration with invalid tier index (> 2)", async () => {
    const invalidAppPda = PublicKey.findProgramAddressSync(
      [
        Buffer.from("creator-application"),
        campaignPda.toBuffer(),
        unapprovedCreator.publicKey.toBuffer(),
      ],
      program.programId
    )[0];

    try {
      await program.methods
        .registerCreator("hacker_tier", 99)
        .accounts({
          campaign: campaignPda,
          application: invalidAppPda,
          creator: unapprovedCreator.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([unapprovedCreator])
        .rpc();
      expect.fail("Should have thrown InvalidTier error");
    } catch (err: any) {
      expect(err.error.errorCode.code).to.equal("InvalidTier");
    }
  });

  it("5. Submits technical content URI (Arweave/GitHub link)", async () => {
    const contentUri = "https://arweave.net/tx_solana_breakpoint_2026_technical_deepdive";

    await program.methods
      .submitContent(contentUri)
      .accounts({
        campaign: campaignPda,
        application: applicationPda,
        creator: creator.publicKey,
      })
      .signers([creator])
      .rpc();

    const app = await program.account.creatorApplication.fetch(applicationPda);
    expect(app.contentUri).to.equal(contentUri);
    expect(app.status).to.deep.equal({ underReview: {} });
  });

  it("6. Evaluates & Approves grant via Committee Authority", async () => {
    const qualityScore = 95;

    await program.methods
      .attestAndEvaluate(qualityScore, true, null)
      .accounts({
        campaign: campaignPda,
        application: applicationPda,
        vault: vaultPda,
        authority: authority.publicKey,
      })
      .signers([authority])
      .rpc();

    const app = await program.account.creatorApplication.fetch(applicationPda);
    expect(app.status).to.deep.equal({ approved: {} });
    expect(app.approvedGrantAmount.toNumber()).to.equal(2_500_000_000); // 2,500 USDC
  });

  it("7. Creator claims travel stipend directly to token wallet", async () => {
    const balanceBefore = await getAccount(provider.connection, creatorTokenAccount);

    await program.methods
      .claimTravelGrant()
      .accounts({
        campaign: campaignPda,
        application: applicationPda,
        vault: vaultPda,
        creatorTokenAccount: creatorTokenAccount,
        creator: creator.publicKey,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .signers([creator])
      .rpc();

    const balanceAfter = await getAccount(provider.connection, creatorTokenAccount);
    const delta = Number(balanceAfter.amount) - Number(balanceBefore.amount);
    expect(delta).to.equal(2_500_000_000);

    const app = await program.account.creatorApplication.fetch(applicationPda);
    expect(app.status).to.deep.equal({ disbursed: {} });
  });

  it("8. Re-claiming disbursed grant is rejected by invariant check", async () => {
    try {
      await program.methods
        .claimTravelGrant()
        .accounts({
          campaign: campaignPda,
          application: applicationPda,
          vault: vaultPda,
          creatorTokenAccount: creatorTokenAccount,
          creator: creator.publicKey,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([creator])
        .rpc();
      expect.fail("Should have prevented duplicate grant claim");
    } catch (err: any) {
      expect(err.error.errorCode.code).to.equal("InvalidState");
    }
  });
});