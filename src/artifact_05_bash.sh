# 1. Clone & Install Dependencies
git clone https://github.com/solana-travelboost/breakpoint-2026.git
cd breakpoint-2026
yarn install

# 2. Build Anchor Smart Program
anchor build

# 3. Deploy to Solana Devnet / Mainnet
solana config set --url https://api.devnet.solana.com
anchor deploy --program-name travel_boost

# 4. Initialize Program Configuration
yarn ts-node scripts/init-campaign.ts \
  --campaign-id 2026 \
  --title "Solana Breakpoint 2026 Global Creators" \
  --tranche-a-bps 5000 \
  --max-recipients 50 \
  --funding-usdc 100000