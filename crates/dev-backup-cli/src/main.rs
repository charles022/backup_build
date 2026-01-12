use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use dev_backup_btrfs as btrfs;
use dev_backup_core::config::Config;
use dev_backup_core::manifest::{ManifestRecord, ManifestStore};
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
    command: Command,
}

#[derive(Subcommand)]
enum Command {
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
}

#[derive(Subcommand)]
enum LsCommand {
    Send { label: String, parent: Option<String> },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Init { target } => init(&cli.config, target),
        Command::Snapshot { label } => snapshot(&cli.config, &label),
        Command::Artifact { action } => artifact(&cli.config, action),
        Command::Restore { action } => restore(&cli.config, action),
        Command::Sync { action } => sync(&cli.config, action).await,
        Command::Ws { action } => ws(action),
        Command::Ls { action } => ls(action),
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
            println!("LS initialized at {}", base.display());
            println!("TODO: generate age keypair in {}/keys", base.display());
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

fn ws(action: WsCommand) -> Result<()> {
    match action {
        WsCommand::RunMonth { .. } => Err(anyhow!("ws run-month not implemented")),
    }
}

fn ls(action: LsCommand) -> Result<()> {
    match action {
        LsCommand::Send { .. } => Err(anyhow!("ls send not implemented")),
    }
}

fn ensure_label(label: &str) -> Result<()> {
    let mut parts = label.split('-');
    let year = parts
        .next()
        .ok_or_else(|| anyhow!("label must be YYYY-MM"))?;
    let month = parts
        .next()
        .ok_or_else(|| anyhow!("label must be YYYY-MM"))?;
    if parts.next().is_some() || year.len() != 4 || month.len() != 2 {
        return Err(anyhow!("label must be YYYY-MM"));
    }
    if !year.chars().all(|c| c.is_ascii_digit()) || !month.chars().all(|c| c.is_ascii_digit()) {
        return Err(anyhow!("label must be YYYY-MM"));
    }
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
