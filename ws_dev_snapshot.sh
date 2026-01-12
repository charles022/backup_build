#!/bin/bash
set -euo pipefail

# ws_dev_snapshot.sh
# Usage: ws_dev_snapshot.sh YYYY-MM

LABEL="$1"
SOURCE_SUBVOL="/home/chuck/code"
SNAPSHOT_DIR="/home/chuck/snapshots"
SNAPSHOT_PATH="$SNAPSHOT_DIR/dev@$LABEL"

if [ -d "$SNAPSHOT_PATH" ]; then
    echo "Snapshot $SNAPSHOT_PATH already exists."
    exit 0
fi

echo "Creating snapshot dev@$LABEL..."
sudo btrfs subvolume snapshot -r "$SOURCE_SUBVOL" "$SNAPSHOT_PATH"
echo "Snapshot created at $SNAPSHOT_PATH"
