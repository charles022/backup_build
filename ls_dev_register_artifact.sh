#!/bin/bash
set -euo pipefail

# ls_dev_register_artifact.sh
# Usage: ls_dev_register_artifact.sh PATH_TO_ARTIFACT

ARTIFACT_PATH="$1"
BASE_DIR="/srv/btrfs-backups/dev"
MANIFEST="$BASE_DIR/manifests/snapshots_v2.tsv"

if [ ! -f "$ARTIFACT_PATH" ]; then
    echo "Error: Artifact $ARTIFACT_PATH not found."
    exit 1
fi

FILENAME=$(basename "$ARTIFACT_PATH")

# Parse Filename
# Format: dev@YYYY-MM.full.send.zst.age OR dev@YYYY-MM.incr.from_YYYY-MM.send.zst.age
# Regex would be best, or just string manipulation.

if [[ "$FILENAME" =~ ^dev@([0-9]{4}-[0-9]{2})\.full\.send\.zst\.age$ ]]; then
    LABEL="${BASH_REMATCH[1]}"
    TYPE="anchor"
    PARENT=""
    DEST_DIR="$BASE_DIR/artifacts/anchors"
elif [[ "$FILENAME" =~ ^dev@([0-9]{4}-[0-9]{2})\.incr\.from_([0-9]{4}-[0-9]{2})\.send\.zst\.age$ ]]; then
    LABEL="${BASH_REMATCH[1]}"
    TYPE="incremental"
    PARENT="${BASH_REMATCH[2]}"
    DEST_DIR="$BASE_DIR/artifacts/incr"
else
    echo "Error: Invalid filename format: $FILENAME"
    exit 1
fi

echo "Registering $TYPE snapshot $LABEL..."

# Calculate Metadata
BYTES=$(stat -c%s "$ARTIFACT_PATH")
SHA256=$(sha256sum "$ARTIFACT_PATH" | awk '{print $1}')
TS=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

# Move Artifact
DEST_PATH="$DEST_DIR/$FILENAME"
mv "$ARTIFACT_PATH" "$DEST_PATH"
# Ensure ownership
chown root:root "$DEST_PATH"
chmod 644 "$DEST_PATH"

# Update Manifest
# ts | label | type | parent | bytes | sha256 | local_path | object_key
# object_key is empty initially (until pushed to cloud)
echo -e "$TS\t$LABEL\t$TYPE\t$PARENT\t$BYTES\t$SHA256\t$DEST_PATH\t" >> "$MANIFEST"

echo "Artifact registered in manifest."

# Trigger Restore of LS Working Tree
echo "Updating LS working tree..."
/usr/local/bin/restore_backup_LS.sh "$LABEL"

echo "Registration complete."
