# Repository Guidelines

## Project Structure & Module Organization
This repository is a Rust Cargo workspace with multiple crates:
- `crates/dev-backup-cli` — CLI binary (`dev-backup`) and orchestration.
- `crates/dev-backup-core` — config, manifest, and policy logic.
- `crates/dev-backup-storage` — artifact parsing, crypto helpers, and R2 client.
- `crates/dev-backup-btrfs` — Btrfs subprocess wrappers.
- `docs` — build/config templates.
- `systemd` — unit and timer files for scheduled runs.

## Build, Test, and Development Commands
Key commands for local development:
- `cargo build --release` — build the CLI.
- `sudo install -m 755 target/release/dev-backup /usr/local/bin/` — install the binary.
- `sudo mkdir -p /etc/dev-backup && sudo cp docs/config.example.toml /etc/dev-backup/config.toml` — create config.
- `sudo dev-backup --config /etc/dev-backup/config.toml init ls` — initialize LS layout + manifest.
- `dev-backup --config /etc/dev-backup/config.toml init ws` — initialize WS snapshot root.
- `dev-backup ws request YYYY-MM` — stream a snapshot from LS into the WS snapshot root and update the working tree.
- `dev-backup ws request latest --auto-parent` — use the latest local snapshot as an incremental parent if available.

## Coding Style & Naming Conventions
- Rust 2021 edition; format with `cargo fmt` and lint with `cargo clippy`.
- Keep module boundaries clear: CLI orchestration in `dev-backup-cli`, core logic in `dev-backup-core`.
- Use explicit names for Btrfs operations (for example `snapshot_readonly`, `send_incremental_to_file`).

## Testing Guidelines
Integration tests live in `crates/dev-backup-cli/tests/restore_plan.rs`. Run `cargo test` and prefer unit tests for manifest/policy logic plus integration tests with a disposable Btrfs volume.

## Commit & Pull Request Guidelines
No established commit format. Use imperative, scoped summaries (for example `Implement artifact build pipeline`). PRs should list commands run and any system prerequisites (Btrfs, age, zstd, R2 credentials).

## Security & Configuration Tips
- Keep secrets out of the repo; use `/etc/dev-backup/config.toml` for credentials.
- LS private key should live under `/srv/btrfs-backups/dev/keys` and never be copied to WS.

## Notes
- `dev-backup` expects a TOML config (default `/etc/dev-backup/config.toml`). Use `docs/config.example.toml` as the base.
- R2 integration is implemented in `crates/dev-backup-storage/src/cloud.rs` using the AWS S3 SDK.
- Artifact build/restore uses streaming pipelines (`btrfs` → `zstd` → `age`) in `crates/dev-backup-cli/src/main.rs`.
- `ws run-month` now pulls the manifest (local LS path or R2) and applies the anchor policy before building artifacts.
- `ws request` supports remote overrides via `--ls-host` and `--ls-user`.
- `ws request --auto-parent` picks the latest local `dev@YYYY-MM` snapshot (excluding the target) as the parent.
- `init ls` now generates an age keypair in `/srv/btrfs-backups/dev/keys` if missing.

## Next Steps
1. Add validation that a chosen parent is an ancestor before attempting `btrfs send -p`.
2. Add integration tests for `ws request` flows and a basic end-to-end restore scenario.
3. Add key rotation and manifest integrity verification (SHA256 checks on restore).
