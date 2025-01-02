use crate::bitslideslib::config;

use super::slide::Slide;
use anyhow::Result;
use std::{collections::HashMap, path::PathBuf};

const DEFAULT_VOLUME_CONFIG_FILE: &str = ".volume.yml";

#[derive(Debug)]
pub struct Volume {
    /// Name of the volume
    pub name: String,
    /// Keyword used for the slides subfolder
    pub keyword: String,
    /// Path to the volume root. Ex. /path/to/volumes/foo
    pub path: PathBuf,
    /// Slides that are part of the volume. Including the volume mailbox
    pub slides: HashMap<String, Slide>,
}

impl Volume {
    pub fn new(name: String, keyword: &str, path: PathBuf) -> Self {
        Self {
            name,
            keyword: keyword.to_owned(),
            path,
            slides: HashMap::new(),
        }
    }

    pub fn from_path(maybe_volume: PathBuf, keyword: &str) -> Option<Self> {
        let slides_path = maybe_volume.join(keyword);
        if slides_path.exists() {
            match maybe_volume.file_name() {
                Some(name) => {
                    let name = name.to_string_lossy().to_string();
                    return Some(Self::new(name, keyword, maybe_volume.to_owned()));
                }
                None => {
                    let volume_conf =
                        config::read_volume_config(slides_path.join(DEFAULT_VOLUME_CONFIG_FILE));
                    if let Ok(v) = volume_conf {
                        if let Some(n) = v.name {
                            return Some(Self::new(n, keyword, maybe_volume.to_owned()));
                        }
                    }

                    log::warn!(
                        "A volume has been identified at {maybe_volume:?} but it is nameless"
                    );
                }
            };
        }
        None
    }

    pub fn add_slide(&mut self, slide: Slide) {
        self.slides.insert(slide.name.clone(), slide);
    }

    pub fn create_slide(&mut self, name: &str) -> Result<()> {
        let path = self.path.join(&self.keyword).join(name);
        std::fs::create_dir_all(&path)?;
        self.slides
            .insert(name.to_owned(), Slide::new(name.to_owned(), path, None));
        Ok(())
    }
}

#[cfg(any())]
impl std::fmt::Display for Volume {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)?;
        for slide in self.slides.values() {
            write!(f, "\n  - {}", slide)?;
        }
        Ok(())
    }
}
