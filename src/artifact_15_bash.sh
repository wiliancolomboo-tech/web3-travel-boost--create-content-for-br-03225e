# Set deployment target
solana config set --url https://api.devnet.solana.com

# Deploy Anchor bytecode
anchor deploy --program-name breakpoint_travelboost --provider.cluster devnet