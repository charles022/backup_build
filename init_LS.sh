#!/bin/bash
set -euo pipefail

# init_LS.sh
# Purpose: Build LS from clean OS (Fedora)

# 1. Install dependencies
echo "Installing dependencies..."
sudo dnf install -y btrfs-progs zstd age rclone pv

# 2. Create repo structure
BASE_DIR="/srv/btrfs-backups/dev"
echo "Creating directory structure at $BASE_DIR..."

sudo mkdir -p "$BASE_DIR/artifacts/anchors"
sudo mkdir -p "$BASE_DIR/artifacts/incr"
sudo mkdir -p "$BASE_DIR/manifests"
sudo mkdir -p "$BASE_DIR/keys"
sudo mkdir -p "$BASE_DIR/restore/worktree_receive"
sudo mkdir -p "$BASE_DIR/tmp"
sudo mkdir -p "$BASE_DIR/logs"
sudo mkdir -p "$BASE_DIR/locks"

# 3. Generate age keypair if not exists
KEY_FILE="$BASE_DIR/keys/ls_dev_backup.key"
PUB_FILE="$BASE_DIR/keys/ls_dev_backup.pub"

if [ ! -f "$KEY_FILE" ]; then
    echo "Generating age keypair..."
    age-keygen -o "$KEY_FILE"
    # Extract public key (age-keygen output contains it, but we can also parse the key file if needed, 
    # actually age-keygen -y reads from private key)
    age-keygen -y "$KEY_FILE" > "$PUB_FILE"
    chmod 600 "$KEY_FILE"
else
    echo "Keypair already exists."
fi

# 4. Initialize manifests and state
MANIFEST_FILE="$BASE_DIR/manifests/snapshots_v2.tsv"
if [ ! -f "$MANIFEST_FILE" ]; then
    echo "Initializing manifest..."
    echo -e "ts\tlbel\ttype\tparent\tbytes\tsha256\tlocal_path\tobject_key" > "$MANIFEST_FILE"
fi

STATE_FILE="$BASE_DIR/manifests/state.env"
if [ ! -f "$STATE_FILE" ]; then
    touch "$STATE_FILE"
fi

# 5. Install LS scripts
# Assuming scripts are in the current directory and prefixed with ls_ or refer to LS roles.
# The README lists specific scripts for LS:
# ls_dev_register_artifact.sh, push_to_cloud.sh, pull_from_cloud.sh, store_on_LS.sh, 
# restore_backup_LS.sh, LS_send.sh, migrate_manifest_to_v2.sh, ls_dev_plan_restore.sh, ls_dev_latest_label.sh
# We will copy them to /usr/local/bin

INSTALL_DIR="/usr/local/bin"
echo "Installing scripts to $INSTALL_DIR..."

# List of LS scripts
SCRIPTS=(
    "ls_dev_register_artifact.sh"
    "push_to_cloud.sh"
    "pull_from_cloud.sh"
    "store_on_LS.sh"
    "restore_backup_LS.sh"
    "LS_send.sh"
    "migrate_manifest_to_v2.sh"
    "ls_dev_plan_restore.sh"
    "ls_dev_latest_label.sh"
)

# Check if we are in the source directory containing these scripts
# For now, we will warn if missing, as they might be created later.
for script in "${SCRIPTS[@]}"; do
    if [ -f "$script" ]; then
        sudo cp "$script" "$INSTALL_DIR/"
        sudo chmod +x "$INSTALL_DIR/$script"
        echo "Installed $script"
    else
        echo "Warning: $script not found in current directory. Skipping copy."
    fi
done

echo "LS Initialization complete."
