#!/bin/bash
set -euo pipefail

# restore_backup_LS.sh
# Usage: restore_backup_LS.sh LABEL

LABEL="$1"
BASE_DIR="/srv/btrfs-backups/dev"
KEY_FILE="$BASE_DIR/keys/ls_dev_backup.key"
RESTORE_SNAPSHOT_DIR="$BASE_DIR/restore/snapshots"
WORKTREE="/home/chuck/code"

mkdir -p "$RESTORE_SNAPSHOT_DIR"

if [ ! -f "$KEY_FILE" ]; then
    echo "Error: Private key not found at $KEY_FILE"
    exit 1
fi

# Plan restore
echo "Planning restore for $LABEL..."
PLAN=$(/usr/local/bin/ls_dev_plan_restore.sh "$LABEL")

if [ -z "$PLAN" ]; then
    echo "Error: Could not plan restore (snapshot not found or invalid chain)."
    exit 1
fi

echo "Restore plan:"
echo "$PLAN"

# Execute Plan
for ARTIFACT in $PLAN; do
    echo "Processing $ARTIFACT..."
    
    # Extract Label from filename to check if we already have it
    FILENAME=$(basename "$ARTIFACT")
    if [[ "$FILENAME" =~ dev@([0-9]{4}-[0-9]{2}) ]]; then
        ARTIFACT_LABEL="${BASH_REMATCH[0]}" # dev@YYYY-MM
    else
        echo "Error: Could not parse label from $FILENAME"
        exit 1
    fi
    
    if [ -d "$RESTORE_SNAPSHOT_DIR/$ARTIFACT_LABEL" ]; then
        echo "Snapshot $ARTIFACT_LABEL already exists. Skipping receive."
        continue
    fi
    
    echo "Decrypting and Receiving $ARTIFACT_LABEL..."
    # Decrypt -> Decompress -> Receive
    # We use -i for private key
    age -d -i "$KEY_FILE" "$ARTIFACT" | \
        zstd -d | \
        sudo btrfs receive "$RESTORE_SNAPSHOT_DIR"
done

# Update Working Copy
echo "Updating working copy at $WORKTREE..."

if [ -d "$WORKTREE" ]; then
    # Check if it's a subvolume
    if sudo btrfs subvolume show "$WORKTREE" >/dev/null 2>&1; then
        echo "Deleting old subvolume..."
        sudo btrfs subvolume delete "$WORKTREE"
    else
        echo "Warning: $WORKTREE is a directory, not a subvolume. Moving it aside."
        sudo mv "$WORKTREE" "${WORKTREE}_backup_$(date +%s)"
    fi
fi

# Create new writable snapshot
TARGET_SNAPSHOT="$RESTORE_SNAPSHOT_DIR/dev@$LABEL"
if [ ! -d "$TARGET_SNAPSHOT" ]; then
    echo "Error: Target snapshot $TARGET_SNAPSHOT missing after restore."
    exit 1
fi

echo "Snapshotting $TARGET_SNAPSHOT to $WORKTREE..."
sudo btrfs subvolume snapshot "$TARGET_SNAPSHOT" "$WORKTREE"

echo "Restore complete. LS Working Copy is now at dev@$LABEL."
