use anyhow::Result;
use serde::Deserialize;
use std::path::{Path, PathBuf};

pub use checksums::Algorithm;

/// Set of roots
/// 
/// This configuration is used to define a set of root paths that will contain volumes, along with the keyword each root will use.
/// 
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct RootsetConfig {
    /// Keyword to use for this rootset
    pub keyword: String,
    /// List of root paths that will contain volumes
    pub roots: Vec<PathBuf>,
}

/// Policy to apply in case of a file collision
/// 
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

/// Global configuration
/// 
/// This configuration is used to define the global settings of the library.
/// 
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

/// Volume configuration
/// 
/// This configuration is used to define the settings of a volume.
/// 
#[derive(Deserialize, Debug)]
pub struct VolumeConfig {
    /// Optional name of the volume. If provided it will take precedence over the OS context.
    pub name: Option<String>,
}

/// Read a volume configuration file
/// 
pub fn read_volume_config<P>(file_path: P) -> Result<VolumeConfig>
where
    P: AsRef<Path>,
{
    let file_content = std::fs::read_to_string(file_path)?;
    let config = serde_yaml::from_str(&file_content)?;
    Ok(config)
}

/// Slide configuration
/// 
/// This configuration is used to define the settings of a slide.
/// 
#[derive(Deserialize, Debug)]
pub struct SlideConfig {
    /// Default route for the slide.
    pub route: Option<String>,
}

/// Read a slide configuration file
/// 
pub fn read_slide_config<P>(file_path: P) -> Result<SlideConfig>
where
    P: AsRef<Path>,
{
    let file_content = std::fs::read_to_string(file_path)?;
    let config = serde_yaml::from_str(&file_content)?;
    Ok(config)
}
