# Repository Guidelines

## Project Structure & Module Organization
The rebuild lives under `rust-rebuild/` and will become the only retained tree. The workspace is organized as a Cargo multi-crate setup:
- `rust-rebuild/crates/dev-backup-cli` — CLI binary (`dev-backup`) and command wiring.
- `rust-rebuild/crates/dev-backup-core` — config, manifest, and policy logic.
- `rust-rebuild/crates/dev-backup-storage` — artifact parsing, crypto, and cloud stubs.
- `rust-rebuild/crates/dev-backup-btrfs` — Btrfs subprocess wrappers.
- `rust-rebuild/docs` — build and config templates.
- `rust-rebuild/systemd` — service and timer units.

## Build, Test, and Development Commands
Key commands for local development:
- `cd rust-rebuild && cargo build --release` — build the CLI.
- `sudo install -m 755 target/release/dev-backup /usr/local/bin/` — install the binary.
- `sudo mkdir -p /etc/dev-backup && sudo cp docs/config.example.toml /etc/dev-backup/config.toml` — create config.
- `sudo dev-backup --config /etc/dev-backup/config.toml init ls` — initialize LS layout + manifest.
- `dev-backup --config /etc/dev-backup/config.toml init ws` — initialize WS snapshot root.

## Coding Style & Naming Conventions
- Rust 2021 edition; format with `cargo fmt` and lint with `cargo clippy`.
- Keep module boundaries clear: CLI orchestration in `dev-backup-cli`, core logic in `dev-backup-core`.
- Use explicit names for Btrfs operations (for example `snapshot_readonly`, `send_incremental_to_file`).

## Testing Guidelines
No automated tests yet. Preferred additions: unit tests for manifest parsing and policy decisions, and integration tests with a disposable Btrfs volume. Run `cargo test` after adding tests.

## Commit & Pull Request Guidelines
No established commit format. Use imperative, scoped summaries (for example `Implement artifact build pipeline`). PRs should list commands run and any system prerequisites (Btrfs, age, zstd, R2 credentials).

## Security & Configuration Tips
- Keep secrets out of the repo; use `/etc/dev-backup/config.toml` and `rclone`/S3 credentials.
- LS private key should live under `/srv/btrfs-backups/dev/keys` and never be copied to WS.

## Notes
- `dev-backup` expects a TOML config (default `/etc/dev-backup/config.toml`). Use `docs/config.example.toml` as the base.
- R2 and age support are stubbed in `crates/dev-backup-storage/src/cloud.rs` and `crates/dev-backup-storage/src/crypto.rs`.
- Btrfs wrappers in `crates/dev-backup-btrfs/src/lib.rs` are implemented for snapshot/send/receive via subprocess.

## Next Steps
1. Implement artifact build/restore/sync flows with streaming pipes (btrfs → zstd → age).
2. Add R2 integration using an S3-compatible crate and wire sync push/pull.
3. Expand ws run-month using the policy module + manifest pull.
