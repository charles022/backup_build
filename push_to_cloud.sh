#!/bin/bash
set -euo pipefail

# push_to_cloud.sh
# Purpose: Push artifacts/manifests to R2

BASE_DIR="/srv/btrfs-backups/dev"
MANIFEST="$BASE_DIR/manifests/snapshots_v2.tsv"
TEMP_MANIFEST="$BASE_DIR/manifests/snapshots_v2.tsv.tmp"
R2_REMOTE="cloudflare-r2"
R2_BUCKET="dev-backups"

if [ ! -f "$MANIFEST" ]; then
    echo "Manifest not found."
    exit 1
fi

echo "Scanning manifest for unpushed artifacts..."

# Process Manifest
# We use a temporary file to rebuild the manifest with updated object_keys
touch "$TEMP_MANIFEST"
# Write Header
head -n 1 "$MANIFEST" > "$TEMP_MANIFEST"

# Read body
tail -n +2 "$MANIFEST" | while IFS=$'\t' read -r TS LABEL TYPE PARENT BYTES SHA256 LOCAL_PATH OBJECT_KEY;
do
    if [ -z "$OBJECT_KEY" ] && [ -f "$LOCAL_PATH" ]; then
        # Needs push
        RELATIVE_PATH=$(realpath --relative-to="$BASE_DIR" "$LOCAL_PATH")
        REMOTE_PATH="$RELATIVE_PATH"
        
        echo "Pushing $LOCAL_PATH to $R2_REMOTE:$R2_BUCKET/$REMOTE_PATH..."
        
        # Determine Rclone command (using copyto for explicit naming)
        if rclone copyto "$LOCAL_PATH" "$R2_REMOTE:$R2_BUCKET/$REMOTE_PATH"; then
             OBJECT_KEY="$REMOTE_PATH"
             echo "Success."
        else
             echo "Failed to push $LOCAL_PATH"
             # Keep OBJECT_KEY empty so we retry next time
        fi
    fi
    
    # Write line to temp manifest
    echo -e "$TS\t$LABEL\t$TYPE\t$PARENT\t$BYTES\t$SHA256\t$LOCAL_PATH\t$OBJECT_KEY" >> "$TEMP_MANIFEST"
done

# Atomically replace manifest
mv "$TEMP_MANIFEST" "$MANIFEST"

# Push Manifest
echo "Pushing manifest..."
rclone copyto "$MANIFEST" "$R2_REMOTE:$R2_BUCKET/manifests/snapshots_v2.tsv"

echo "Push complete."
