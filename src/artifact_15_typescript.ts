import axios from "axios";
import { Resend } from "resend";
import { CONFIG } from "./config";

const resend = new Resend(CONFIG.RESEND_API_KEY);

export async function sendGrantEmailNotification(
  recipientEmail: string,
  creatorWallet: string,
  milestoneIndex: number,
  solAmount: number,
  claimTx: string
): Promise<void> {
  if (!CONFIG.RESEND_API_KEY) {
    console.warn("[Notifications] RESEND_API_KEY missing; skipping email dispatch.");
    return;
  }

  const subject = `🎉 Breakpoint 2026 TravelBoost Approved: Milestone #${milestoneIndex + 1}`;
  const htmlContent = `
    <div style="font-family: Arial, sans-serif; max-width: 600px; margin: auto; padding: 20px; border: 1px solid #e0e0e0; border-radius: 8px;">
      <h2 style="color: #9945FF;">Solana Breakpoint 2026 TravelBoost Grant</h2>
      <p>Congratulations <strong>${creatorWallet}</strong>!</p>
      <p>Your technical content submission for Solana Breakpoint 2026 has passed verification by the review oracle.</p>
      <div style="background: #f7f7f7; padding: 15px; border-radius: 6px; margin: 20px 0;">
        <p><strong>Milestone:</strong> Tier ${milestoneIndex + 1}</p>
        <p><strong>Disbursed Lamports:</strong> ${solAmount} SOL</p>
        <p><strong>Transaction Signature:</strong> <a href="https://explorer.solana.com/tx/${claimTx}?cluster=devnet" target="_blank">${claimTx.slice(0, 16)}...</a></p>
      </div>
      <p>Use these funds to secure your travel arrangements, hotel reservations, or event tickets for Breakpoint 2026.</p>
      <p style="font-size: 12px; color: #888;">Solana Foundation Grants Committee &bull; Decentralized Content Engine</p>
    </div>
  `;

  try {
    const response = await resend.emails.send({
      from: CONFIG.NOTIFICATION_EMAIL_FROM,
      to: recipientEmail,
      subject,
      html: htmlContent,
    });
    console.log(`[Resend] Successfully dispatched email notification: ${response.data?.id}`);
  } catch (error) {
    console.error("[Resend] Failed to send email:", error);
  }
}

export async function broadcastDiscordMilestone(
  creator: string,
  milestoneIdx: number,
  metadataUri: string,
  txSignature: string
): Promise<void> {
  if (!CONFIG.DISCORD_WEBHOOK_URL) {
    console.warn("[Notifications] DISCORD_WEBHOOK_URL missing; skipping broadcast.");
    return;
  }

  const payload = {
    username: "Breakpoint 2026 TravelBoost Oracle",
    avatar_url: "https://solana.com/branding/solanaLogoMark.png",
    embeds: [
      {
        title: "🚀 Breakpoint 2026 Content Milestone Attested!",
        color: 0x14f195,
        fields: [
          { name: "Creator", value: `\`${creator}\``, inline: true },
          { name: "Milestone Tier", value: `Tier ${milestoneIdx + 1}`, inline: true },
          { name: "Content Artifact", value: `[View Content](${metadataUri})`, inline: false },
          { name: "Verification Tx", value: `[Explorer Link](https://explorer.solana.com/tx/${txSignature}?cluster=devnet)`, inline: false }
        ],
        timestamp: new Date().toISOString(),
        footer: { text: "Solana Breakpoint 2026 Travel Grant Protocol" }
      }
    ]
  };

  try {
    await axios.post(CONFIG.DISCORD_WEBHOOK_URL, payload);
    console.log("[Discord] Broadcast milestone notification successful.");
  } catch (error) {
    console.error("[Discord] Webhook dispatch error:", error);
  }
}