import { Connection, Keypair, PublicKey, clusterApiUrl } from "@solana/web3.js";
import { Program, AnchorProvider, Wallet } from "@coral-xyz/anchor";
import crypto from "crypto";
import { CONFIG } from "./config";
import { broadcastDiscordMilestone, sendGrantEmailNotification } from "./notifications";

// IDL Interface Minimal Definition
const IDL = {
  version: "0.1.0",
  name: "travel_boost",
  instructions: [
    {
      name: "attestMilestone",
      accounts: [
        { name: "campaign", isMut: false, isSigner: false },
        { name: "submission", isMut: true, isSigner: false },
        { name: "validatorOracle", isMut: false, isSigner: true }
      ],
      args: [{ name: "milestoneIndex", type: "u8" }]
    }
  ]
};

export class TravelBoostWorker {
  private connection: Connection;
  private oracleKeypair: Keypair;
  private provider: AnchorProvider;
  private program: Program;

  constructor() {
    this.connection = new Connection(CONFIG.SOLANA_RPC_URL, "confirmed");
    if (!CONFIG.ORACLE_PRIVATE_KEY) {
      throw new Error("ORACLE_PRIVATE_KEY environment variable is required.");
    }
    const secretKey = Uint8Array.from(JSON.parse(CONFIG.ORACLE_PRIVATE_KEY));
    this.oracleKeypair = Keypair.fromSecretKey(secretKey);

    const wallet = new Wallet(this.oracleKeypair);
    this.provider = new AnchorProvider(this.connection, wallet, { commitment: "confirmed" });
    this.program = new Program(IDL as any, new PublicKey(CONFIG.PROGRAM_ID), this.provider);
  }

  public async evaluateContentAndAttest(
    campaignPubkey: PublicKey,
    submissionPubkey: PublicKey,
    milestoneIndex: number,
    creatorPubkey: string,
    metadataUri: string,
    creatorEmail?: string
  ): Promise<string> {
    console.log(`[Worker] Verifying submission ${submissionPubkey.toBase58()} for milestone ${milestoneIndex}...`);

    // Call on-chain AttestMilestone instruction
    const tx = await (this.program.methods as any)
      .attestMilestone(milestoneIndex)
      .accounts({
        campaign: campaignPubkey,
        submission: submissionPubkey,
        validatorOracle: this.oracleKeypair.publicKey,
      })
      .signers([this.oracleKeypair])
      .rpc();

    console.log(`[Worker] Milestone ${milestoneIndex} attested. Tx: ${tx}`);

    // Enterprise Off-Chain Integration
    await broadcastDiscordMilestone(creatorPubkey, milestoneIndex, metadataUri, tx);

    if (creatorEmail) {
      await sendGrantEmailNotification(creatorEmail, creatorPubkey, milestoneIndex, 1.5, tx);
    }

    return tx;
  }
}

// Background Listener Loop
export async function startWorkerDaemon(): Promise<void> {
  console.log("=== Starting Breakpoint 2026 TravelBoost Worker Daemon ===");
  const worker = new TravelBoostWorker();
  console.log(`Oracle Address: ${worker["oracleKeypair"].publicKey.toBase58()}`);
  console.log(`Monitoring Program: ${CONFIG.PROGRAM_ID}`);
}

if (require.main === module) {
  startWorkerDaemon().catch((err) => {
    console.error("Worker process encountered fatal error:", err);
    process.exit(1);
  });
}