#!/bin/bash
set -euo pipefail

SERVICE_NAME="take-snapshot"
SERVICE_FILE="${SERVICE_NAME}.service"
TIMER_FILE="${SERVICE_NAME}.timer"
INSTALL_DIR="/etc/systemd/system"

# Install scripts to ~/.local/bin with backup
SCRIPTS_TO_INSTALL=("take_snapshot.sh" "convert_to_subvolume.sh" "prune_snapshots.py")
LOCAL_BIN="$HOME/.local/bin"
BACKUP_DIR="$LOCAL_BIN/old"

mkdir -p "$LOCAL_BIN"

for script in "${SCRIPTS_TO_INSTALL[@]}"; do
    # Check if source file exists in current directory
    if [ -f "$script" ]; then
        target="$LOCAL_BIN/$script"
        
        # Backup existing file if it exists
        if [ -f "$target" ]; then
            mkdir -p "$BACKUP_DIR"
            timestamp=$(date +%Y%m%d_%H%M%S)
            echo "Backing up existing $script to $BACKUP_DIR/$script-$timestamp"
            mv "$target" "$BACKUP_DIR/$script-$timestamp"
        fi
        
        echo "Installing $script to $LOCAL_BIN..."
        cp "$script" "$target"
        chmod +x "$target"
    else
        echo "Warning: Source file '$script' not found in current directory. Skipping installation."
    fi
done

echo "Installing systemd service and timer..."

# Install service file
# We use 'cp' but we could also use 'install'. 
# We need sudo to write to /etc/systemd/system
if [ -f "$SERVICE_FILE" ]; then
    echo "Copying $SERVICE_FILE to $INSTALL_DIR..."
    sudo cp "$SERVICE_FILE" "$INSTALL_DIR/"
    sudo chmod 644 "$INSTALL_DIR/$SERVICE_FILE"
else
    echo "Error: $SERVICE_FILE not found in current directory."
    exit 1
fi

# Install timer file
if [ -f "$TIMER_FILE" ]; then
    echo "Copying $TIMER_FILE to $INSTALL_DIR..."
    sudo cp "$TIMER_FILE" "$INSTALL_DIR/"
    sudo chmod 644 "$INSTALL_DIR/$TIMER_FILE"
else
    echo "Error: $TIMER_FILE not found in current directory."
    exit 1
fi

echo "Reloading systemd daemon..."
sudo systemctl daemon-reload

echo "Enabling and starting the timer..."
sudo systemctl enable --now "$TIMER_FILE"

echo "Status of the timer:"
systemctl status "$TIMER_FILE" --no-pager

echo "Setup complete. The snapshot service is scheduled to run daily."
