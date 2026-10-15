# 1. Build Solana Program
anchor build

# 2. Get Program ID
solana address -k target/deploy/breakboost-keypair.json

# 3. Synchronize keys across Anchor.toml and lib.rs
anchor keys sync

# 4. Deploy to Solana Devnet / Mainnet-Beta
solana config set --url https://api.devnet.solana.com
anchor deploy --provider.cluster devnet

# 5. Initialize Campaign via CLI
npm run cli -- init-campaign \
  --name "Solana Breakpoint 2026 Travel Boost" \
  --budget 50000 \
  --max-stipend 2500 \
  --mint EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v