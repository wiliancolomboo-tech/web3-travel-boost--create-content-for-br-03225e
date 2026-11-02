# 1. Install Anchor CLI and dependencies
cargo install --git https://github.com/coral-xyz/anchor --tag v0.30.1 anchor-cli --locked

# 2. Build Anchor Binary
anchor build

# 3. Synchronize Program ID with your deployer keypair
anchor keys sync

# 4. Deploy to Solana Devnet
solana program deploy \
  --url https://api.devnet.solana.com \
  --keypair ~/.config/solana/id.json \
  target/deploy/breakpoint_boost.so