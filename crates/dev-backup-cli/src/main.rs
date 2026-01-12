use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use dev_backup_btrfs as btrfs;
use dev_backup_core::config::Config;
use dev_backup_core::manifest::{ManifestRecord, ManifestStore};
use dev_backup_storage::artifact::{parse_artifact_filename, sha256_file, ArtifactType};
use std::fs;
use std::path::{Path, PathBuf};

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

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Init { target } => init(&cli.config, target),
        Command::Snapshot { label } => snapshot(&cli.config, &label),
        Command::Artifact { action } => artifact(&cli.config, action),
        Command::Restore { action } => restore(action),
        Command::Sync { action } => sync(action),
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
        ArtifactCommand::Build { label: _, parent: _ } => {
            Err(anyhow!("artifact build not implemented"))
        }
        ArtifactCommand::Register { path } => register_artifact(&cfg, &path),
    }
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
        ts: time::OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339)?,
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

fn restore(action: RestoreCommand) -> Result<()> {
    match action {
        RestoreCommand::Plan { .. } => Err(anyhow!("restore plan not implemented")),
        RestoreCommand::Hydrate { .. } => Err(anyhow!("restore hydrate not implemented")),
        RestoreCommand::Apply { .. } => Err(anyhow!("restore apply not implemented")),
    }
}

fn sync(action: SyncCommand) -> Result<()> {
    match action {
        SyncCommand::Push => Err(anyhow!("sync push not implemented")),
        SyncCommand::Pull { .. } => Err(anyhow!("sync pull not implemented")),
    }
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
