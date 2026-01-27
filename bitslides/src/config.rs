use anyhow::Result;
use serde::Deserialize;
use std::path::Path;

pub const DEFAULT_KEYWORD: &str = "Slides";

/// Configuration file representation.
///
#[derive(Deserialize)]
pub struct Config {
    // Slides folder keyword
    pub keyword: Option<String>,
    // Root folders to watch
    pub roots: Vec<String>,
    // Optional trace file path format
    pub trace: Option<String>,
}

/// Reads a configuration file.
///
pub fn read_config<P>(file_path: P) -> Result<Config>
where
    P: AsRef<Path>,
{
    let file_content = std::fs::read_to_string(file_path)?;
    let config = serde_yaml::from_str(&file_content)?;
    Ok(config)
}
