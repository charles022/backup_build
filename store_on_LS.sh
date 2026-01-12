#!/bin/bash
set -euo pipefail

# store_on_LS.sh
# Usage: store_on_LS.sh SOURCE_DIR

SOURCE_DIR="$1"
BASE_DIR="/srv/btrfs-backups/dev"

if [ ! -d "$SOURCE_DIR" ]; then
    echo "Error: Source directory $SOURCE_DIR not found."
    exit 1
fi

echo "Storing artifacts from $SOURCE_DIR to LS repo..."

# Move Manifest
if [ -f "$SOURCE_DIR/snapshots_v2.tsv" ]; then
    echo "Updating manifest..."
    cp "$SOURCE_DIR/snapshots_v2.tsv" "$BASE_DIR/manifests/snapshots_v2.tsv"
fi

# Move Artifacts
# We look for typical artifact names
find "$SOURCE_DIR" -type f -name "*.age" | while read -r ARTIFACT; do
    FILENAME=$(basename "$ARTIFACT")
    if [[ "$FILENAME" =~ \.full\.send\.zst\.age$ ]]; then
        DEST="$BASE_DIR/artifacts/anchors/"
    elif [[ "$FILENAME" =~ \.incr\.from_.*\.send\.zst\.age$ ]]; then
        DEST="$BASE_DIR/artifacts/incr/"
    else
        echo "Skipping unknown file $FILENAME"
        continue
    fi
    
    mkdir -p "$DEST"
    echo "Moving $FILENAME to $DEST"
    mv "$ARTIFACT" "$DEST/"
    # Update ownership
    chown root:root "$DEST/$FILENAME"
    chmod 644 "$DEST/$FILENAME"
done

echo "Store complete."
