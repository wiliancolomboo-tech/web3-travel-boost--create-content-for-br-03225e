import * as anchor from "@coral-xyz/anchor";
import { Program, web3, BN } from "@coral-xyz/anchor";
import { Resend } from "resend";
import axios from "axios";
import crypto from "crypto";
import dotenv from "dotenv";

dotenv.config();

// Configuration schema
interface ServiceConfig {
  rpcUrl: string;
  programId: string;
  validatorKeypairPath: string;
  resendApiKey: string;
  discordWebhookUrl: string;
}

const config: ServiceConfig = {
  rpcUrl: process.env.SOLANA_RPC_URL || "http://127.0.0.1:8899",
  programId: process.env.PROGRAM_ID || "TrvL8o26BreakpointBoost11111111111111111111",
  validatorKeypairPath: process.env.VALIDATOR_KEYPAIR || "./validator-keypair.json",
  resendApiKey: process.env.RESEND_API_KEY || "re_test_mock_key",
  discordWebhookUrl: process.env.DISCORD_WEBHOOK_URL || "",
};

const resend = new Resend(config.resendApiKey);

/**
 * Calculates SHA256 cryptographic digest of technical content.
 */
export function computeContentHash(contentBuffer: Buffer): Buffer {
  return crypto.createHash("sha256").update(contentBuffer).digest();
}

/**
 * Evaluates Breakpoint 2026 technical requirements.
 * Verifies word count, code blocks, and required Breakpoint 2026 tags.
 */
export function validateBreakpointTechnicalContent(rawContent: string): {
  isValid: boolean;
  reason?: string;
} {
  const minLength = 500; // Minimum 500 words for technical depth
  const wordCount = rawContent.trim().split(/\s+/).length;

  if (wordCount < minLength) {
    return {
      isValid: false,
      reason: `Content too short (${wordCount} words). Minimum required: ${minLength} words.`,
    };
  }

  const containsBreakpointRef = /Breakpoint(\s+)?2026/i.test(rawContent);
  if (!containsBreakpointRef) {
    return {
      isValid: false,
      reason: "Missing mandatory reference to 'Breakpoint 2026'.",
    };
  }

  const containsCodeArtifacts = /