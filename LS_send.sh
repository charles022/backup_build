#!/bin/bash
set -euo pipefail

# LS_send.sh
# Usage: LS_send.sh LABEL [PARENT_LABEL]

LABEL="$1"
PARENT_LABEL="${2:-}"
SNAPSHOT_DIR="/srv/btrfs-backups/dev/restore/snapshots"
SNAPSHOT="$SNAPSHOT_DIR/dev@$LABEL"

if [ ! -d "$SNAPSHOT" ]; then
    echo "Error: Snapshot $SNAPSHOT not found on LS." >&2
    exit 1
fi

if [ -n "$PARENT_LABEL" ]; then
    PARENT="$SNAPSHOT_DIR/dev@$PARENT_LABEL"
    if [ ! -d "$PARENT" ]; then
        echo "Error: Parent snapshot $PARENT not found on LS." >&2
        exit 1
    fi
    # Send Incremental
    # Note: We send to stdout
    sudo btrfs send -p "$PARENT" "$SNAPSHOT"
else
    # Send Full
    sudo btrfs send "$SNAPSHOT"
fi
