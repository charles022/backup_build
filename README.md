# TCLD Tools

A suite of Bash scripts for managing Btrfs subvolumes and automating daily snapshots.

## Features

*   **Automated Snapshots**: `take_snapshot.sh` creates read-only snapshots of specified subvolumes (default: `/home` and `~/code`).
*   **In-Place Conversion**: `convert_to_subvolume.sh` converts standard directories into Btrfs subvolumes efficiently using `cp --reflink`, preserving disk space.
*   **Systemd Integration**: Includes service and timer files to run snapshots automatically on a daily schedule.

## Scripts

### `take_snapshot.sh`
The main orchestration script.
*   **Usage**: `./take_snapshot.sh`
*   **Function**:
    1.  Checks if configured paths (e.g., `~/code`) are subvolumes.
    2.  If a path is a directory, it invokes `convert_to_subvolume.sh` to convert it.
    3.  Creates a read-only snapshot in `/snapshot/<name>/<timestamp>`.

### `convert_to_subvolume.sh`
A utility script to convert a standard directory to a Btrfs subvolume.
*   **Usage**: `./convert_to_subvolume.sh /path/to/dir`
*   **Function**:
    1.  Moves the original directory to a backup location.
    2.  Creates a new subvolume at the original path.
    3.  Reflinks data from the backup to the new subvolume (instant copy, no extra space used).
    4.  Removes the backup upon success.

### `prune_snapshots.py`
Manages snapshot retention.
*   **Usage**: `./prune_snapshots.py`
*   **Policy**:
    *   Keeps all snapshots from the last 30 days.
    *   For snapshots older than 30 days, keeps the earliest snapshot of each month.
    *   Deletes all others.

### `install_service.sh`
Sets up the systemd timer for daily snapshots.
*   **Usage**: `./install_service.sh`
*   **Function**: Installs `take-snapshot.service` and `take-snapshot.timer` to `/etc/systemd/system/` and enables the timer.

## Installation & Setup

1.  **Prerequisites**:
    *   The target directories must be on a **Btrfs** filesystem.
    *   Root/Sudo privileges are required for subvolume operations.

2.  **Install Scripts**:
    Copy the core scripts to your local bin directory (required for the systemd service):
    ```bash
    mkdir -p ~/.local/bin
    cp take_snapshot.sh convert_to_subvolume.sh prune_snapshots.py ~/.local/bin/
    chmod +x ~/.local/bin/take_snapshot.sh ~/.local/bin/convert_to_subvolume.sh ~/.local/bin/prune_snapshots.py
    ```

3.  **Setup Automation**:
    Run the installer to schedule daily snapshots:
    ```bash
    ./install_service.sh
    ```

## Snapshot Management

Snapshots are stored in `/snapshot/`.

*   **List snapshots**:
    ```bash
    sudo btrfs subvolume list -s /snapshot
    ```

*   **Check disk usage**:
    ```bash
    sudo btrfs filesystem du -s -h /snapshot/*/*
    ```