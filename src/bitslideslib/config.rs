use anyhow::Result;
use serde::Deserialize;
use std::path::{Path, PathBuf};

pub use checksums::Algorithm;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct RootsetConfig {
    /// Keyword to use for this rootset
    pub keyword: String,
    /// List of root paths that will contain volumes
    pub roots: Vec<PathBuf>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum CollisionPolicy {
    /// Overwrite the destination file
    Overwrite,
    /// Skip the file
    Skip,
    /// Rename the file
    Rename { suffix: String },
    /// Fail the operation
    Fail,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct GlobalConfig {
    /// List of rootset configurations
    pub rootsets: Vec<RootsetConfig>,
    /// If true, do not perform any filesystem operation
    pub dry_run: bool,
    /// If provided, the path to a file where to write the trace
    pub trace: Option<PathBuf>,
    /// If provided, the algorithm to use for checksumming
    pub check: Option<Algorithm>,
    /// What to do in case of a file collision
    pub collision: CollisionPolicy,
    /// If true, enable a secure algorithm for moving files
    pub safe: bool,
    /// Number of retries in case of a failure (checksum mismatch, etc)
    pub retries: u8,
}

#[derive(Deserialize, Debug)]
pub struct VolumeConfig {
    pub name: Option<String>,
}

pub fn read_volume_config<P>(file_path: P) -> Result<VolumeConfig>
where
    P: AsRef<Path>,
{
    let file_content = std::fs::read_to_string(file_path)?;
    let config = serde_yaml::from_str(&file_content)?;
    Ok(config)
}

#[derive(Deserialize, Debug)]
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
