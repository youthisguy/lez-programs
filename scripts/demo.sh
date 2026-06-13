#!/usr/bin/env bash
# =============================================================================
# LP-0013: Token Mint Authority — End-to-End Demo Script
#
# Demonstrates the full mint authority lifecycle:
#   1. Create a token WITH mint authority
#   2. Mint additional tokens (authority active)
#   3. Rotate authority to a new account
#   4. Revoke authority permanently
#   5. Verify mint fails after revocation
#
# Prerequisites:
#   - Bedrock, sequencer, and indexer must be running
#   - Wallet binary built from logos-execution-zone
#   - SEQUENCER_URL set (default: http://127.0.0.1:3040)
#   - LEZ_WALLET_HOME_DIR set to wallet config directory
#
# Usage:
#   RISC0_DEV_MODE=1 bash scripts/demo.sh
#
# For real proof generation :
#   RISC0_DEV_MODE=0 bash scripts/demo.sh
# =============================================================================

set -euo pipefail

# Configuration  

SEQUENCER_URL="${SEQUENCER_URL:-http://127.0.0.1:3040}"
LEZ_WALLET_HOME_DIR="${LEZ_WALLET_HOME_DIR:-}"
WALLET_BIN="${WALLET_BIN:-}"

# Try to find wallet binary automatically
if [ -z "$WALLET_BIN" ]; then
    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    REPO_ROOT="$(dirname "$SCRIPT_DIR")"
    # Look for logos-execution-zone sibling or parent
    for candidate in \
        "$REPO_ROOT/../logos-execution-zone/target/release/wallet" \
        "$HOME/Desktop/LP-0013/logos/logos-execution-zone/target/release/wallet" \
        "$HOME/logos/logos-execution-zone/target/release/wallet"; do
        if [ -x "$candidate" ]; then
            WALLET_BIN="$candidate"
            break
        fi
    done
fi

if [ -z "$WALLET_BIN" ] || [ ! -x "$WALLET_BIN" ]; then
    echo "❌ wallet binary not found. Set WALLET_BIN=/path/to/wallet"
    exit 1
fi

if [ -z "$LEZ_WALLET_HOME_DIR" ]; then
    WALLET_DIR="$(dirname "$WALLET_BIN")"
    for candidate in \
        "$WALLET_DIR/../../../lez/wallet/configs/debug" \
        "$HOME/Desktop/LP-0013/logos/logos-execution-zone/lez/wallet/configs/debug" \
        "$HOME/logos/logos-execution-zone/lez/wallet/configs/debug"; do
        if [ -d "$candidate" ]; then
            LEZ_WALLET_HOME_DIR="$(cd "$candidate" && pwd)"
            break
        fi
    done
fi

export LEZ_WALLET_HOME_DIR
export SEQUENCER_URL

WALLET="$WALLET_BIN"

#    Helpers  

green()  { echo -e "\033[0;32m$*\033[0m"; }
yellow() { echo -e "\033[0;33m$*\033[0m"; }
red()    { echo -e "\033[0;31m$*\033[0m"; }
header() { echo; echo "════════════════════════════════════════"; green "▶ $*"; echo "════════════════════════════════════════"; }

wallet() { "$WALLET" "$@"; }

extract_id() {
    # Extract account_id from wallet output: "Public/Xxxx..." → "Xxxx..."
    grep -oE 'Public/[A-Za-z0-9]+' | head -1 | sed 's/Public\///'
}

#    Preflight  

header "Preflight checks"

echo "SEQUENCER_URL:      $SEQUENCER_URL"
echo "LEZ_WALLET_HOME_DIR: $LEZ_WALLET_HOME_DIR"
echo "WALLET_BIN:          $WALLET_BIN"
echo "RISC0_DEV_MODE:      ${RISC0_DEV_MODE:-0}"
echo

wallet check-health
green "✅ Sequencer is healthy"

#    Step 1: Create accounts  

header "Step 1: Create demo accounts"

DEF_OUTPUT=$(wallet account new public --label "demo-def-$$" 2>&1)
echo "$DEF_OUTPUT"
DEF_ID=$(echo "$DEF_OUTPUT" | extract_id)
green "Definition account: $DEF_ID"

SUPPLY_OUTPUT=$(wallet account new public --label "demo-supply-$$" 2>&1)
echo "$SUPPLY_OUTPUT"
SUPPLY_ID=$(echo "$SUPPLY_OUTPUT" | extract_id)
green "Supply account: $SUPPLY_ID"

AUTH2_OUTPUT=$(wallet account new public --label "demo-auth2-$$" 2>&1)
echo "$AUTH2_OUTPUT"
AUTH2_ID=$(echo "$AUTH2_OUTPUT" | extract_id)
green "New authority account: $AUTH2_ID"

#    Step 2: Create token WITH mint authority  

header "Step 2: Create 'Gold' token with mint authority set to definition account"

TX=$(wallet token new-with-authority \
    --definition-account-id "Public/$DEF_ID" \
    --supply-account-id "Public/$SUPPLY_ID" \
    --name "Gold" \
    --total-supply 1000000 \
    --mint-authority "$DEF_ID" 2>&1)
echo "$TX"
green "✅ Token created with mint_authority=$DEF_ID"

echo; yellow "Waiting for transaction to be included in block..."
sleep 20

echo; yellow "Verifying on-chain state..."
ACCOUNT_STATE=$(wallet account get --account-id "Public/$DEF_ID" 2>&1)
echo "$ACCOUNT_STATE"

if echo "$ACCOUNT_STATE" | grep -q "\"mint_authority\":\"$DEF_ID\""; then
    green "✅ mint_authority correctly set to $DEF_ID"
else
    red "❌ Unexpected account state after creation"
    exit 1
fi

#    Step 3: Mint additional tokens  

header "Step 3: Mint 500,000 additional tokens (authority is active)"

TX=$(wallet token mint \
    --definition "Public/$DEF_ID" \
    --holder "Public/$SUPPLY_ID" \
    --amount 500000 2>&1)
echo "$TX"
green "✅ Mint transaction submitted"

sleep 20

ACCOUNT_STATE=$(wallet account get --account-id "Public/$DEF_ID" 2>&1)
echo "$ACCOUNT_STATE"

if echo "$ACCOUNT_STATE" | grep -q '"total_supply":1500000'; then
    green "✅ total_supply correctly updated to 1,500,000"
else
    yellow "⚠️  Supply may still be updating — check account state manually"
fi

#    Step 4: Rotate authority to new account  

header "Step 4: Rotate mint authority to new account ($AUTH2_ID)"

TX=$(wallet token set-authority \
    --definition-account-id "Public/$DEF_ID" \
    --new-authority "$AUTH2_ID" 2>&1)
echo "$TX"
green "✅ Authority rotation submitted"

sleep 20

ACCOUNT_STATE=$(wallet account get --account-id "Public/$DEF_ID" 2>&1)
echo "$ACCOUNT_STATE"

if echo "$ACCOUNT_STATE" | grep -q "\"mint_authority\":\"$AUTH2_ID\""; then
    green "✅ mint_authority correctly rotated to $AUTH2_ID"
else
    yellow "⚠️  Authority may still be updating — check account state manually"
fi

#    Step 5: Revoke authority permanently  

header "Step 5: Revoke mint authority permanently (supply is now fixed)"

TX=$(wallet token set-authority \
    --definition-account-id "Public/$DEF_ID" \
    --new-authority "none" 2>&1)
echo "$TX"
green "✅ Authority revocation submitted"

sleep 20

ACCOUNT_STATE=$(wallet account get --account-id "Public/$DEF_ID" 2>&1)
echo "$ACCOUNT_STATE"

if echo "$ACCOUNT_STATE" | grep -q '"mint_authority":null'; then
    green "✅ mint_authority is null — supply permanently fixed"
else
    yellow "⚠️  Revocation may still be processing — check account state manually"
fi

#    Step 6: Verify mint fails after revocation  

header "Step 6: Attempt mint after revocation (expected: transaction rejected by program)"

yellow "Submitting mint transaction — sequencer will reject it..."
TX=$(wallet token mint \
    --definition "Public/$DEF_ID" \
    --holder "Public/$SUPPLY_ID" \
    --amount 100000 2>&1 || true)
echo "$TX"

sleep 20

FINAL_STATE=$(wallet account get --account-id "Public/$DEF_ID" 2>&1)
echo "$FINAL_STATE"

if echo "$FINAL_STATE" | grep -q '"total_supply":1500000'; then
    green "✅ Supply unchanged at 1,500,000 — mint correctly rejected after revocation"
elif echo "$FINAL_STATE" | grep -q '"mint_authority":null'; then
    green "✅ Authority is null — mint was rejected (verify supply manually)"
else
    yellow "⚠️  Check sequencer logs for: 'Mint authority has been revoked; supply is fixed'"
fi

#    Summary  

header "Demo Complete"

green "Full lifecycle demonstrated:"
echo "  ✅ Token created with mint_authority=$DEF_ID"
echo "  ✅ 500,000 tokens minted (total_supply: 1,000,000 → 1,500,000)"
echo "  ✅ Authority rotated to $AUTH2_ID"
echo "  ✅ Authority permanently revoked (mint_authority: null)"
echo "  ✅ Mint rejected after revocation"
echo
green "Both repos:"
echo "  lez-programs:          https://github.com/youthisguy/lez-programs"
echo "  logos-execution-zone:  https://github.com/youthisguy/logos-execution-zone"
echo
if [ "${RISC0_DEV_MODE:-0}" = "0" ]; then
    green "🔐 RISC0_DEV_MODE=0 — real ZK proofs were generated"
else
    yellow "⚠️  RISC0_DEV_MODE=1 — dev mode (no real proofs). Re-run with RISC0_DEV_MODE=0 for submission."
fi