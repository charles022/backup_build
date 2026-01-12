use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub paths: Paths,
    pub cloud: Option<Cloud>,
    pub crypto: Option<Crypto>,
    pub remote: Option<Remote>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Paths {
    pub dataset: String,
    pub snapshots: String,
    pub ls_root: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Cloud {
    pub endpoint: String,
    pub bucket: String,
    pub access_key: String,
    pub secret_key: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Crypto {
    pub age_public_key: Option<String>,
    pub age_private_key_path: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Remote {
    pub ls_host: Option<String>,
    pub ls_user: Option<String>,
}

impl Config {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("failed to read config: {}", path.as_ref().display()))?;
        let cfg = toml::from_str(&contents)
            .with_context(|| format!("failed to parse config: {}", path.as_ref().display()))?;
        Ok(cfg)
    }
}
