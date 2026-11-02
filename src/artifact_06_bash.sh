# 1. Build Solana Anchor Program
anchor build

# 2. Deploy Program to Solana Devnet or Mainnet-Beta
solana program deploy \
  --program-id target/deploy/travel_boost-keypair.json \
  target/deploy/travel_boost.so \
  --url https://api.devnet.solana.com

# 3. Initialize Campaign using the Anchor CLI Script
npm run init-campaign -- \
  --title "Breakpoint 2026 Travel Boost" \
  --mint EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v \
  --recipients 100