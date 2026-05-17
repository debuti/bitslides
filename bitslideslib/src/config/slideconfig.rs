use anyhow::Result;
use std::path::Path;
use serde::Deserialize;

/// Slide configuration
///
/// This configuration is used to define the settings of a slide.
///
#[derive(Deserialize, Debug)]
pub struct SlideConfig {
    /// Default route for the slide.
    pub route: Option<String>,
}

impl SlideConfig {
    /// Read a slide configuration file
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
