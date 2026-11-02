# 1. Clone & Build Solana Anchor Program
git clone https://github.com/solana-travelboost/breakpoint-2026.git
cd breakpoint-2026
anchor build

# 2. Deploy to Solana Devnet / Mainnet-Beta
solana config set --url https://api.devnet.solana.com
solana airdrop 2
anchor deploy --provider.cluster devnet

# 3. Configure Orchestrator Environment Variables
cp .env.example .env
# Fill in RESEND_API_KEY, DISCORD_WEBHOOK_URL, ORACLE_PRIVATE_KEY