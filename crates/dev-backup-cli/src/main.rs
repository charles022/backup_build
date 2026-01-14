use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use dev_backup_btrfs as btrfs;
use dev_backup_core::config::Config;
use dev_backup_core::manifest::{ManifestRecord, ManifestStore};
use dev_backup_core::policy::{decide_snapshot_type, PolicyInput, SnapshotDecision};
use dev_backup_storage::artifact::{parse_artifact_filename, sha256_file, ArtifactType};
use dev_backup_storage::cloud::{R2Client, R2Config};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

#[derive(Parser)]
#[command(name = "dev-backup", version, about = "Btrfs backup and restore tooling")]
struct Cli {
    #[arg(long, default_value = "/etc/dev-backup/config.toml")]
    config: String,
    #[command(subcommand)]
    command: CliCommand,
}

#[derive(Subcommand)]
enum CliCommand {
    Init {
        #[arg(value_enum)]
        target: InitTarget,
    },
    Snapshot {
        label: String,
    },
    Artifact {
        #[command(subcommand)]
        action: ArtifactCommand,
    },
    Restore {
        #[command(subcommand)]
        action: RestoreCommand,
    },
    Sync {
        #[command(subcommand)]
        action: SyncCommand,
    },
    Ws {
        #[command(subcommand)]
        action: WsCommand,
    },
    Ls {
        #[command(subcommand)]
        action: LsCommand,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum InitTarget {
    Ls,
    Ws,
}

#[derive(Subcommand)]
enum ArtifactCommand {
    Build {
        label: String,
        parent: Option<String>,
    },
    Register {
        path: String,
    },
}

#[derive(Subcommand)]
enum RestoreCommand {
    Plan { label: String },
    Hydrate { label: String },
    Apply { label: String },
}

#[derive(Subcommand)]
enum SyncCommand {
    Push,
    Pull { label: String, dest: Option<String> },
}

#[derive(Subcommand)]
enum WsCommand {
    RunMonth { label: String },
    Request {
        label: String,
        parent: Option<String>,
        #[arg(long)]
        auto_parent: bool,
        #[arg(long)]
        ls_host: Option<String>,
        #[arg(long)]
        ls_user: Option<String>,
    },
}

#[derive(Subcommand)]
enum LsCommand {
    Send { label: String, parent: Option<String> },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        CliCommand::Init { target } => init(&cli.config, target),
        CliCommand::Snapshot { label } => snapshot(&cli.config, &label),
        CliCommand::Artifact { action } => artifact(&cli.config, action),
        CliCommand::Restore { action } => restore(&cli.config, action),
        CliCommand::Sync { action } => sync(&cli.config, action).await,
        CliCommand::Ws { action } => ws(&cli.config, action).await,
        CliCommand::Ls { action } => ls(&cli.config, action),
    }
}

fn load_config(path: &str) -> Result<Config> {
    Config::load(path).with_context(|| format!("config required at {path}"))
}

fn init(config_path: &str, target: InitTarget) -> Result<()> {
    let cfg = load_config(config_path)?;
    match target {
        InitTarget::Ls => {
            let base = PathBuf::from(&cfg.paths.ls_root);
            let dirs = [
                base.join("artifacts/anchors"),
                base.join("artifacts/incr"),
                base.join("manifests"),
                base.join("keys"),
                base.join("restore/snapshots"),
                base.join("tmp"),
                base.join("logs"),
                base.join("locks"),
            ];
            for dir in dirs {
                btrfs::ensure_dir(&dir)?;
            }
            let manifest_path = base.join("manifests/snapshots_v2.tsv");
            let store = ManifestStore::new(&manifest_path);
            store.ensure_initialized()?;
            let private_key = base.join("keys/ls_dev_backup.key");
            let public_key = base.join("keys/ls_dev_backup.pub");
            ensure_age_keypair(&private_key, &public_key)?;
            println!("LS initialized at {}", base.display());
        }
        InitTarget::Ws => {
            if !btrfs::is_btrfs_mount(&cfg.paths.dataset)? {
                return Err(anyhow!("dataset path is not on btrfs: {}", cfg.paths.dataset));
            }
            btrfs::ensure_dir(Path::new(&cfg.paths.snapshots))?;
            println!("WS initialized. Snapshot root at {}", cfg.paths.snapshots);
        }
    }
    Ok(())
}

fn snapshot(config_path: &str, label: &str) -> Result<()> {
    let cfg = load_config(config_path)?;
    ensure_label(label)?;
    let snapshot_path = format!("{}/dev@{}", cfg.paths.snapshots, label);
    if Path::new(&snapshot_path).exists() {
        println!("Snapshot already exists: {snapshot_path}");
        return Ok(());
    }
    btrfs::snapshot_readonly(&cfg.paths.dataset, &snapshot_path)?;
    println!("Created snapshot {snapshot_path}");
    Ok(())
}

fn artifact(config_path: &str, action: ArtifactCommand) -> Result<()> {
    let cfg = load_config(config_path)?;
    match action {
        ArtifactCommand::Build { label, parent } => build_artifact(&cfg, &label, parent.as_deref()),
        ArtifactCommand::Register { path } => register_artifact(&cfg, &path),
    }
}

fn build_artifact(cfg: &Config, label: &str, parent: Option<&str>) -> Result<()> {
    ensure_label(label)?;
    if let Some(parent_label) = parent {
        ensure_label(parent_label)?;
    }

    let snapshot_path = format!("{}/dev@{}", cfg.paths.snapshots, label);
    if !Path::new(&snapshot_path).exists() {
        return Err(anyhow!("snapshot not found: {snapshot_path}"));
    }

    let parent_path = parent.map(|p| format!("{}/dev@{}", cfg.paths.snapshots, p));
    if let Some(ref path) = parent_path {
        if !Path::new(path).exists() {
            return Err(anyhow!("parent snapshot not found: {path}"));
        }
    }

    let output_name = if let Some(parent_label) = parent {
        format!("dev@{label}.incr.from_{parent_label}.send.zst.age")
    } else {
        format!("dev@{label}.full.send.zst.age")
    };

    let public_key = cfg
        .crypto
        .as_ref()
        .and_then(|crypto| crypto.age_public_key.as_deref())
        .ok_or_else(|| anyhow!("age_public_key is required in config"))?;

    run_send_pipeline(&snapshot_path, parent_path.as_deref(), &output_name, public_key)?;
    println!("Artifact created: {output_name}");
    Ok(())
}

fn register_artifact(cfg: &Config, path: &str) -> Result<()> {
    let filename = Path::new(path)
        .file_name()
        .and_then(|v| v.to_str())
        .ok_or_else(|| anyhow!("invalid artifact path: {path}"))?;
    let info = parse_artifact_filename(filename)
        .ok_or_else(|| anyhow!("invalid artifact name: {filename}"))?;

    let dest_dir = match info.artifact_type {
        ArtifactType::Anchor => Path::new(&cfg.paths.ls_root).join("artifacts/anchors"),
        ArtifactType::Incremental => Path::new(&cfg.paths.ls_root).join("artifacts/incr"),
    };
    btrfs::ensure_dir(&dest_dir)?;

    let dest_path = dest_dir.join(&info.filename);
    fs::rename(path, &dest_path)
        .with_context(|| format!("failed to move artifact to {}", dest_path.display()))?;

    let bytes = dest_path.metadata()?.len();
    let sha256 = sha256_file(dest_path.to_str().unwrap_or_default())?;

    let record = ManifestRecord {
        ts: OffsetDateTime::now_utc().format(&Rfc3339)?,
        label: info.label,
        record_type: match info.artifact_type {
            ArtifactType::Anchor => "anchor".to_string(),
            ArtifactType::Incremental => "incremental".to_string(),
        },
        parent: info.parent.unwrap_or_default(),
        bytes,
        sha256,
        local_path: dest_path.to_string_lossy().to_string(),
        object_key: String::new(),
    };

    let manifest_path = Path::new(&cfg.paths.ls_root).join("manifests/snapshots_v2.tsv");
    let store = ManifestStore::new(&manifest_path);
    store.ensure_initialized()?;
    store.append_record(&record)?;

    println!("Registered artifact and updated manifest.");
    Ok(())
}

fn restore(config_path: &str, action: RestoreCommand) -> Result<()> {
    let cfg = load_config(config_path)?;
    match action {
        RestoreCommand::Plan { label } => {
            let plan = plan_restore(&cfg, &label)?;
            for record in plan {
                println!("{}", record.local_path);
            }
            Ok(())
        }
        RestoreCommand::Hydrate { label } => hydrate_restore(&cfg, &label),
        RestoreCommand::Apply { label } => apply_restore(&cfg, &label),
    }
}

fn plan_restore(cfg: &Config, label: &str) -> Result<Vec<ManifestRecord>> {
    let manifest_path = Path::new(&cfg.paths.ls_root).join("manifests/snapshots_v2.tsv");
    let store = ManifestStore::new(&manifest_path);
    let records = store.read_records()?;
    if records.is_empty() {
        return Err(anyhow!("manifest is empty"));
    }

    let resolved_label = resolve_label_input(&records, label)?;
    let mut latest_by_label: HashMap<String, ManifestRecord> = HashMap::new();
    for record in records {
        latest_by_label.insert(record.label.clone(), record);
    }

    let mut chain = Vec::new();
    let mut current = resolved_label;
    loop {
        let record = latest_by_label
            .get(&current)
            .ok_or_else(|| anyhow!("label not found in manifest: {current}"))?
            .clone();
        chain.push(record.clone());

        if record.record_type == "anchor" {
            break;
        }

        if record.parent.is_empty() {
            return Err(anyhow!("incremental record missing parent for {current}"));
        }

        let parent_snapshot = format!(
            "{}/restore/snapshots/dev@{}",
            cfg.paths.ls_root, record.parent
        );
        if Path::new(&parent_snapshot).exists() {
            break;
        }

        current = record.parent.clone();
    }

    chain.reverse();
    Ok(chain)
}

fn hydrate_restore(cfg: &Config, label: &str) -> Result<()> {
    let private_key = cfg
        .crypto
        .as_ref()
        .and_then(|crypto| crypto.age_private_key_path.as_deref())
        .ok_or_else(|| anyhow!("age_private_key_path is required in config"))?;

    let restore_dir = format!("{}/restore/snapshots", cfg.paths.ls_root);
    btrfs::ensure_dir(Path::new(&restore_dir))?;

    let plan = plan_restore(cfg, label)?;
    for record in plan {
        let snapshot_path = format!("{restore_dir}/dev@{}", record.label);
        if Path::new(&snapshot_path).exists() {
            println!("Snapshot already hydrated: {snapshot_path}");
            continue;
        }
        if record.local_path.is_empty() {
            return Err(anyhow!("missing local_path for {}", record.label));
        }
        if !Path::new(&record.local_path).exists() {
            return Err(anyhow!("artifact missing: {}", record.local_path));
        }
        println!("Hydrating dev@{}...", record.label);
        run_receive_pipeline(&record.local_path, &restore_dir, private_key)?;
    }
    Ok(())
}

fn apply_restore(cfg: &Config, label: &str) -> Result<()> {
    let resolved_label = resolve_label_from_manifest(cfg, label)?;
    let restore_snapshot = format!(
        "{}/restore/snapshots/dev@{}",
        cfg.paths.ls_root, resolved_label
    );
    if !Path::new(&restore_snapshot).exists() {
        return Err(anyhow!("restore snapshot missing: {restore_snapshot}"));
    }

    let worktree = Path::new(&cfg.paths.dataset);
    if worktree.exists() {
        if btrfs::subvolume_exists(worktree.to_str().unwrap_or_default())? {
            btrfs::subvolume_delete(worktree.to_str().unwrap_or_default())?;
        } else {
            let backup_name = format!(
                "{}_backup_{}",
                cfg.paths.dataset,
                OffsetDateTime::now_utc().unix_timestamp()
            );
            fs::rename(worktree, &backup_name)
                .with_context(|| format!("failed to move existing worktree to {backup_name}"))?;
        }
    }

    btrfs::snapshot_writable(&restore_snapshot, worktree.to_str().unwrap_or_default())?;
    println!("Working tree updated to dev@{resolved_label}");
    Ok(())
}

async fn sync(config_path: &str, action: SyncCommand) -> Result<()> {
    let cfg = load_config(config_path)?;
    match action {
        SyncCommand::Push => sync_push(&cfg).await,
        SyncCommand::Pull { label, dest } => sync_pull(&cfg, &label, dest.as_deref()).await,
    }
}

async fn sync_push(cfg: &Config) -> Result<()> {
    let cloud = cfg
        .cloud
        .as_ref()
        .ok_or_else(|| anyhow!("cloud config is required"))?;
    let client = R2Client::new(R2Config {
        endpoint: cloud.endpoint.clone(),
        bucket: cloud.bucket.clone(),
        access_key: cloud.access_key.clone(),
        secret_key: cloud.secret_key.clone(),
    })
    .await?;

    let manifest_path = Path::new(&cfg.paths.ls_root).join("manifests/snapshots_v2.tsv");
    let store = ManifestStore::new(&manifest_path);
    let mut records = store.read_records()?;

    let mut changed = false;
    for record in &mut records {
        if !record.object_key.is_empty() {
            continue;
        }
        if record.local_path.is_empty() {
            return Err(anyhow!("missing local_path for {}", record.label));
        }
        let local_path = Path::new(&record.local_path);
        if !local_path.exists() {
            return Err(anyhow!("artifact missing: {}", record.local_path));
        }
        let object_key = build_object_key(&cfg.paths.ls_root, local_path);
        client
            .upload_object(&object_key, local_path.to_str().unwrap_or_default())
            .await?;
        record.object_key = object_key;
        changed = true;
    }

    if changed {
        store.write_records(&records)?;
    }

    client
        .upload_object(
            "manifests/snapshots_v2.tsv",
            manifest_path.to_str().unwrap_or_default(),
        )
        .await?;
    println!("Sync push complete");
    Ok(())
}

async fn sync_pull(cfg: &Config, label: &str, dest: Option<&str>) -> Result<()> {
    let cloud = cfg
        .cloud
        .as_ref()
        .ok_or_else(|| anyhow!("cloud config is required"))?;
    let client = R2Client::new(R2Config {
        endpoint: cloud.endpoint.clone(),
        bucket: cloud.bucket.clone(),
        access_key: cloud.access_key.clone(),
        secret_key: cloud.secret_key.clone(),
    })
    .await?;

    let dest_dir = dest.unwrap_or("/tmp/dev-backup-cloud-pull");
    btrfs::ensure_dir(Path::new(dest_dir))?;

    let manifest_path = Path::new(dest_dir).join("snapshots_v2.tsv");
    client
        .download_object(
            "manifests/snapshots_v2.tsv",
            manifest_path.to_str().unwrap_or_default(),
        )
        .await?;

    let store = ManifestStore::new(&manifest_path);
    let records = store.read_records()?;
    if records.is_empty() {
        return Err(anyhow!("downloaded manifest is empty"));
    }

    let resolved_label = if label == "latest" {
        resolve_latest_label(&records)?.ok_or_else(|| anyhow!("no label found"))?
    } else {
        label.to_string()
    };

    let plan = plan_chain_from_records(&records, &resolved_label)?;
    for record in plan {
        if record.object_key.is_empty() {
            return Err(anyhow!("missing object_key for {}", record.label));
        }
        let dest_path = Path::new(dest_dir).join(&record.object_key);
        if let Some(parent) = dest_path.parent() {
            btrfs::ensure_dir(parent)?;
        }
        client
            .download_object(&record.object_key, dest_path.to_str().unwrap_or_default())
            .await?;
    }

    println!("Sync pull complete into {dest_dir}");
    Ok(())
}

fn plan_chain_from_records(records: &[ManifestRecord], label: &str) -> Result<Vec<ManifestRecord>> {
    let mut latest_by_label: HashMap<String, ManifestRecord> = HashMap::new();
    for record in records {
        latest_by_label.insert(record.label.clone(), record.clone());
    }

    let mut chain = Vec::new();
    let mut current = label.to_string();
    loop {
        let record = latest_by_label
            .get(&current)
            .ok_or_else(|| anyhow!("label not found in manifest: {current}"))?
            .clone();
        chain.push(record.clone());

        if record.record_type == "anchor" {
            break;
        }
        if record.parent.is_empty() {
            return Err(anyhow!("incremental record missing parent for {current}"));
        }
        current = record.parent.clone();
    }

    chain.reverse();
    Ok(chain)
}

fn resolve_latest_label(records: &[ManifestRecord]) -> Result<Option<String>> {
    let mut best: Option<(OffsetDateTime, String)> = None;
    for record in records {
        let ts = OffsetDateTime::parse(&record.ts, &Rfc3339)
            .with_context(|| format!("invalid timestamp: {}", record.ts))?;
        match &best {
            None => best = Some((ts, record.label.clone())),
            Some((best_ts, _)) if ts > *best_ts => best = Some((ts, record.label.clone())),
            _ => {}
        }
    }
    Ok(best.map(|(_, label)| label))
}

fn resolve_label_input(records: &[ManifestRecord], label: &str) -> Result<String> {
    if label == "latest" {
        return resolve_latest_label(records)?
            .ok_or_else(|| anyhow!("no label found in manifest"));
    }
    ensure_label(label)?;
    Ok(label.to_string())
}

fn resolve_label_from_manifest(cfg: &Config, label: &str) -> Result<String> {
    let manifest_path = Path::new(&cfg.paths.ls_root).join("manifests/snapshots_v2.tsv");
    let store = ManifestStore::new(&manifest_path);
    let records = store.read_records()?;
    if records.is_empty() {
        return Err(anyhow!("manifest is empty"));
    }
    resolve_label_input(&records, label)
}

fn build_object_key(ls_root: &str, local_path: &Path) -> String {
    let root = Path::new(ls_root);
    let key = local_path
        .strip_prefix(root)
        .unwrap_or(local_path)
        .to_string_lossy()
        .to_string();
    key.trim_start_matches('/').to_string()
}

async fn ws(config_path: &str, action: WsCommand) -> Result<()> {
    let cfg = load_config(config_path)?;
    match action {
        WsCommand::RunMonth { label } => ws_run_month(&cfg, &label).await,
        WsCommand::Request {
            label,
            parent,
            auto_parent,
            ls_host,
            ls_user,
        } => ws_request(
            &cfg,
            config_path,
            &label,
            parent.as_deref(),
            auto_parent,
            ls_host,
            ls_user,
        )
        .await,
    }
}

fn ls(config_path: &str, action: LsCommand) -> Result<()> {
    let cfg = load_config(config_path)?;
    match action {
        LsCommand::Send { label, parent } => ls_send(&cfg, &label, parent.as_deref()),
    }
}

fn ls_send(cfg: &Config, label: &str, parent: Option<&str>) -> Result<()> {
    let resolved_label = resolve_label_from_manifest(cfg, label)?;
    if let Some(parent_label) = parent {
        ensure_label(parent_label)?;
    }

    let snapshot_dir = format!("{}/restore/snapshots", cfg.paths.ls_root);
    let snapshot_path = format!("{snapshot_dir}/dev@{resolved_label}");
    if !Path::new(&snapshot_path).exists() {
        return Err(anyhow!("snapshot not found on LS: {snapshot_path}"));
    }

    let parent_path = parent.map(|p| format!("{snapshot_dir}/dev@{p}"));
    if let Some(ref path) = parent_path {
        if !Path::new(path).exists() {
            return Err(anyhow!("parent snapshot not found on LS: {path}"));
        }
    }

    let mut cmd = Command::new("btrfs");
    if let Some(parent_path) = parent_path.as_deref() {
        cmd.args(["send", "-p", parent_path, &snapshot_path]);
    } else {
        cmd.args(["send", &snapshot_path]);
    }

    let status = cmd
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .context("failed to run btrfs send")?;
    if !status.success() {
        return Err(anyhow!("btrfs send failed"));
    }
    Ok(())
}

fn ensure_label(label: &str) -> Result<()> {
    if !is_valid_label(label) {
        return Err(anyhow!("label must be YYYY-MM"));
    }
    Ok(())
}

fn ensure_age_keypair(private_path: &Path, public_path: &Path) -> Result<()> {
    if !private_path.exists() {
        let status = Command::new("age-keygen")
            .args(["-o", private_path.to_str().unwrap_or_default()])
            .status()
            .context("failed to run age-keygen")?;
        if !status.success() {
            return Err(anyhow!("age-keygen failed"));
        }
    }

    if !public_path.exists() {
        let output = Command::new("age-keygen")
            .args(["-y", private_path.to_str().unwrap_or_default()])
            .output()
            .context("failed to derive age public key")?;
        if !output.status.success() {
            return Err(anyhow!("age-keygen -y failed"));
        }
        fs::write(public_path, output.stdout)
            .with_context(|| format!("failed to write public key: {}", public_path.display()))?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let private_perm = fs::Permissions::from_mode(0o600);
        fs::set_permissions(private_path, private_perm)
            .with_context(|| format!("failed to set permissions on {}", private_path.display()))?;
        let public_perm = fs::Permissions::from_mode(0o644);
        fs::set_permissions(public_path, public_perm)
            .with_context(|| format!("failed to set permissions on {}", public_path.display()))?;
    }

    Ok(())
}

fn is_valid_label(label: &str) -> bool {
    let mut parts = label.split('-');
    let year = match parts.next() {
        Some(value) => value,
        None => return false,
    };
    let month = match parts.next() {
        Some(value) => value,
        None => return false,
    };
    if parts.next().is_some() || year.len() != 4 || month.len() != 2 {
        return false;
    }
    if !year.chars().all(|c| c.is_ascii_digit()) || !month.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    true
}

async fn ws_run_month(cfg: &Config, label: &str) -> Result<()> {
    ensure_label(label)?;
    let records = fetch_manifest_records_for_ws(cfg).await?;
    let sorted_records = sort_records_by_ts(&records)?;

    let decision = if sorted_records.is_empty() {
        SnapshotDecision::Anchor
    } else {
        decide_snapshot_type(&sorted_records, PolicyInput::default())?
    };

    let parent_label = match decision {
        SnapshotDecision::Anchor => None,
        SnapshotDecision::Incremental => Some(latest_label_from_records(&sorted_records)?),
    };

    snapshot_from_cfg(cfg, label)?;
    build_artifact(cfg, label, parent_label.as_deref())?;

    match parent_label {
        Some(parent) => println!("Run-month complete: incremental from {parent}"),
        None => println!("Run-month complete: anchor"),
    }
    Ok(())
}

async fn ws_request(
    cfg: &Config,
    config_path: &str,
    label: &str,
    parent: Option<&str>,
    auto_parent: bool,
    ls_host: Option<String>,
    ls_user: Option<String>,
) -> Result<()> {
    let resolved_label = resolve_label_for_ws_request(cfg, label).await?;
    let mut parent_label = parent.map(|value| value.to_string());
    if let Some(ref label) = parent_label {
        ensure_label(label)?;
    } else if auto_parent {
        parent_label = find_latest_local_snapshot_label(&cfg.paths.snapshots, &resolved_label)?;
    }

    btrfs::ensure_dir(Path::new(&cfg.paths.snapshots))?;
    let (host, user) = resolve_remote_target(cfg, ls_host, ls_user);

    let mut send_child = if is_local_host(&host) {
        spawn_local_ls_send(config_path, &resolved_label, parent_label.as_deref())?
    } else {
        spawn_remote_ls_send(&user, &host, &resolved_label, parent_label.as_deref())?
    };

    let send_stdout = send_child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("failed to capture ls send stdout"))?;

    let mut recv_child = Command::new("btrfs")
        .args(["receive", &cfg.paths.snapshots])
        .stdin(Stdio::from(send_stdout))
        .stderr(Stdio::inherit())
        .spawn()
        .context("failed to start btrfs receive")?;

    let recv_status = recv_child.wait().context("failed to wait on btrfs receive")?;
    let send_status = send_child.wait().context("failed to wait on ls send")?;

    if !send_status.success() {
        return Err(anyhow!("ls send failed"));
    }
    if !recv_status.success() {
        return Err(anyhow!("btrfs receive failed"));
    }

    let snapshot_path = format!("{}/dev@{}", cfg.paths.snapshots, resolved_label);
    if !Path::new(&snapshot_path).exists() {
        return Err(anyhow!("received snapshot missing: {snapshot_path}"));
    }

    update_worktree_from_snapshot(cfg, &snapshot_path, &resolved_label)?;
    Ok(())
}

async fn resolve_label_for_ws_request(cfg: &Config, label: &str) -> Result<String> {
    if label != "latest" {
        ensure_label(label)?;
        return Ok(label.to_string());
    }
    let records = fetch_manifest_records_for_ws(cfg).await?;
    if records.is_empty() {
        return Err(anyhow!("manifest unavailable to resolve latest label"));
    }
    resolve_latest_label(&records)?
        .ok_or_else(|| anyhow!("no label found in manifest"))
}

fn resolve_remote_target(
    cfg: &Config,
    ls_host: Option<String>,
    ls_user: Option<String>,
) -> (String, String) {
    let default_user = std::env::var("USER").unwrap_or_else(|_| "chuck".to_string());
    let host = ls_host
        .or_else(|| cfg.remote.as_ref().and_then(|remote| remote.ls_host.clone()))
        .unwrap_or_else(|| "localhost".to_string());
    let user = ls_user
        .or_else(|| cfg.remote.as_ref().and_then(|remote| remote.ls_user.clone()))
        .unwrap_or(default_user);
    (host, user)
}

fn is_local_host(host: &str) -> bool {
    host == "localhost" || host == "127.0.0.1"
}

fn spawn_local_ls_send(config_path: &str, label: &str, parent: Option<&str>) -> Result<std::process::Child> {
    let mut cmd = Command::new("dev-backup");
    cmd.args(["--config", config_path, "ls", "send", label]);
    if let Some(parent_label) = parent {
        cmd.arg(parent_label);
    }
    let child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .context("failed to spawn local ls send")?;
    Ok(child)
}

fn spawn_remote_ls_send(
    user: &str,
    host: &str,
    label: &str,
    parent: Option<&str>,
) -> Result<std::process::Child> {
    let target = format!("{user}@{host}");
    let mut cmd = Command::new("ssh");
    cmd.arg(target)
        .arg("dev-backup")
        .arg("--config")
        .arg("/etc/dev-backup/config.toml")
        .arg("ls")
        .arg("send")
        .arg(label);
    if let Some(parent_label) = parent {
        cmd.arg(parent_label);
    }
    let child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .context("failed to spawn remote ls send")?;
    Ok(child)
}

fn snapshot_from_cfg(cfg: &Config, label: &str) -> Result<()> {
    let snapshot_path = format!("{}/dev@{}", cfg.paths.snapshots, label);
    if Path::new(&snapshot_path).exists() {
        println!("Snapshot already exists: {snapshot_path}");
        return Ok(());
    }
    btrfs::snapshot_readonly(&cfg.paths.dataset, &snapshot_path)?;
    println!("Created snapshot {snapshot_path}");
    Ok(())
}

async fn fetch_manifest_records_for_ws(cfg: &Config) -> Result<Vec<ManifestRecord>> {
    let local_manifest = Path::new(&cfg.paths.ls_root).join("manifests/snapshots_v2.tsv");
    if local_manifest.exists() {
        let store = ManifestStore::new(&local_manifest);
        return store.read_records();
    }

    let cloud = match cfg.cloud.as_ref() {
        Some(cloud) => cloud,
        None => return Ok(Vec::new()),
    };

    let client = R2Client::new(R2Config {
        endpoint: cloud.endpoint.clone(),
        bucket: cloud.bucket.clone(),
        access_key: cloud.access_key.clone(),
        secret_key: cloud.secret_key.clone(),
    })
    .await?;

    let tmp_path = std::env::temp_dir().join(format!(
        "dev-backup-manifest-{}.tsv",
        OffsetDateTime::now_utc().unix_timestamp()
    ));
    client
        .download_object(
            "manifests/snapshots_v2.tsv",
            tmp_path.to_str().unwrap_or_default(),
        )
        .await?;

    let store = ManifestStore::new(&tmp_path);
    store.read_records()
}

fn sort_records_by_ts(records: &[ManifestRecord]) -> Result<Vec<ManifestRecord>> {
    let mut parsed = Vec::with_capacity(records.len());
    for record in records {
        let ts = OffsetDateTime::parse(&record.ts, &Rfc3339)
            .with_context(|| format!("invalid timestamp: {}", record.ts))?;
        parsed.push((ts, record.clone()));
    }
    parsed.sort_by_key(|(ts, _)| *ts);
    Ok(parsed.into_iter().map(|(_, record)| record).collect())
}

fn latest_label_from_records(records: &[ManifestRecord]) -> Result<String> {
    resolve_latest_label(records)?
        .ok_or_else(|| anyhow!("no label found in manifest"))
}

fn find_latest_local_snapshot_label(
    snapshots_root: &str,
    exclude_label: &str,
) -> Result<Option<String>> {
    let mut candidates = Vec::new();
    if !Path::new(snapshots_root).exists() {
        return Ok(None);
    }
    for entry in fs::read_dir(snapshots_root)
        .with_context(|| format!("failed to read snapshot root: {snapshots_root}"))?
    {
        let entry = entry?;
        let name = entry.file_name();
        let name = match name.to_str() {
            Some(value) => value,
            None => continue,
        };
        if let Some(label) = name.strip_prefix("dev@") {
            if label == exclude_label {
                continue;
            }
            if is_valid_label(label) {
                candidates.push(label.to_string());
            }
        }
    }
    candidates.sort();
    Ok(candidates.pop())
}

fn update_worktree_from_snapshot(cfg: &Config, snapshot_path: &str, label: &str) -> Result<()> {
    let worktree = Path::new(&cfg.paths.dataset);
    if worktree.exists() {
        if btrfs::subvolume_exists(worktree.to_str().unwrap_or_default())? {
            btrfs::subvolume_delete(worktree.to_str().unwrap_or_default())?;
        } else {
            let backup_name = format!(
                "{}_backup_{}",
                cfg.paths.dataset,
                OffsetDateTime::now_utc().unix_timestamp()
            );
            fs::rename(worktree, &backup_name)
                .with_context(|| format!("failed to move existing worktree to {backup_name}"))?;
        }
    }
    btrfs::snapshot_writable(snapshot_path, worktree.to_str().unwrap_or_default())?;
    println!("Working tree updated to dev@{label}");
    Ok(())
}

fn run_send_pipeline(
    snapshot: &str,
    parent: Option<&str>,
    output_path: &str,
    public_key: &str,
) -> Result<()> {
    let mut send_cmd = Command::new("btrfs");
    if let Some(parent_path) = parent {
        send_cmd.args(["send", "-p", parent_path, snapshot]);
    } else {
        send_cmd.args(["send", snapshot]);
    }
    let mut send_child = send_cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .context("failed to start btrfs send")?;

    let send_stdout = send_child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("failed to capture btrfs send stdout"))?;

    let mut zstd_child = Command::new("zstd")
        .args(["-3"])
        .stdin(Stdio::from(send_stdout))
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .context("failed to start zstd")?;

    let zstd_stdout = zstd_child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("failed to capture zstd stdout"))?;

    let mut age_child = Command::new("age")
        .args(["-R", public_key, "-o", output_path])
        .stdin(Stdio::from(zstd_stdout))
        .stderr(Stdio::inherit())
        .spawn()
        .context("failed to start age")?;

    let age_status = age_child.wait().context("failed to wait on age")?;
    let zstd_status = zstd_child.wait().context("failed to wait on zstd")?;
    let send_status = send_child.wait().context("failed to wait on btrfs send")?;

    if !send_status.success() {
        return Err(anyhow!("btrfs send failed"));
    }
    if !zstd_status.success() {
        return Err(anyhow!("zstd failed"));
    }
    if !age_status.success() {
        return Err(anyhow!("age failed"));
    }

    Ok(())
}

fn run_receive_pipeline(input_path: &str, snapshot_dir: &str, private_key: &str) -> Result<()> {
    let mut age_child = Command::new("age")
        .args(["-d", "-i", private_key, input_path])
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .context("failed to start age decrypt")?;

    let age_stdout = age_child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("failed to capture age stdout"))?;

    let mut zstd_child = Command::new("zstd")
        .args(["-d"])
        .stdin(Stdio::from(age_stdout))
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .context("failed to start zstd")?;

    let zstd_stdout = zstd_child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("failed to capture zstd stdout"))?;

    let mut recv_child = Command::new("btrfs")
        .args(["receive", snapshot_dir])
        .stdin(Stdio::from(zstd_stdout))
        .stderr(Stdio::inherit())
        .spawn()
        .context("failed to start btrfs receive")?;

    let recv_status = recv_child.wait().context("failed to wait on btrfs receive")?;
    let zstd_status = zstd_child.wait().context("failed to wait on zstd")?;
    let age_status = age_child.wait().context("failed to wait on age")?;

    if !age_status.success() {
        return Err(anyhow!("age decrypt failed"));
    }
    if !zstd_status.success() {
        return Err(anyhow!("zstd decode failed"));
    }
    if !recv_status.success() {
        return Err(anyhow!("btrfs receive failed"));
    }

    Ok(())
}
