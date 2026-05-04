use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Serialize, Deserialize, Clone)]
pub struct Source {
    pub uri: String,
    pub added_at: u64,
    pub last_synced: u64,
    pub no_ignore: bool,
    pub max_filesize: Option<u64>,
}

pub fn load(path: &Path) -> Result<Vec<Source>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let data = std::fs::read_to_string(path).context("failed to read sources")?;
    serde_json::from_str(&data).context("failed to parse sources")
}

pub fn save(path: &Path, sources: &[Source]) -> Result<()> {
    let data = serde_json::to_string_pretty(sources)?;
    std::fs::write(path, data).context("failed to write sources")
}
