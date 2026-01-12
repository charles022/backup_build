use anyhow::{anyhow, Context, Result};
use std::process::Command;

pub fn encrypt_to_age(public_key: &str, input_path: &str, output_path: &str) -> Result<()> {
    let status = Command::new("age")
        .args(["-R", public_key, "-o", output_path, input_path])
        .status()
        .with_context(|| format!("failed to run age on {input_path}"))?;
    if !status.success() {
        return Err(anyhow!("age encryption failed for {input_path}"));
    }
    Ok(())
}

pub fn decrypt_from_age(private_key_path: &str, input_path: &str, output_path: &str) -> Result<()> {
    let status = Command::new("age")
        .args(["-d", "-i", private_key_path, "-o", output_path, input_path])
        .status()
        .with_context(|| format!("failed to run age on {input_path}"))?;
    if !status.success() {
        return Err(anyhow!("age decryption failed for {input_path}"));
    }
    Ok(())
}
