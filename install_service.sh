#!/bin/bash
set -euo pipefail

SERVICE_NAME="take-snapshot"
SERVICE_FILE="${SERVICE_NAME}.service"
TIMER_FILE="${SERVICE_NAME}.timer"
INSTALL_DIR="/etc/systemd/system"

# Check if the snapshot script exists where expected
TARGET_SCRIPT="$HOME/.local/bin/take_snapshot.sh"
if [ ! -f "$TARGET_SCRIPT" ]; then
    echo "Warning: $TARGET_SCRIPT not found."
    echo "Please ensure 'take_snapshot.sh' is installed to ~/.local/bin/ before the service runs."
fi

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
