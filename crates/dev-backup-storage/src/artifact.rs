use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtifactType {
    Anchor,
    Incremental,
}

#[derive(Debug, Clone)]
pub struct ArtifactInfo {
    pub label: String,
    pub artifact_type: ArtifactType,
    pub parent: Option<String>,
    pub filename: String,
}

pub fn parse_artifact_filename(filename: &str) -> Option<ArtifactInfo> {
    if let Some(label) = filename.strip_prefix("dev@").and_then(|name| name.strip_suffix(".full.send.zst.age")) {
        return Some(ArtifactInfo {
            label: label.to_string(),
            artifact_type: ArtifactType::Anchor,
            parent: None,
            filename: filename.to_string(),
        });
    }

    let trimmed = filename.strip_prefix("dev@")?;
    let trimmed = trimmed.strip_suffix(".send.zst.age")?;
    let mut parts = trimmed.split(".incr.from_");
    let label = parts.next()?;
    let parent = parts.next()?;
    if parts.next().is_some() {
        return None;
    }

    Some(ArtifactInfo {
        label: label.to_string(),
        artifact_type: ArtifactType::Incremental,
        parent: Some(parent.to_string()),
        filename: filename.to_string(),
    })
}

pub fn sha256_file(path: &str) -> Result<String> {
    let mut file = File::open(path).with_context(|| format!("failed to open artifact: {path}"))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let read = file.read(&mut buf)?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}
