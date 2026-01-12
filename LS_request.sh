#!/bin/bash
set -euo pipefail

# LS_request.sh
# Usage: LS_request.sh LABEL

LABEL="$1"
CONFIG_FILE="/home/chuck/.config/dev-backup/config.env"
if [ -f "$CONFIG_FILE" ]; then
    source "$CONFIG_FILE"
else
    LS_HOST="localhost"
    LS_USER="chuck"
fi

SNAPSHOT_ROOT="/home/chuck/snapshots"
WORKTREE="/home/chuck/code"

mkdir -p "$SNAPSHOT_ROOT"

# Find latest local snapshot for parent
# Assuming we want to restore specifically LABEL.
# If we have a previous snapshot, we can use it as parent.
# Logic: Look for any snapshot that might be a parent.
# For simplicity, we just look for the immediate predecessor if known, or just the latest available local snapshot.
# But valid parent must be an ancestor of LABEL.
# Without a manifest on WS, we don't know if 'dev@XYZ' is an ancestor of 'dev@LABEL'.
# But if we use 'latest' logic on LS side?
# The user asks for LABEL.
# If we have *any* snapshot, is it a valid parent? Not necessarily.
# Safest: Request FULL unless we are sure.
# OR: We pass our latest local label to LS, and LS checks if it is a valid parent of LABEL.
# `LS_send.sh` doesn't currently check ancestry, it just tries `btrfs send -p`.
# If `btrfs send -p` fails (not related), it errors.
# Btrfs enforces that parent must be related.

# Attempt to find a parent
PARENT_LABEL=""
# List local snapshots
LATEST_LOCAL=$(ls -1 "$SNAPSHOT_ROOT" | grep "dev@" | sort | tail -n 1)

if [ -n "$LATEST_LOCAL" ]; then
    # Extract label
    if [[ "$LATEST_LOCAL" =~ dev@([0-9]{4}-[0-9]{2}) ]]; then
        PARENT_LABEL="${BASH_REMATCH[1]}"
    fi
fi

if [ "$LABEL" == "latest" ]; then
    # Resolve "latest" by asking LS?
    # Or just pass "latest" to LS_send.sh? LS_send.sh expects a LABEL.
    # We should probably resolve it first.
    if [ "$LS_HOST" == "localhost" ]; then
        RESOLVED_LABEL=$(/usr/local/bin/ls_dev_latest_label.sh)
    else
        RESOLVED_LABEL=$(ssh "$LS_USER@$LS_HOST" "/usr/local/bin/ls_dev_latest_label.sh")
    fi
    echo "Resolved latest to $RESOLVED_LABEL"
    LABEL="$RESOLVED_LABEL"
fi

echo "Requesting dev@$LABEL from LS (Parent: ${PARENT_LABEL:-none})..."

if [ -z "$PARENT_LABEL" ]; then
    CMD="sudo /usr/local/bin/LS_send.sh $LABEL"
else
    CMD="sudo /usr/local/bin/LS_send.sh $LABEL $PARENT_LABEL"
fi

# Execute SSH and Pipe
# If localhost, direct
if [ "$LS_HOST" == "localhost" ]; then
    $CMD | sudo btrfs receive "$SNAPSHOT_ROOT"
else
    ssh "$LS_USER@$LS_HOST" "$CMD" | sudo btrfs receive "$SNAPSHOT_ROOT"
fi

# Update Working Tree
echo "Updating WS working tree..."
if [ -d "$WORKTREE" ]; then
    if sudo btrfs subvolume show "$WORKTREE" >/dev/null 2>&1; then
        sudo btrfs subvolume delete "$WORKTREE"
    else
        sudo mv "$WORKTREE" "${WORKTREE}_backup_$(date +%s)"
    fi
fi

TARGET_SNAPSHOT="$SNAPSHOT_ROOT/dev@$LABEL"
sudo btrfs subvolume snapshot "$TARGET_SNAPSHOT" "$WORKTREE"

echo "WS Restore complete."
