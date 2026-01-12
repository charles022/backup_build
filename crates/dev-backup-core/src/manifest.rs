use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestRecord {
    pub ts: String,
    pub label: String,
    #[serde(rename = "type")]
    pub record_type: String,
    pub parent: String,
    pub bytes: u64,
    pub sha256: String,
    pub local_path: String,
    pub object_key: String,
}

pub struct ManifestStore {
    path: PathBuf,
}

impl ManifestStore {
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    pub fn ensure_initialized(&self) -> Result<()> {
        if self.path.exists() {
            return Ok(());
        }
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create manifest directory: {}", parent.display()))?;
        }
        let mut writer = csv::WriterBuilder::new()
            .delimiter(b'\t')
            .from_path(&self.path)
            .with_context(|| format!("failed to create manifest: {}", self.path.display()))?;
        writer
            .write_record([
                "ts",
                "label",
                "type",
                "parent",
                "bytes",
                "sha256",
                "local_path",
                "object_key",
            ])
            .context("failed to write manifest header")?;
        writer.flush().context("failed to flush manifest header")?;
        Ok(())
    }

    pub fn read_records(&self) -> Result<Vec<ManifestRecord>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(b'\t')
            .from_path(&self.path)
            .with_context(|| format!("failed to read manifest: {}", self.path.display()))?;
        let mut records = Vec::new();
        for result in reader.deserialize() {
            let record: ManifestRecord = result.context("failed to parse manifest row")?;
            records.push(record);
        }
        Ok(records)
    }

    pub fn append_record(&self, record: &ManifestRecord) -> Result<()> {
        let file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(&self.path)
            .with_context(|| format!("failed to open manifest: {}", self.path.display()))?;
        let mut writer = csv::WriterBuilder::new().delimiter(b'\t').from_writer(file);
        writer.serialize(record).context("failed to append manifest record")?;
        writer.flush().context("failed to flush manifest")?;
        Ok(())
    }
}
