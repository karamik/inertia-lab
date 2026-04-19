#!/bin/bash
# healthcheck.sh - Inertia health check for Docker

# Check if process is running
if ! pgrep -f inertiad > /dev/null; then
    echo "❌ Inertia daemon not running"
    exit 1
fi

# Check if status endpoint responds
if command -v inertiad &> /dev/null; then
    STATUS=$(inertiad --datadir /data status 2>/dev/null | grep "Node running" | cut -d':' -f2 | tr -d ' ')
    if [ "$STATUS" = "Yes" ]; then
        echo "✅ Inertia is healthy"
        exit 0
    fi
fi

# Fallback: check if bridge port is listening
if netstat -tln 2>/dev/null | grep -q ":18888 "; then
    echo "✅ Bridge port is listening"
    exit 0
fi

echo "⚠️ Inertia health check failed"
exit 1
