use serde::Deserialize;
use std::path::Path;
use anyhow::Result;


/// Volume configuration
///
/// This configuration is used to define the settings of a volume.
///
#[derive(Deserialize, Debug)]
pub struct VolumeConfig {
    /// Optional name of the volume. If provided it will take precedence over the OS context.
    pub name: Option<String>,
    /// Optional enable status of the volume. Disabled volumes will be identified but not processed.
    pub disabled: Option<bool>,
}

impl VolumeConfig {
    /// Read a volume configuration file
    ///
    pub fn new<P>(file_path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let file_content = std::fs::read_to_string(file_path)?;
        let config = serde_yaml::from_str(&file_content)?;
        Ok(config)
    }
}
