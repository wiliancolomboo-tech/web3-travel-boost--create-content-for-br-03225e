# 1. Build smart contract optimized binary
anchor build

# 2. Synchronize Program ID keys
solana address -k target/deploy/travel_boost-keypair.json
# Ensure ID matches declare_id! in src/lib.rs

# 3. Deploy to Solana Devnet
solana program deploy \
  --url https://api.devnet.solana.com \
  --program-id target/deploy/travel_boost-keypair.json \
  target/deploy/travel_boost.so

# 4. Initialize Campaign via CLI
ts-node src/index.ts init-campaign \
  --budget 100 \
  --deadline 1790000000 \
  --network devnet