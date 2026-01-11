#!/bin/bash
# AIAM Absolute Verification Script
# This script provides agents with verified state proof before Tier 2/3 actions.

CONFIG_FILE="$HOME/.config/tuxtalks-rs/config.json"
echo "🔍 [AIAM] Initiating Absolute Verification..."

# 1. Check Config Existence
if [ ! -f "$CONFIG_FILE" ]; then
    echo "❌ CONFIG ERROR: $CONFIG_FILE not found"
    exit 1
fi

# 2. Extract configured player
CURRENT_PLAYER=$(grep -oP '"player":\s*"\K[^"]+' "$CONFIG_FILE")
echo "📌 Configured Player: $CURRENT_PLAYER"

# 3. Verify Process State
echo "📡 Checking process state..."

JRIVER_PID=$(pgrep -f "mediacenter")
STRAWBERRY_PID=$(pgrep -f "strawberry")
ELISA_PID=$(pgrep -f "elisa")

if [ ! -z "$JRIVER_PID" ]; then echo "✅ JRiver is RUNNING (PID: $JRIVER_PID)"; else echo "❌ JRiver is NOT running"; fi
if [ ! -z "$STRAWBERRY_PID" ]; then echo "✅ Strawberry is RUNNING (PID: $STRAWBERRY_PID)"; else echo "❌ Strawberry is NOT running"; fi
if [ ! -z "$ELISA_PID" ]; then echo "✅ Elisa is RUNNING (PID: $ELISA_PID)"; else echo "❌ Elisa is NOT running"; fi

# 4. Binary Scan (Verify No-Touch Zones)
echo "🛡️ Scanning for legacy/conflicting binaries..."
if [ -f "$HOME/.local/bin/tuxtalks-gui" ]; then
    echo "⚠️ WARNING: Legacy binary found at ~/.local/bin/tuxtalks-gui"
else
    echo "✅ No legacy binary conflicts in ~/.local/bin"
fi

echo "🏁 [AIAM] Verification Complete."
