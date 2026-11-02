# -------------------------------------------------------------
# 1. BUILD & DEPLOY SOLANA SMART CONTRACT
# -------------------------------------------------------------
# Install Anchor CLI 0.30.1 and build program
anchor build

# Deploy to Solana Devnet or Mainnet-Beta
solana program deploy \
  --program-id target/deploy/breakpoint_boost-keypair.json \
  target/deploy/breakpoint_boost.so \
  --url https://api.devnet.solana.com

# -------------------------------------------------------------
# 2. RUN FULL TEST SUITE
# -------------------------------------------------------------
anchor test

# -------------------------------------------------------------
# 3. CONFIGURE ENVIRONMENT & LAUNCH CONTAINER WORKER
# -------------------------------------------------------------
cat <<EOF > .env
SOLANA_RPC_URL=https://api.devnet.solana.com
PROGRAM_ID=BrkPnt2o26Boost11111111111111111111111111111
TOKEN_MINT=EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v
ORACLE_PRIVATE_KEY_HEX=$(openssl rand -hex 64)
RESEND_API_KEY=re_123456789abcdef
DISCORD_WEBHOOK_URL=https://api.example.com/webhooks/alerts/placeholder
EMAIL_FROM=grants@solana.org
EOF

docker compose up --build -d
docker compose logs -f