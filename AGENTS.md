# Repository Guidelines

## Project Structure & Module Organization
This repository is a **Bash script suite**; there is no `src/` or `tests/` tree. All logic lives in top-level `.sh` files, grouped by purpose:
- Initialization: `init_LS.sh`, `init_WS.sh`
- Snapshot + export: `ws_dev_snapshot.sh`, `ws_dev_export_and_push.sh`, `ws_dev_run_month.sh`
- LS registry + planning: `ls_dev_register_artifact.sh`, `ls_dev_plan_restore.sh`, `ls_dev_latest_label.sh`
- Cloud sync: `push_to_cloud.sh`, `pull_from_cloud.sh`, `store_on_LS.sh`
- Restore: `restore_backup_LS.sh`, `LS_send.sh`, `LS_request.sh`
Reference docs are in `README.md`. Runtime data lives outside the repo (for example `/srv/btrfs-backups/dev` and `/home/chuck/code`).

## Build, Test, and Development Commands
There is no build step. Key operational commands:
- `sudo ./init_LS.sh` — set up LS dependencies, repo layout, and age keys.
- `./init_WS.sh` — ensure `/home/chuck/code` is a Btrfs subvolume and install WS deps.
- `./ws_dev_run_month.sh YYYY-MM` — create snapshot and export the monthly artifact.
- `./push_to_cloud.sh` — mirror LS artifacts/manifests to Cloudflare R2.
- `./pull_from_cloud.sh latest` + `./store_on_LS.sh /tmp/dev-backup-cloud-pull` — rehydrate LS artifacts.
- `./restore_backup_LS.sh latest` or `./LS_request.sh latest` — restore LS or stream to WS.

## Coding Style & Naming Conventions
- Use `#!/bin/bash` and `set -euo pipefail`.
- Indent with 4 spaces; keep scripts linear and readable.
- Quote variable expansions and use uppercase for configuration variables (for example `BASE_DIR`, `LS_HOST`).
- Script naming follows the pattern `ws_`/`ls_` + `dev_` + action (for example `ws_dev_snapshot.sh`).

## Testing Guidelines
No automated tests are present. Validate changes by running the relevant script in a safe environment and checking manifest integrity (`/srv/btrfs-backups/dev/manifests/snapshots_v2.tsv`). For lightweight validation, use `bash -n <script>` and run `./ls_dev_plan_restore.sh <label>` to confirm restore chains.

## Commit & Pull Request Guidelines
Git history only includes an `init commit`, so no established convention exists. Use clear, imperative subjects (for example `Add restore chain validation`) and describe impact on LS/WS behavior. PRs should include: a short purpose statement, commands run, and any safety considerations (destructive steps, required privileges, or data migrations).

## Security & Configuration Tips
- Age keys live under `/srv/btrfs-backups/dev/keys`; never commit secrets.
- WS config is read from `~/.config/dev-backup/config.env`.
- R2 credentials should be handled through `rclone` configuration, not hard-coded.
- Avoid editing manifests by hand; use the registration scripts to keep metadata consistent.
