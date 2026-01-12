#!/bin/bash
set -euo pipefail

# pull_from_cloud.sh
# Usage: pull_from_cloud.sh [LABEL] [DEST_DIR]

LABEL="${1:-latest}"
DEST_DIR="${2:-/tmp/dev-backup-cloud-pull}"
R2_REMOTE="cloudflare-r2"
R2_BUCKET="dev-backups"

mkdir -p "$DEST_DIR"

echo "Fetching manifest..."
rclone copyto "$R2_REMOTE:$R2_BUCKET/manifests/snapshots_v2.tsv" "$DEST_DIR/snapshots_v2.tsv"
MANIFEST="$DEST_DIR/snapshots_v2.tsv"

if [ "$LABEL" == "latest" ]; then
    # Find latest label
    # Sort by TS (col 1) and take last
    LABEL=$(tail -n +2 "$MANIFEST" | sort -k1 | tail -n 1 | awk '{print $2}')
    echo "Resolved 'latest' to $LABEL"
fi

echo "Planning download for $LABEL..."

# Build Chain (similar to planner, but using local copy of manifest and looking at object_keys)
CHAIN=()
CURRENT_LABEL="$LABEL"

while true; do
    # Get line
    LINE=$(grep -P "\t$CURRENT_LABEL\t" "$MANIFEST" | tail -n 1)
    if [ -z "$LINE" ]; then
        echo "Error: Label $CURRENT_LABEL not found in manifest."
        exit 1
    fi
    
    TYPE=$(echo "$LINE" | cut -f3)
    PARENT=$(echo "$LINE" | cut -f4)
    OBJECT_KEY=$(echo "$LINE" | cut -f8)
    
    if [ -z "$OBJECT_KEY" ]; then
        echo "Error: Artifact for $CURRENT_LABEL has no object key in cloud manifest."
        exit 1
    fi
    
    CHAIN+=("$OBJECT_KEY")
    
    if [ "$TYPE" == "anchor" ]; then
        break
    fi
    CURRENT_LABEL="$PARENT"
done

echo "Downloading chain..."
for KEY in "${CHAIN[@]}"; do
    echo "Downloading $KEY..."
    rclone copyto "$R2_REMOTE:$R2_BUCKET/$KEY" "$DEST_DIR/$KEY"
done

echo "Download complete in $DEST_DIR"
