use anyhow::Result;
use serde::Deserialize;
use std::path::Path;

#[derive(Deserialize)]
pub struct GlobalConfig {
    pub keyword: Option<String>,
    pub roots: Vec<String>,
}

pub fn read_global_config<P>(file_path: P) -> Result<GlobalConfig>
where
    P: AsRef<Path>,
{
    let file_content = std::fs::read_to_string(file_path)?;
    let config = serde_yaml::from_str(&file_content)?;
    Ok(config)
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
