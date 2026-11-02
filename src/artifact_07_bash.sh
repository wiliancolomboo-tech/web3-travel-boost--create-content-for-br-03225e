# 1. Install Anchor CLI and Solana Tool Suite
sh -c "$(curl -sSfL https://release.anza.xyz/stable/install)"
cargo install --git https://github.com/coral-xyz/anchor avm --locked --force
avm install 0.30.1
avm use 0.30.1

# 2. Configure network (Devnet / Mainnet-Beta)
solana config set --url https://api.devnet.solana.com

# 3. Build smart contracts with deterministic verifiable binary
anchor build

# 4. Deploy Program to Solana Devnet
solana airdrop 2
anchor deploy --provider.cluster devnet

# 5. Run end-to-end QA validation suite
anchor test