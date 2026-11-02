import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { BreakpointBoost } from "../target/types/breakpoint_boost";
import {
  TOKEN_PROGRAM_ID,
  createMint,
  createAccount,
  mintTo,
  getAccount,
} from "@solana/spl-token";
import { Keypair, PublicKey, SystemProgram } from "@solana/web3.js";
import { expect } from "chai";
import * as crypto from "crypto";

describe("breakpoint-boost unit & integration suite", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.BreakpointBoost as Program<BreakpointBoost>;

  const sponsor = Keypair.generate();
  const verifier = Keypair.generate();
  const applicant = Keypair.generate();
  const unauthorizedUser = Keypair.generate();

  let mintPubkey: PublicKey;
  let sponsorTokenAccount: PublicKey;
  let applicantTokenAccount: PublicKey;
  let grantPoolPda: PublicKey;
  let vaultPda: PublicKey;
  let applicationPda: PublicKey;

  const poolName = "Breakpoint2026Track";
  const stipendAmount = new anchor.BN(1_000_000_000); // 1,000 USDC (6 decimals)
  const depositAmount = new anchor.BN(10_000_000_000); // 10,000 USDC
  const sampleContent = "Firedancer Deep Dive: Network Layer & Parallel Pipeline Guide";
  const contentDigest = crypto.createHash("sha256").update(sampleContent, "utf8").digest();

  before(async () => {
    // Airdrop SOL to test actors
    for (const actor of [sponsor, verifier, applicant, unauthorizedUser]) {
      const sig = await provider.connection.requestAirdrop(
        actor.publicKey,
        10 * anchor.web3.LAMPORTS_PER_SOL
      );
      await provider.connection.confirmTransaction(sig);
    }

    // Create Mock USDC Mint
    mintPubkey = await createMint(
      provider.connection,
      sponsor,
      sponsor.publicKey,
      null,
      6
    );

    sponsorTokenAccount = await createAccount(
      provider.connection,
      sponsor,
      mintPubkey,
      sponsor.publicKey
    );

    applicantTokenAccount = await createAccount(
      provider.connection,
      applicant,
      mintPubkey,
      applicant.publicKey
    );

    await mintTo(
      provider.connection,
      sponsor,
      mintPubkey,
      sponsorTokenAccount,
      sponsor,
      50_000_000_000
    );

    [grantPoolPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("grant_pool"), sponsor.publicKey.toBuffer(), Buffer.from(poolName)],
      program.programId
    );

    [vaultPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("vault"), grantPoolPda.toBuffer()],
      program.programId
    );

    [applicationPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("application"), grantPoolPda.toBuffer(), applicant.publicKey.toBuffer()],
      program.programId
    );
  });

  it("1. Initializes a Breakpoint 2026 Grant Pool successfully", async () => {
    const futureExpiry = new anchor.BN(Math.floor(Date.now() / 1000) + 86400 * 60);

    await program.methods
      .initializePool(poolName, verifier.publicKey, stipendAmount, futureExpiry)
      .accounts({
        sponsor: sponsor.publicKey,
        tokenMint: mintPubkey,
        grantPool: grantPoolPda,
        vault: vaultPda,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      })
      .signers([sponsor])
      .rpc();

    const poolAccount = await program.account.grantPool.fetch(grantPoolPda);
    expect(poolAccount.sponsor.toBase58()).to.equal(sponsor.publicKey.toBase58());
    expect(poolAccount.verifier.toBase58()).to.equal(verifier.publicKey.toBase58());
    expect(poolAccount.totalStipendPerGrant.toNumber()).to.equal(stipendAmount.toNumber());
    expect(poolAccount.name).to.equal(poolName);
  });

  it("2. Deposits funds into the program token vault", async () => {
    await program.methods
      .depositPoolFunds(depositAmount)
      .accounts({
        sponsor: sponsor.publicKey,
        grantPool: grantPoolPda,
        sponsorTokenAccount: sponsorTokenAccount,
        vault: vaultPda,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .signers([sponsor])
      .rpc();

    const vaultInfo = await getAccount(provider.connection, vaultPda);
    expect(Number(vaultInfo.amount)).to.equal(depositAmount.toNumber());
  });

  it("3. Builder submits technical content proposal application", async () => {
    const contentUri = "https://github.com/solana-developers/breakpoint-2026-guide";

    await program.methods
      .applyForGrant(1, contentUri, Array.from(contentDigest))
      .accounts({
        applicant: applicant.publicKey,
        grantPool: grantPoolPda,
        grantApplication: applicationPda,
        systemProgram: SystemProgram.programId,
      })
      .signers([applicant])
      .rpc();

    const appState = await program.account.grantApplication.fetch(applicationPda);
    expect(appState.applicant.toBase58()).to.equal(applicant.publicKey.toBase58());
    expect(appState.status).to.equal(0); // Submitted
    expect(appState.milestone1Paid).to.be.false;
  });

  it("4. Prevents unauthorized verifier from approving milestone", async () => {
    try {
      await program.methods
        .approveAndDisburseContent(Array.from(contentDigest))
        .accounts({
          verifier: unauthorizedUser.publicKey,
          grantPool: grantPoolPda,
          application: applicationPda,
          applicantTokenAccount: applicantTokenAccount,
          vault: vaultPda,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([unauthorizedUser])
        .rpc();
      expect.fail("Expected transaction to fail with UnauthorizedVerifier");
    } catch (err: any) {
      expect(err.error?.errorCode?.code).to.equal("UnauthorizedVerifier");
    }
  });

  it("5. Rejects approval if content digest does not match on-chain record", async () => {
    const invalidDigest = crypto.createHash("sha256").update("Tampered content", "utf8").digest();

    try {
      await program.methods
        .approveAndDisburseContent(Array.from(invalidDigest))
        .accounts({
          verifier: verifier.publicKey,
          grantPool: grantPoolPda,
          application: applicationPda,
          applicantTokenAccount: applicantTokenAccount,
          vault: vaultPda,
          tokenProgram: TOKEN_PROGRAM_ID,
        })
        .signers([verifier])
        .rpc();
      expect.fail("Expected ContentHashMismatch error");
    } catch (err: any) {
      expect(err.error?.errorCode?.code).to.equal("ContentHashMismatch");
    }
  });

  it("6. Authorized verifier approves content: triggers 50% upfront travel stipend", async () => {
    const initialBalance = Number((await getAccount(provider.connection, applicantTokenAccount)).amount);

    await program.methods
      .approveAndDisburseContent(Array.from(contentDigest))
      .accounts({
        verifier: verifier.publicKey,
        grantPool: grantPoolPda,
        application: applicationPda,
        applicantTokenAccount: applicantTokenAccount,
        vault: vaultPda,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .signers([verifier])
      .rpc();

    const updatedApp = await program.account.grantApplication.fetch(applicationPda);
    expect(updatedApp.status).to.equal(1); // ContentVerified
    expect(updatedApp.milestone1Paid).to.be.true;

    const afterBalance = Number((await getAccount(provider.connection, applicantTokenAccount)).amount);
    const expectedPayout = stipendAmount.toNumber() * 0.5;
    expect(afterBalance - initialBalance).to.equal(expectedPayout);
  });

  it("7. Verifies Breakpoint 2026 on-site attendance & unlocks final 50% balance", async () => {
    const initialBalance = Number((await getAccount(provider.connection, applicantTokenAccount)).amount);
    const mockTicketSig = new Array(64).fill(7);

    await program.methods
      .verifyAttendanceAndFinalize(mockTicketSig)
      .accounts({
        verifier: verifier.publicKey,
        grantPool: grantPoolPda,
        application: applicationPda,
        applicantTokenAccount: applicantTokenAccount,
        vault: vaultPda,
        tokenProgram: TOKEN_PROGRAM_ID,
      })
      .signers([verifier])
      .rpc();

    const finalizedApp = await program.account.grantApplication.fetch(applicationPda);
    expect(finalizedApp.status).to.equal(2); // Completed
    expect(finalizedApp.milestone2Paid).to.be.true;

    const finalBalance = Number((await getAccount(provider.connection, applicantTokenAccount)).amount);
    const expectedSecondPayout = stipendAmount.toNumber() * 0.5;
    expect(finalBalance - initialBalance).to.equal(expectedSecondPayout);
  });
});