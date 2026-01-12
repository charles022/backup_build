#!/bin/bash
set -euo pipefail

# init_WS.sh
# Purpose: Build WS from clean OS

# 1. Install dependencies
echo "Installing dependencies..."
# Assuming Fedora based on current OS, but generic checks are good.
if command -v dnf >/dev/null; then
    sudo dnf install -y btrfs-progs zstd age pv
elif command -v apt-get >/dev/null; then
    sudo apt-get update && sudo apt-get install -y btrfs-progs zstd age pv
fi

# 2. Ensure /home/chuck/code is a Btrfs subvolume
TARGET_DIR="/home/chuck/code"
if [ ! -d "$TARGET_DIR" ]; then
    echo "Creating $TARGET_DIR..."
    mkdir -p "$TARGET_DIR"
    # Note: We cannot easily make it a subvolume if it's not on a btrfs mount. 
    # This script assumes the parent filesystem is Btrfs.
    # If strictly needed, we would 'btrfs subvolume create "$TARGET_DIR"'
    # Check if it is btrfs:
    fs_type=$(stat -f --format=%T "$TARGET_DIR" || echo "unknown")
    if [ "$fs_type" != "btrfs" ]; then
        echo "Error: $TARGET_DIR is not on a btrfs filesystem."
        exit 1
    fi
    # Check if it is a subvolume (inode 256 usually, or listed in subvolume list)
    # Getting subvolume ID
    subvol_id=$(sudo btrfs inspect-internal rootid "$TARGET_DIR" 2>/dev/null || echo "error")
    if [ "$subvol_id" == "error" ] || [ "$subvol_id" == "5" ]; then 
        # 5 is FS root. It might be okay, but usually we want a specific subvol.
        # We will attempt to create it if it doesn't exist or warn.
        echo "Warning: $TARGET_DIR might not be a dedicated subvolume (ID: $subvol_id)."
        echo "Creating it as a subvolume is recommended for atomic snapshots."
        # Attempt to create if empty
        if [ -z "$(ls -A "$TARGET_DIR")" ]; then
             echo "Directory is empty, converting to subvolume..."
             rmdir "$TARGET_DIR"
             sudo btrfs subvolume create "$TARGET_DIR"
             sudo chown "$USER:$USER" "$TARGET_DIR"
        fi
    fi
else
    # Directory exists
    fs_type=$(stat -f --format=%T "$TARGET_DIR")
    if [ "$fs_type" != "btrfs" ]; then
        echo "Error: $TARGET_DIR is not on a btrfs filesystem."
        exit 1
    fi
fi

# 3. Create snapshot root
SNAPSHOT_ROOT="/home/chuck/snapshots"
echo "Creating snapshot root at $SNAPSHOT_ROOT..."
mkdir -p "$SNAPSHOT_ROOT"

# 4. Install WS scripts
INSTALL_DIR="/usr/local/bin"
echo "Installing scripts to $INSTALL_DIR..."

SCRIPTS=(
    "ws_dev_snapshot.sh"
    "ws_dev_export_and_push.sh"
    "ws_dev_run_month.sh"
    "LS_request.sh"
)

for script in "${SCRIPTS[@]}"; do
    if [ -f "$script" ]; then
        sudo cp "$script" "$INSTALL_DIR/"
        sudo chmod +x "$INSTALL_DIR/$script"
        echo "Installed $script"
    else
        echo "Warning: $script not found in current directory. Skipping copy."
    fi
done

# 5. Configuration
CONFIG_DIR="/home/chuck/.config/dev-backup"
mkdir -p "$CONFIG_DIR"
CONFIG_FILE="$CONFIG_DIR/config.env"

if [ ! -f "$CONFIG_FILE" ]; then
    echo "Creating default config at $CONFIG_FILE..."
    cat <<EOF > "$CONFIG_FILE"
# Configuration for Dev Backup System
LS_HOST="localhost" # Change to LS IP/Hostname
LS_USER="chuck"      # User on LS
LS_KEY_PATH="/srv/btrfs-backups/dev/keys/ls_dev_backup.pub" # Path to pubkey if local, or fetch remotely
EOF
fi

echo "WS Initialization complete."
