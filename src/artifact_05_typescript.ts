import { Connection, PublicKey, Keypair } from "@solana/web3.js";
import { Program, AnchorProvider, Wallet, Idl, BorshCoder, EventParser } from "@coral-xyz/anchor";
import { Resend } from "resend";
import axios from "axios";
import * as dotenv from "dotenv";

dotenv.config();

// Configuration Interfaces
interface Config {
  solanaRpcUrl: string;
  programId: string;
  resendApiKey: string;
  discordWebhookUrl: string;
  adminKeypair: Keypair;
}

const PROGRAM_ID = new PublicKey(
  process.env.PROGRAM_ID || "BrkBt6WzB9gTq8X3gZg6jR1Z9qjXGzXGzXGzXGzXGzXX"
);

export class BreakBoostOrchestrator {
  private connection: Connection;
  private resend: Resend;
  private discordWebhook: string;
  private programId: PublicKey;

  constructor(config: Config) {
    this.connection = new Connection(config.solanaRpcUrl, {
      commitment: "confirmed",
      wsEndpoint: config.solanaRpcUrl.replace("https://", "wss://").replace("http://", "ws://"),
    });
    this.resend = new Resend(config.resendApiKey);
    this.discordWebhook = config.discordWebhookUrl;
    this.programId = new PublicKey(config.programId);
  }

  public async start(): Promise<void> {
    console.log("========================================================");
    console.log(" 🚀 BreakBoost 2026 Enterprise Relayer is Active");
    console.log(` 🛰 Program ID: ${this.programId.toBase58()}`);
    console.log("========================================================");

    this.connection.onLogs(
      this.programId,
      async (logs, ctx) => {
        if (logs.err) return;
        try {
          await this.processTransactionLogs(logs.signature, logs.logs);
        } catch (err) {
          console.error(`[Error] Failed processing signature ${logs.signature}:`, err);
        }
      },
      "confirmed"
    );
  }

  private async processTransactionLogs(signature: string, logs: string[]): Promise<void> {
    for (const log of logs) {
      if (log.includes("Program data:")) {
        // Event data parsing simulation
        if (log.includes("StipendDisbursedEvent") || logs.some(l => l.includes("verify_and_disburse_stipend"))) {
          console.log(`[Event Detected] StipendDisbursed at tx: ${signature}`);
          await this.notifyDisbursement(signature, "CreatorWalletPlaceholder", 1500000000); // 1,500 USDC
          break;
        } else if (log.includes("ProposalCreatedEvent") || logs.some(l => l.includes("create_proposal"))) {
          console.log(`[Event Detected] ProposalCreated at tx: ${signature}`);
          await this.notifyNewProposal(signature, "Solana Firedancer Deep Dive", "5x...creator");
          break;
        }
      }
    }
  }

  public async notifyNewProposal(signature: string, title: string, creator: string): Promise<void> {
    // 1. Post to Discord Review Channel
    if (this.discordWebhook) {
      await axios.post(this.discordWebhook, {
        embeds: [
          {
            title: "🌟 New Breakpoint 2026 Travel Boost Proposal!",
            color: 0x9945ff, // Solana Purple
            fields: [
              { name: "Content Title", value: title, inline: true },
              { name: "Creator", value: `\`${creator}\``, inline: true },
              { name: "Solana Explorer", value: `[View Transaction](https://explorer.solana.com/tx/${signature}?cluster=devnet)` },
            ],
            footer: { text: "BreakBoost 2026 Decentralized Curation" },
            timestamp: new Date().toISOString(),
          },
        ],
      });
    }
  }

  public async notifyDisbursement(
    signature: string,
    recipientAddress: string,
    amountLamports: number
  ): Promise<void> {
    const formattedAmount = (amountLamports / 1_000_000).toLocaleString();

    // 1. Post to Discord General Announcement
    if (this.discordWebhook) {
      await axios.post(this.discordWebhook, {
        content: `🎉 **Breakpoint 2026 Travel Boost Disbursed!** \nCreator \`${recipientAddress}\` just received **$${formattedAmount} USDC** to fund their flights & accommodation!\nTx: https://explorer.solana.com/tx/${signature}?cluster=devnet`,
      });
    }

    // 2. Dispatch Enterprise Travel Voucher Email via Resend
    const recipientEmail = process.env.DISBURSEMENT_NOTIFICATION_EMAIL || "builder@solana.org";
    try {
      await this.resend.emails.send({
        from: "BreakBoost 2026 <stipends@solana-breakpoint.com>",
        to: [recipientEmail],
        subject: "✈️ Your Solana Breakpoint 2026 Travel Stipend Has Been Disbursed!",
        html: `
          <div style="font-family: Arial, sans-serif; background-color: #0b0c10; color: #ffffff; padding: 24px; border-radius: 8px;">
            <h1 style="color: #14f195;">Congratulations! Your Breakpoint Travel Boost is Approved.</h1>
            <p>Your technical content contribution has been verified on-chain and your travel stipend is now in your wallet.</p>
            <div style="background-color: #1f2833; padding: 16px; border-radius: 6px; margin: 16px 0;">
              <p><strong>Amount:</strong> $${formattedAmount} USDC</p>
              <p><strong>Recipient Account:</strong> ${recipientAddress}</p>
              <p><strong>Transaction Signature:</strong> <a style="color: #14f195;" href="https://explorer.solana.com/tx/${signature}?cluster=devnet">${signature.slice(0, 16)}...</a></p>
            </div>
            <p>Please proceed with booking your flight and hotel for <strong>Solana Breakpoint 2026</strong>.</p>
            <p style="font-size: 12px; color: #888;">Generated automatically by the BreakBoost 2026 Autonomous Verifier.</p>
          </div>
        `,
      });
      console.log(`[Resend] Disbursement voucher successfully sent to ${recipientEmail}`);
    } catch (e) {
      console.error("[Resend] Error delivering email dispatch:", e);
    }
  }
}

// Execution Entrypoint
if (require.main === module) {
  const orchestrator = new BreakBoostOrchestrator({
    solanaRpcUrl: process.env.SOLANA_RPC_URL || "https://api.devnet.solana.com",
    programId: PROGRAM_ID.toBase58(),
    resendApiKey: process.env.RESEND_API_KEY || "re_test_dummy_key",
    discordWebhookUrl: process.env.DISCORD_WEBHOOK_URL || "",
    adminKeypair: Keypair.generate(),
  });

  orchestrator.start().catch((err) => {
    console.error("Fatal in Orchestrator:", err);
    process.exit(1);
  });
}