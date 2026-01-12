use anyhow::{anyhow, Result};

pub fn encrypt_to_age(_public_key: &str, _input_path: &str, _output_path: &str) -> Result<()> {
    Err(anyhow!("encrypt_to_age not implemented"))
}

pub fn decrypt_from_age(_private_key_path: &str, _input_path: &str, _output_path: &str) -> Result<()> {
    Err(anyhow!("decrypt_from_age not implemented"))
}
