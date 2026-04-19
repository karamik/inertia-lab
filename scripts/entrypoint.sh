#!/bin/bash
# entrypoint.sh - Inertia Docker entrypoint

set -e

echo "╔════════════════════════════════════════════════════════════╗"
echo "║                    INERTIA PROTOCOL                        ║"
echo "║                  Post-Internet Digital Species             ║"
echo "║                  In Physics We Trust. 🌌                   ║"
echo "╚════════════════════════════════════════════════════════════╝"

# Create data directory if not exists
mkdir -p /data

# Check for keypair
if [ ! -f /data/key.json ]; then
    echo "🔑 Generating new keypair..."
    inertiad --datadir /data generate-key --output /data/key.json
else
    echo "✅ Existing keypair found"
fi

# Display node ID
PUBKEY=$(cat /data/key.json | grep public_key | cut -d'"' -f4 | cut -c1-16)
echo "🆔 Node ID: ${PUBKEY}..."

# Start Inertia daemon
echo "🚀 Starting Inertia node..."
exec inertiad --datadir /data "$@"
