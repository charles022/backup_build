use anyhow::{anyhow, Context, Result};
use std::fs::File;
use std::path::Path;
use std::process::{Command, Stdio};

fn run_btrfs(args: &[&str]) -> Result<()> {
    let status = Command::new("btrfs")
        .args(args)
        .status()
        .with_context(|| format!("failed to run btrfs {args:?}"))?;
    if !status.success() {
        return Err(anyhow!("btrfs {args:?} failed"));
    }
    Ok(())
}

pub fn snapshot_readonly(source: &str, dest: &str) -> Result<()> {
    run_btrfs(&["subvolume", "snapshot", "-r", source, dest])
}

pub fn snapshot_writable(source: &str, dest: &str) -> Result<()> {
    run_btrfs(&["subvolume", "snapshot", source, dest])
}

pub fn subvolume_delete(path: &str) -> Result<()> {
    run_btrfs(&["subvolume", "delete", path])
}

pub fn send_full_to_file(snapshot: &str, output_path: &str) -> Result<()> {
    let output = File::create(output_path)
        .with_context(|| format!("failed to create output: {output_path}"))?;
    let status = Command::new("btrfs")
        .args(["send", snapshot])
        .stdout(Stdio::from(output))
        .status()
        .with_context(|| format!("failed to run btrfs send on {snapshot}"))?;
    if !status.success() {
        return Err(anyhow!("btrfs send failed for {snapshot}"));
    }
    Ok(())
}

pub fn send_incremental_to_file(parent: &str, snapshot: &str, output_path: &str) -> Result<()> {
    let output = File::create(output_path)
        .with_context(|| format!("failed to create output: {output_path}"))?;
    let status = Command::new("btrfs")
        .args(["send", "-p", parent, snapshot])
        .stdout(Stdio::from(output))
        .status()
        .with_context(|| format!("failed to run btrfs send -p {parent} {snapshot}"))?;
    if !status.success() {
        return Err(anyhow!("btrfs send -p failed for {snapshot}"));
    }
    Ok(())
}

pub fn receive_from_file(snapshot_dir: &str, input_path: &str) -> Result<()> {
    let input = File::open(input_path)
        .with_context(|| format!("failed to open input: {input_path}"))?;
    let status = Command::new("btrfs")
        .args(["receive", snapshot_dir])
        .stdin(Stdio::from(input))
        .status()
        .with_context(|| format!("failed to run btrfs receive into {snapshot_dir}"))?;
    if !status.success() {
        return Err(anyhow!("btrfs receive failed into {snapshot_dir}"));
    }
    Ok(())
}

pub fn subvolume_exists(path: &str) -> Result<bool> {
    let status = Command::new("btrfs")
        .args(["subvolume", "show", path])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    match status {
        Ok(s) => Ok(s.success()),
        Err(_) => Ok(false),
    }
}

pub fn is_btrfs_mount(path: &str) -> Result<bool> {
    let stat = std::fs::metadata(path)
        .with_context(|| format!("failed to stat {path}"))?;
    if !stat.is_dir() {
        return Ok(false);
    }
    let output = Command::new("stat")
        .args(["-f", "--format=%T", path])
        .output()
        .with_context(|| format!("failed to run stat on {path}"))?;
    if !output.status.success() {
        return Err(anyhow!("stat failed on {path}"));
    }
    let fs_type = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(fs_type == "btrfs")
}

pub fn ensure_dir(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)
        .with_context(|| format!("failed to create directory: {}", path.display()))
}
