# 1. Build Solana Program
anchor build

# 2. Deploy to Solana Devnet / Mainnet
solana program deploy \
  --program-id target/deploy/breakpoint_boost-keypair.json \
  target/deploy/breakpoint_boost.so \
  --url devnet

# 3. Launch Dockerized Notification & Automation Service
export RESEND_API_KEY="re_123456789"
export DISCORD_WEBHOOK_URL="https://api.example.com/webhooks/alerts/placeholder"
docker compose up -d --build

# 4. View Real-time Telemetry Logs
docker logs -f breakpoint_boost_agent