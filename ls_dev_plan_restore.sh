#!/bin/bash
set -euo pipefail

# ls_dev_plan_restore.sh
# Usage: ls_dev_plan_restore.sh TARGET_LABEL

TARGET_LABEL="$1"
BASE_DIR="/srv/btrfs-backups/dev"
MANIFEST="$BASE_DIR/manifests/snapshots_v2.tsv"

# Function to get info from manifest
get_info() {
    local label="$1"
    # ts | label | type | parent | bytes | sha256 | local_path | object_key
    grep -P "\t$label\t" "$MANIFEST" | tail -n 1
}

# Check if target exists
TARGET_INFO=$(get_info "$TARGET_LABEL")
if [ -z "$TARGET_INFO" ]; then
    echo "Error: Snapshot $TARGET_LABEL not found in manifest." >&2
    exit 1
fi

# Build chain backwards
CHAIN=()
CURRENT_LABEL="$TARGET_LABEL"

while true; do
    INFO=$(get_info "$CURRENT_LABEL")
    if [ -z "$INFO" ]; then
         echo "Error: Missing info for $CURRENT_LABEL in chain." >&2
         exit 1
    fi
    
    TYPE=$(echo "$INFO" | cut -f3)
    PARENT=$(echo "$INFO" | cut -f4)
    PATH=$(echo "$INFO" | cut -f7)
    
    CHAIN+=("$PATH")
    
    if [ "$TYPE" == "anchor" ]; then
        break
    fi
    
    # Optimization: Check if PARENT is already a valid subvolume on LS
    # We assume snapshots are stored in a standard location on LS for restoration purposes.
    # But wait, restore_backup_LS writes to /home/chuck/code (the working tree).
    # Does it keep the intermediate snapshots?
    # Usually yes, to allow future incremental receives.
    # Let's assume LS keeps "hydrated" snapshots in /srv/btrfs-backups/dev/restore/snapshots/ or similar?
    # The README doesn't specify where hydrated snapshots live on LS, only "Provides a writable working copy".
    # And "LS automatically updates ... after each snapshot".
    # To support incremental receive efficiently, we should keep the *received* snapshots read-only somewhere.
    # Let's assume /srv/btrfs-backups/dev/restore/snapshots/$LABEL
    
    RESTORE_SNAPSHOT_DIR="/srv/btrfs-backups/dev/restore/snapshots"
    if [ -d "$RESTORE_SNAPSHOT_DIR/$PARENT" ]; then
        # Parent exists, we don't need to go further back
        break
    fi
    
    CURRENT_LABEL="$PARENT"
done

# Output in reverse order (Oldest -> Newest)
for (( idx=${#CHAIN[@]}-1 ; idx>=0 ; idx-- )) ; do
    echo "${CHAIN[idx]}"
done
