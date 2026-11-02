import dotenv from "dotenv";
dotenv.config();

export const CONFIG = {
  SOLANA_RPC_URL: process.env.SOLANA_RPC_URL || "https://api.devnet.solana.com",
  PROGRAM_ID: process.env.PROGRAM_ID || "TrvL86V7K4qE5q9sA5tW4bMvUeB2NqV5cZ7h8mP1r9d",
  ORACLE_PRIVATE_KEY: process.env.ORACLE_PRIVATE_KEY || "",
  RESEND_API_KEY: process.env.RESEND_API_KEY || "",
  DISCORD_WEBHOOK_URL: process.env.DISCORD_WEBHOOK_URL || "",
  NOTIFICATION_EMAIL_FROM: process.env.NOTIFICATION_EMAIL_FROM || "grants@breakpoint2026.solana.com",
};