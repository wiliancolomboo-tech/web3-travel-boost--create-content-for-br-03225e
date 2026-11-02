# Clone and build Solana Anchor program
cd programs/breakpoint-travelboost
anchor build

# Sync Program ID into lib.rs and Anchor.toml
anchor keys sync