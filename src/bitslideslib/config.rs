use anyhow::Result;
use serde::Deserialize;
use std::path::{Path, PathBuf};


pub struct GlobalConfig
{
    pub roots: Vec<RootsetConfig>,
    pub dry_run: bool,
}
pub struct RootsetConfig
{
    pub keyword: String,
    pub roots: Vec<PathBuf>,
}

#[derive(Deserialize)]
pub struct SlideConfig {
    pub route: Option<String>,
}

pub fn read_slide_config<P>(file_path: P) -> Result<SlideConfig>
where
    P: AsRef<Path>,
{
    let file_content = std::fs::read_to_string(file_path)?;
    let config = serde_yaml::from_str(&file_content)?;
    Ok(config)
}
