use anyhow::{Context, Result};
use aws_credential_types::Credentials;
use aws_sdk_s3::config::Region;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client;
use std::path::Path;
use tokio::io::AsyncWriteExt;

#[derive(Debug, Clone)]
pub struct R2Config {
    pub endpoint: String,
    pub bucket: String,
    pub access_key: String,
    pub secret_key: String,
}

#[derive(Debug, Clone)]
pub struct R2Client {
    client: Client,
    bucket: String,
}

impl R2Client {
    pub async fn new(config: R2Config) -> Result<Self> {
        let creds = Credentials::new(
            config.access_key,
            config.secret_key,
            None,
            None,
            "dev-backup",
        );
        let shared = aws_credential_types::provider::SharedCredentialsProvider::new(creds);
        let sdk_config = aws_config::from_env()
            .region(Region::new("auto"))
            .endpoint_url(config.endpoint)
            .credentials_provider(shared)
            .load()
            .await;
        let s3_config = aws_sdk_s3::config::Builder::from(&sdk_config)
            .force_path_style(true)
            .build();
        let client = Client::from_conf(s3_config);
        Ok(Self {
            client,
            bucket: config.bucket,
        })
    }

    pub async fn upload_object(&self, key: &str, path: &str) -> Result<()> {
        let body = ByteStream::from_path(Path::new(path))
            .await
            .with_context(|| format!("failed to read file for upload: {path}"))?;
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(body)
            .send()
            .await
            .with_context(|| format!("failed to upload {key}"))?;
        Ok(())
    }

    pub async fn download_object(&self, key: &str, path: &str) -> Result<()> {
        let output = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .with_context(|| format!("failed to download {key}"))?;

        let mut file = tokio::fs::File::create(path)
            .await
            .with_context(|| format!("failed to create download file: {path}"))?;
        let mut body = output.body.into_async_read();
        tokio::io::copy(&mut body, &mut file)
            .await
            .with_context(|| format!("failed to write downloaded file: {path}"))?;
        file.flush()
            .await
            .with_context(|| format!("failed to flush downloaded file: {path}"))?;
        Ok(())
    }
}
