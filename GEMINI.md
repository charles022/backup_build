# Dev Backup System

## Project Overview

**Dev Backup** is a robust, Btrfs-based backup, restore, and synchronization system designed for developers. It ensures data safety through local snapshots and encrypted cloud storage (Cloudflare R2), while maintaining high performance for local development.

### Core Architecture

*   **Workstation (WS):** The primary development machine where snapshots are created.
*   **Local Server (LS):** The canonical backup repository that handles artifact storage, encryption management, and cloud synchronization.
*   **Cloudflare R2:** Cold, encrypted object storage for redundancy.

### Key Features

*   **Btrfs Integration:** deeply integrated with Btrfs for efficient snapshots and send/receive operations.
*   **Artifacts:** Supports **Anchor** (full) and **Incremental** (delta) backup artifacts.
*   **Security:** Client-side encryption using `age` and compression with `zstd` before data leaves the local environment.
*   **Deterministic Restore:** Manifest-based restore planning ensures exact state recovery.
*   **Rust Workspace:** Modular design with separate crates for CLI, core logic, storage, and Btrfs interactions.

## Project Structure

The project is organized as a Rust workspace:

*   `crates/dev-backup-cli`: The main command-line interface (`dev-backup`).
*   `crates/dev-backup-core`: Core logic for configuration, manifests, and policy decisions.
*   `crates/dev-backup-storage`: Handles artifact processing, cloud interaction (S3/R2), and crypto.
*   `crates/dev-backup-btrfs`: Wrapper for executing Btrfs shell commands.
*   `docs/`: Documentation and example configuration.

## Building and Installation

### Prerequisites

*   Rust toolchain (stable)
*   `btrfs-progs` (installed on the system)
*   `zstd` (installed on the system)
*   `age` (installed on the system)

### Build Command

To build the release binary:

```bash
cargo build --release
```

### Installation

Install the binary to your system path:

```bash
sudo install -m 755 target/release/dev-backup /usr/local/bin/
```

## Configuration

The system is configured via a TOML file, typically located at `/etc/dev-backup/config.toml`.

**Example Configuration:**

```toml
[paths]
dataset = "/home/chuck/code"
snapshots = "/home/chuck/snapshots"
ls_root = "/srv/btrfs-backups/dev"

[cloud]
endpoint = "https://<ACCOUNT_ID>.r2.cloudflarestorage.com"
bucket = "dev-backups"
access_key = "<R2_ACCESS_KEY>"
secret_key = "<R2_SECRET_KEY>"

[crypto]
age_public_key = "age1..."
age_private_key_path = "/srv/btrfs-backups/dev/keys/ls_dev_backup.key"

[remote]
ls_host = "localhost"
ls_user = "chuck"
```

## Usage Workflows

### Initialization

*   **Local Server (LS):** `sudo dev-backup --config <path> init ls`
*   **Workstation (WS):** `dev-backup --config <path> init ws`

### Monthly Backup (on WS)

Triggers the monthly snapshot and artifact creation process:

```bash
dev-backup ws run-month --label YYYY-MM
```

### Cloud Sync (on LS)

Pushes local artifacts and manifests to Cloudflare R2:

```bash
dev-backup sync push
```

### Restore (on LS)

1.  **Pull from Cloud:** `dev-backup sync pull --label latest`
2.  **Hydrate Snapshots:** `dev-backup restore hydrate --label latest`
3.  **Apply to Worktree:** `dev-backup restore apply --label latest`

## Development Conventions

*   **Error Handling:** Uses `anyhow` for flexible error propagation.
*   **CLI:** Uses `clap` for argument parsing.
*   **Async/Sync:** Uses `tokio` for the runtime, but relies on `std::process::Command` for invoking external tools like `btrfs`, `zstd`, and `age`.
*   **Code Style:** Follows standard Rust formatting (`cargo fmt`) and clippy suggestions.
*   **Testing:** Unit tests are located within the `src/` directories or in a separate `tests/` folder. Run with `cargo test`.
