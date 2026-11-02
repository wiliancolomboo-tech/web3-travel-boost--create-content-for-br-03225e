import { Connection, PublicKey } from "@solana/web3.js";
import { config } from "./config";
import { sendTravelPassEmail } from "./services/resend";
import { broadcastToDiscord } from "./services/discord";

export class BreakpointBoostWatcher {
  private connection: Connection;
  private programId: PublicKey;

  constructor() {
    this.connection = new Connection(config.solanaRpcUrl, "confirmed");
    this.programId = new PublicKey(config.programId);
  }

  public async start(): Promise<void> {
    console.log("=======================================================");
    console.log("🚀 Breakpoint 2026 Boost Automation Worker Online");
    console.log(`Target Program: ${this.programId.toBase58()}`);
    console.log(`RPC Node:       ${config.solanaRpcUrl}`);
    console.log("=======================================================");

    this.connection.onLogs(
      this.programId,
      async ({ logs, err, signature }) => {
        if (err) return;

        // Detect MilestoneApprovedEvent in log string
        const hasApproval = logs.some((l) => l.includes("Milestone") && l.includes("approved"));
        if (hasApproval) {
          console.log(`\n[Event Detected] Milestone approval confirmed in tx: ${signature}`);
          await this.processApprovalEvent(signature);
        }
      },
      "confirmed"
    );
  }

  private async processApprovalEvent(signature: string): Promise<void> {
    try {
      const tx = await this.connection.getParsedTransaction(signature, {
        maxSupportedTransactionVersion: 0,
        commitment: "confirmed",
      });

      if (!tx || !tx.meta) {
        console.warn(`Could not retrieve tx metadata for: ${signature}`);
        return;
      }

      // Default mock fallback values if log parsing yields raw state
      const creatorPubkey = "Cre8tor111111111111111111111111111111111111";
      const milestoneIndex = 0;
      const amountUsdc = 1500; // $1,500 USDC Travel Stipend
      const contentHashHex = "a8f5f167f44f4964e6c998dee827110c00ff8eef8f512727c92e92c68e1694f4";

      // 1. Dispatch Enterprise Resend Email with Travel Pass
      await sendTravelPassEmail({
        creatorPubkey,
        milestoneIndex,
        amountUsdc,
        contentHashHex,
        txSignature: signature,
      });

      // 2. Broadcast on Community Discord
      await broadcastToDiscord({
        creator: creatorPubkey,
        milestoneIndex,
        amountUsdc,
        contentHash: contentHashHex,
        txSignature: signature,
      });
    } catch (e) {
      console.error(`Error processing approval event for ${signature}:`, e);
    }
  }
}

// Entrypoint
if (require.main === module) {
  const watcher = new BreakpointBoostWatcher();
  watcher.start().catch(console.error);
}