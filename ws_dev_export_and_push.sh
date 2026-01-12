#!/bin/bash
set -euo pipefail

# ws_dev_export_and_push.sh
# Usage: ws_dev_export_and_push.sh LABEL [PARENT_LABEL]

LABEL="$1"
PARENT_LABEL="${2:-}"

# Load Config
CONFIG_FILE="/home/chuck/.config/dev-backup/config.env"
if [ -f "$CONFIG_FILE" ]; then
    source "$CONFIG_FILE"
else
    echo "Config file not found. Using defaults."
    LS_HOST="localhost"
    LS_USER="chuck"
    LS_KEY_PATH="/srv/btrfs-backups/dev/keys/ls_dev_backup.pub"
fi

SNAPSHOT_DIR="/home/chuck/snapshots"
SNAPSHOT_PATH="$SNAPSHOT_DIR/dev@$LABEL"
PUB_KEY="$LS_KEY_PATH"

if [ ! -d "$SNAPSHOT_PATH" ]; then
    echo "Error: Snapshot $SNAPSHOT_PATH does not exist."
    exit 1
fi

if [ -z "$PARENT_LABEL" ]; then
    # Anchor
    ARTIFACT_NAME="dev@$LABEL.full.send.zst.age"
    echo "Exporting FULL snapshot dev@$LABEL..."
    sudo btrfs send "$SNAPSHOT_PATH" | \
        zstd -3 | \
        age -R "$PUB_KEY" > "$ARTIFACT_NAME"
else
    # Incremental
    PARENT_PATH="$SNAPSHOT_DIR/dev@$PARENT_LABEL"
    if [ ! -d "$PARENT_PATH" ]; then
        echo "Error: Parent snapshot $PARENT_PATH does not exist."
        exit 1
    fi
    ARTIFACT_NAME="dev@$LABEL.incr.from_$PARENT_LABEL.send.zst.age"
    echo "Exporting INCREMENTAL snapshot dev@$LABEL (from $PARENT_LABEL)..."
    sudo btrfs send -p "$PARENT_PATH" "$SNAPSHOT_PATH" | \
        zstd -3 | \
        age -R "$PUB_KEY" > "$ARTIFACT_NAME"
fi

echo "Artifact created: $ARTIFACT_NAME"
SIZE=$(du -h "$ARTIFACT_NAME" | cut -f1)
echo "Size: $SIZE"

# Push to LS
echo "Pushing to LS ($LS_HOST)..."
# We assume we can scp to a tmp dir on LS
# For local simulation (localhost), we just move it if permissions allow, or use sudo cp
if [ "$LS_HOST" == "localhost" ]; then
    LS_TMP_DIR="/srv/btrfs-backups/dev/tmp"
    sudo mv "$ARTIFACT_NAME" "$LS_TMP_DIR/"
    # Fix permissions so LS user can read it if needed
    sudo chown root:root "$LS_TMP_DIR/$ARTIFACT_NAME"
    
    # Trigger registration
    echo "Triggering registration on LS..."
    sudo /usr/local/bin/ls_dev_register_artifact.sh "$LS_TMP_DIR/$ARTIFACT_NAME"
else
    # Remote
    scp "$ARTIFACT_NAME" "$LS_USER@$LS_HOST:/tmp/"
    ssh "$LS_USER@$LS_HOST" "sudo /usr/local/bin/ls_dev_register_artifact.sh /tmp/$ARTIFACT_NAME"
fi

echo "Export and Push complete."
