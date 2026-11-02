# 1. Build Solana Program
anchor build

# 2. Deploy to Solana Devnet / Mainnet-Beta
solana program deploy \
  --program-id target/deploy/breakpoint_boost-keypair.json \
  target/deploy/breakpoint_boost.so \
  --url https://api.devnet.solana.com

# 3. Initialize Campaign & Escrow
npx ts-node cli/admin-init.ts \
  --campaign-id 2026 \
  --grant-amount 1500000000 \
  --max-recipients 50 \
  --network devnet

# 4. Launch Autonomous Oracle Container
docker-compose up -d --build