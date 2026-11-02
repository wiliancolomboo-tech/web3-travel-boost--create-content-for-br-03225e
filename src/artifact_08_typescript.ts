import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import {
  createMint,
  createAccount,
  mintTo,
  getAccount,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import { assert, expect } from "chai";
import crypto from "crypto";

describe("TravelBoost 2026 Escrow & Milestone Suite", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  // Load program IDL
  const program = anchor.workspace.TravelBoost as Program<any>;

  let mint: PublicKey;
  let sponsor: Keypair;
  let recipient: Keypair;
  let oracle: Keypair;
  let sponsorTokenAccount: PublicKey;
  let recipientTokenAccount: PublicKey;

  const grantId = new anchor.BN(Date.now());
  const travelAdvance = new anchor.BN(1_000_000_000); // 1,000 USDC (6 decimals or 9)
  const contentBalance = new anchor.BN(1_500_000_000); // 1,500 USDC
  const totalFunding = travelAdvance.add(contentBalance); // 2,500 USDC

  let grantPda: PublicKey;
  let grantBump: number;
  let vaultPda: PublicKey;
  let vaultBump: number;

  before(async () => {
    sponsor = Keypair.generate();
    recipient = Keypair.generate();
    oracle = Keypair.generate();

    // Airdrop SOL
    const airdropSig = await provider.connection.requestAirdrop(
      sponsor.publicKey,
      10 * anchor.web3.LAMPORTS_PER_SOL
    );
    await provider.connection.confirmTransaction(airdropSig);

    // Create USDC Mock Mint
    mint = await createMint(
      provider.connection,
      sponsor,
      sponsor.publicKey,
      null,
      6
    );

    // Create Token Accounts
    sponsorTokenAccount = await createAccount(
      provider.connection,
      sponsor,
      mint,
      sponsor.publicKey
    );

    recipientTokenAccount = await createAccount(
      provider.connection,
      sponsor,
      mint,
      recipient.publicKey
    );

    // Mint 10,000 tokens to sponsor
    await mintTo(
      provider.connection,
      sponsor,
      mint,
      sponsorTokenAccount,
      sponsor.publicKey,
      10_000_000_000
    );

    // Derive PDAs
    [grantPda, grantBump] = PublicKey.findProgramAddressSync(
      [Buffer.from("travel_vault"), grantId.toArrayLike(Buffer, "le", 8)],
      program.programId
    );

    [vaultPda, vaultBump] = PublicKey.findProgramAddressSync(
      [Buffer.from("token_vault"), grantId.toArrayLike(Buffer, "le", 8)],
      program.programId
    );
  });

  it("1. Successfully initializes a Breakpoint 2026 Travel Grant Vault", async () => {
    const expiration = Math.floor(Date.now() / 1000) + 86400 * 30; // 30 days
    const deliverableHash = crypto.createHash("sha256").update("Breakpoint2026Spec").digest();

    await program.methods
      .initializeGrant(
        grantId,
        totalFunding,
        travelAdvance,
        contentBalance,
        new anchor.BN(expiration),
        Array.from(deliverableHash)
      )
      .accounts({
        grantRecord: grantPda,
        vaultTokenAccount: vaultPda,
        sponsor: sponsor.publicKey,
        recipient: recipient.publicKey,
        oracle: oracle.publicKey,
        mint: mint,
        sponsorTokenAccount: sponsorTokenAccount,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      })
      .signers([sponsor])
      .rpc();

    const vaultBalance = await getAccount(provider.connection, vaultPda);
    expect(Number(vaultBalance.amount)).to.equal(totalFunding.toNumber());

    const grantRecord = await program.account.grantRecord.fetch(grantPda);
    expect(grantRecord.travelPaid).to.be.false;
    expect(grantRecord.contentPaid).to.be.false;
    expect(grantRecord.recipient.toBase58()).to.equal(recipient.publicKey.toBase58());
  });

  it("2. Recipient successfully claims Tier 1 Travel Advance", async () => {
    const proofRef = crypto.createHash("sha256").update("FlightConfirmedPNR1234").digest();

    await program.methods
      .claimTravelAdvance(grantId, Array.from(proofRef))
      .accounts({
        grantRecord: grantPda,
        vaultTokenAccount: vaultPda,
        recipient: recipient.publicKey,
        recipientTokenAccount: recipientTokenAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .signers([recipient])
      .rpc();

    const recipientBal = await getAccount(provider.connection, recipientTokenAccount);
    expect(Number(recipientBal.amount)).to.equal(travelAdvance.toNumber());

    const grantRecord = await program.account.grantRecord.fetch(grantPda);
    expect(grantRecord.travelPaid).to.be.true;
  });

  it("3. Prevents unauthorized oracle from approving Tier 2 Content Milestone", async () => {
    const maliciousAttester = Keypair.generate();
    const deliverableHash = crypto.createHash("sha256").update("VerifiedContent").digest();

    try {
      await program.methods
        .claimContentMilestone(grantId, Array.from(deliverableHash))
        .accounts({
          grantRecord: grantPda,
          vaultTokenAccount: vaultPda,
          oracleSigner: maliciousAttester.publicKey,
          recipient: recipient.publicKey,
          recipientTokenAccount: recipientTokenAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([maliciousAttester])
        .rpc();
      assert.fail("Should have thrown UnauthorizedOracle");
    } catch (err: any) {
      expect(err.toString()).to.include("UnauthorizedOracle");
    }
  });

  it("4. Authorized Oracle claims Tier 2 Content Milestone successfully", async () => {
    const deliverableHash = crypto.createHash("sha256").update("VerifiedContent").digest();

    await program.methods
      .claimContentMilestone(grantId, Array.from(deliverableHash))
      .accounts({
        grantRecord: grantPda,
        vaultTokenAccount: vaultPda,
        oracleSigner: oracle.publicKey,
        recipient: recipient.publicKey,
        recipientTokenAccount: recipientTokenAccount,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .signers([oracle])
      .rpc();

    const recipientBal = await getAccount(provider.connection, recipientTokenAccount);
    expect(Number(recipientBal.amount)).to.equal(totalFunding.toNumber());

    const grantRecord = await program.account.grantRecord.fetch(grantPda);
    expect(grantRecord.contentPaid).to.be.true;
  });

  it("5. Prevents double-claiming of milestone payouts", async () => {
    const deliverableHash = crypto.createHash("sha256").update("VerifiedContentDuplicate").digest();

    try {
      await program.methods
        .claimContentMilestone(grantId, Array.from(deliverableHash))
        .accounts({
          grantRecord: grantPda,
          vaultTokenAccount: vaultPda,
          oracleSigner: oracle.publicKey,
          recipient: recipient.publicKey,
          recipientTokenAccount: recipientTokenAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([oracle])
        .rpc();
      assert.fail("Should have failed on ContentAlreadyPaid");
    } catch (err: any) {
      expect(err.toString()).to.include("ContentAlreadyPaid");
    }
  });
});