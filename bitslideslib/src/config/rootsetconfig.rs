use anyhow::{bail, Result};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use crate::{config::SlideConfig, slide::Slide, volume::Volume};

/// Set of roots
///
/// This configuration is used to define a set of root paths that will contain volumes, along with the keyword each root will use.
///
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct RootsetConfig {
    /// Keyword to use for this rootset
    pub keyword: String,
    /// List of root absolute paths that will contain volumes (Ex. /media or /mnt)
    pub roots: Vec<PathBuf>,
}

impl RootsetConfig {
    /// Gather information about the environment.
    ///
    /// This function will identify the volumes and slides for each volume in the current system.
    ///
    pub fn identify_env(self) -> Result<HashMap<String, Volume>> {
        let mut volumes: HashMap<String, Volume> = HashMap::new();

        // Identify volumes
        {
            // Identify the volumes in each root
            for root in self.roots {
                match Self::identify_volumes(&root, &self.keyword) {
                    Ok(v) => volumes.extend(v),
                    Err(e) => log::warn!("{e}"),
                }
            }

            // Under Windows we may have volumes as drives (e. C:, D:, etc)
            // TODO: Add an option to opt-out of this analysis
            #[cfg(target_os = "windows")]
            {
                // Retrieve the drives using the windows api
                let drives = {
                    let mut result = Vec::new();
                    const MAX_BUF: usize = 1024;
                    let mut buf = [0u8; MAX_BUF];
                    let length = unsafe {
                        windows::Win32::Storage::FileSystem::GetLogicalDriveStringsA(Some(&mut buf))
                    } as usize;
                    if length > MAX_BUF {
                        log::error!(
                            "The hardcoded buffer is not big enough to retrieve all logical drives"
                        );
                    }
                    let mut ptr = 0;
                    while ptr < length {
                        let drive = CStr::from_bytes_until_nul(&buf[ptr..]).unwrap();
                        let offset_to_next = 1 + drive.count_bytes();
                        ptr += offset_to_next;
                        result.push(PathBuf::from(drive.to_str().unwrap()));
                    }
                    result
                };

                for drive in drives {
                    if let Some(volume) = Volume::from_path(drive, keyword) {
                        volumes.insert(volume.name.clone(), volume);
                    }
                }
            }
        }

        // Identify the slides of each volume
        for (_, volume) in volumes.iter_mut() {
            match Self::identify_slides(volume) {
                Ok(_) => {}
                Err(e) => log::warn!("{e}"),
            }
        }

        Ok(volumes)
    }

    /// Identify volumes inside a each root folder.
    ///
    /// A volume is a folder that contains a slides subfolder (or the chosen keyword).
    /// This subfolder contains the folders whose names will have to match the name of other volumes.
    ///
    fn identify_volumes(root: &Path, keyword: &str) -> Result<HashMap<String, Volume>> {
        let mut volumes = HashMap::new();

        // Implies .exists()
        if !root.is_dir() {
            bail!("{} is not a folder", root.to_string_lossy());
        }

        let entries = root.read_dir();
        if entries.is_err() {
            bail!("{} cannot be read", root.to_string_lossy());
        }

        // Analyze the contents of the root folder
        for entry in entries?.flatten() {
            let file_type = entry.file_type();
            if let Ok(file_type) = file_type {
                if file_type.is_dir() {
                    if let Some(volume) = Volume::from_path(entry.path(), keyword) {
                        volumes.insert(volume.name.clone(), volume);
                    }
                }
            }
        }

        Ok(volumes)
    }

    /// Identify the slides inside a volume.
    ///
    /// Mutates the volume by adding the slides found in the slides subfolder.
    ///
    fn identify_slides(volume: &mut Volume) -> Result<()> {
        let subfolders = volume.path.join(&volume.keyword).read_dir();

        if subfolders.is_err() {
            bail!("Unable to read the folder: {volume:?}");
        }

        for entry in subfolders?.flatten() {
            if let Ok(entry_metadata) = entry.metadata() {
                if entry_metadata.is_dir() {
                    let slide_fullpath = entry.path();
                    let slide_name = slide_fullpath
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .to_string();

                    // Try to fetch the slide configuration if any
                    let slide_conf = {
                        let slide_conf =
                            SlideConfig::new(slide_fullpath.join(super::DEFAULT_SLIDE_CONFIG_FILE));
                        match slide_conf {
                            Ok(s) => s.route,
                            Err(_) => None,
                        }
                    };

                    volume.add_slide(Slide::new(slide_name, slide_fullpath, slide_conf));
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use super::RootsetConfig;
    use crate::tests::setup;
    use crate::Volume;
    use std::collections::HashMap;

    /// Test the identification of volumes inside a root folder
    #[test]
    fn test_identify_volumes() {
        // Prerequisite: Setup the test context
        let ctx = setup().unwrap();

        // Action: Call identify_volumes operation with the 1st root folder and the keyword "slides"
        let volumes = RootsetConfig::identify_volumes(&ctx.roots[0], "slides").unwrap();

        // Check: The result should contain 2 volumes
        assert_eq!(volumes.len(), 2);

        // Check: The result should contain the volumes "foo" and "bar"
        for volume in ["foo", "bar"] {
            assert!(volumes.contains_key(volume) && volumes[volume].path.exists());
        }
    }

    /// Test the identification of slides inside a volume
    #[test]
    fn test_identify_slides() {
        // Prerequisite: Setup the test context
        let ctx = setup().unwrap();

        // Prerequisite: Create a volume object for the "foo" volume
        let mut volume = Volume::new("foo".to_string(), true, "slides", ctx.roots[0].join("foo"));

        // Action: Call identify_slides operation with the volume object
        RootsetConfig::identify_slides(&mut volume).unwrap();

        // Check: The volume should contain 3 slides
        assert_eq!(volume.slides.len(), 3);

        // Check: The volume should contain the slides "foo", "bar" and "baz"
        for slide in ["foo", "bar", "baz"] {
            assert!(volume.slides.contains_key(slide) && volume.slides[slide].path.exists());
        }
    }

    /// Test the identification of volumes and slides inside a set of root folders
    #[test]
    fn test_identify_env() {
        // Prerequisite: Setup the test context
        let ctx = setup().unwrap();

        // Action: Call identify_env operation with the keyword "slides" and the root folders
        let volumes: HashMap<String, Volume> = {
            let rootsetconfig = RootsetConfig {
                keyword: "slides".into(),
                roots: ctx.roots,
            };
            rootsetconfig.identify_env().unwrap()
        };

        // Check: The result should contain 4 volumes
        assert_eq!(volumes.len(), 5);

        // Check: The result should contain the volumes "foo" and "bar"
        for volume in ["foo", "bar"] {
            assert!(volumes.contains_key(volume));

            // Check: The volume is enabled
            assert!(!volumes[volume].disabled);

            // Check: The volume should contain 3 slides
            assert_eq!(volumes[volume].slides.len(), 3);

            // Check: The volume should contain the slides "foo", "bar" and "baz"
            for slide in ["foo", "bar", "baz"] {
                assert!(volumes[volume].slides.contains_key(slide));
            }
        }
        assert!(volumes["foo"].slides["bar"].or_else.is_none());
        assert!(volumes["foo"].slides["baz"].or_else.is_none());

        // Check: The result should contain the volumes "baz" and "els"
        for volume in ["baz", "els"] {
            assert!(volumes.contains_key(volume));

            // Check: The volume is enabled
            assert!(!volumes[volume].disabled);

            // Check: The volume should contain 4 slides
            assert_eq!(volumes[volume].slides.len(), 5);

            // Check: The volume should contain the following slides
            for slide in ["foo", "bar", "baz", "qux_", "quux_"] {
                assert!(volumes[volume].slides.contains_key(slide));
            }
        }
        assert!(volumes["baz"].slides["qux_"].or_else.is_some());
        assert!(volumes["baz"].slides["foo"].or_else.is_some());

        // Check: The result should contain the volume "disabled" (per volume config name override)
        assert!(volumes.contains_key("disabled"));

        // Check: The volume "disabled" is disabled
        assert!(volumes["disabled"].disabled);

        // Check: The volume name is
        assert_eq!(volumes["disabled"].name, "disabled".to_owned());
    }
}
