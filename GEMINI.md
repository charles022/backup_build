# TCLD Tools - Btrfs Snapshot & Subvolume Management

## Project Overview
This project contains a suite of Bash scripts designed to simplify the management of Btrfs subvolumes and snapshots. Its primary capability is converting existing directories into Btrfs subvolumes in-place without data duplication (using reflink-enabled `rsync`) and creating periodic snapshots.

The core problem it solves is migrating standard directories to subvolumes to enable snapshotting, and then automating that snapshot process.

## Key Files
*   **`take_snapshot.sh`**: The main orchestration script. It:
    *   Verifies that target paths (e.g., `/home`, `$HOME/code`) are subvolumes.
    *   Automatically invokes `convert_to_subvolume.sh` if a target is a directory instead of a subvolume.
    *   Creates read-only snapshots in `/snapshot/<base_name>/<timestamp>`.
*   **`convert_to_subvolume.sh`**: A utility script that:
    *   Checks if a directory is suitable for conversion (on Btrfs, not already a subvolume, not in use).
    *   Uses `rsync --clone-dest` to efficiently "copy" data to a new subvolume using reflinks (preserving space).
    *   Swaps the old directory with the new subvolume.
*   **`test_convert_to_subvolume.sh`**: A comprehensive test suite that verifies:
    *   Argument handling and error states.
    *   Detection of "in-use" directories (using `lsof`).
    *   Successful conversion and data integrity checks.

## Usage & Commands

### Prerequisites
*   **Filesystem**: The target directories must reside on a **Btrfs** filesystem.
*   **Tools**:
    *   `rsync` (must support `--clone-dest` and extended attributes).
    *   `btrfs-progs`
    *   `findmnt`
    *   `lsof`
    *   `sudo` access is required for all subvolume operations.

### Running the Tools

**1. Create Snapshots**
To snapshot the configured paths (default: `/home` and `$HOME/code`):
```bash
./take_snapshot.sh
```
*Note: This script calls `sudo` internally.*

**2. Convert a Directory Manually**
To convert a specific directory to a subvolume:
```bash
./convert_to_subvolume.sh /path/to/directory
```

**3. Run Tests**
To verify the tools are working correctly on your system:
```bash
./test_convert_to_subvolume.sh
```
*Note: This creates a temporary test directory in the current directory or `$HOME` to run safe experiments.*

## Development Conventions
*   **Safety**: Scripts enable strict error checking (`set -e` or `set -euo pipefail`).
*   **idempotency**: The snapshot script checks if a subvolume already exists before attempting creation or conversion.
*   **Privilege Management**: Scripts are designed to be run by a user but invoke `sudo` for specific privileged commands (like `btrfs subvolume ...`).
*   **Testing**: New features should be verified using the `test_convert_to_subvolume.sh` harness, which mocks filesystem states and checks exit codes.
