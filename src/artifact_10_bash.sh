# Build Anchor binary and generate IDL
anchor build

# Generate deterministic deployment keypair if not already created
solana-keygen new -o target/deploy/breakpoint_travel_boost-keypair.json --no-bip39-passphrase --force