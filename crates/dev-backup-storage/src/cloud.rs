use anyhow::{anyhow, Result};

#[derive(Debug, Clone)]
pub struct R2Config {
    pub endpoint: String,
    pub bucket: String,
    pub access_key: String,
    pub secret_key: String,
}

#[derive(Debug)]
pub struct R2Client {
    _config: R2Config,
}

impl R2Client {
    pub fn new(config: R2Config) -> Self {
        Self { _config: config }
    }

    pub fn upload_object(&self, _key: &str, _path: &str) -> Result<()> {
        Err(anyhow!("upload_object not implemented"))
    }

    pub fn download_object(&self, _key: &str, _path: &str) -> Result<()> {
        Err(anyhow!("download_object not implemented"))
    }
}
