use anyhow::{bail, Result};
use config::GlobalConfig;
use fs::{Algorithm, ColisionPolicy, MoveRequest};
use slide::Slide;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use syncjob::SyncJob;
use volume::Volume;

pub mod config;
mod fs;
mod slide;
mod syncjob;
mod volume;

const DEFAULT_SLIDE_CONFIG_FILE: &str = ".slide.yml";

/// Library entry point
///
/// This function will read the configuration files, identify the volumes and slides, and execute the sync jobs.
pub async fn slide(config: GlobalConfig) -> Result<()> {
    let mut volumes = HashMap::new();

    for rootset_config in config.roots {
        let some_volumes = process_config(&rootset_config.keyword, &rootset_config.roots);
        if some_volumes.is_err() {
            println!("Error processing some volumes");
        }
    }

    println!("Volumes for all configs: {volumes:#?}");
    println!("----");

    let syncjobs = build_syncjobs(&mut volumes)?;

    println!("Sync jobs: {syncjobs:#?}");

    execute_syncjobs(&volumes, &syncjobs).await

    /*
     * TODO: Execute the tidy-up function
     * 1. Read a .slide.yml file in foo/slides/foo folder
     * 2. That file should have this structure
     *  - rules:
     *    - rule:
     *      - regex: "^Media"
     *      - operation: Move
     *      - destination: "Media/Inbox" # Relative to volume root (mkdir -p if not existing)
     *    - rule:
     *      - regex: "^Photos/Mobile"
     *      - operation: Move_to_new_dir
     *      - params:
     *        - 0: "%Y%M%D"
     *      - destination: "Media/Photos"
     */
}

pub fn process_config(keyword: &str, roots: &[PathBuf]) -> Result<HashMap<String, Volume>> {
    let mut volumes: HashMap<String, Volume> = HashMap::new();

    // Identify the volumes in each root
    for root in roots {
        match identify_volumes(root, keyword) {
            Ok(v) => volumes.extend(v),
            Err(e) => println!("{e}"),
        }
    }
    println!("Found volumes: {volumes:?}");

    // Identify the slides of each volume
    for (_, volume) in volumes.iter_mut() {
        match identify_slides(volume) {
            Ok(_) => {}
            Err(e) => {
                println!("{e}");
            }
        }
    }
    println!("Completed volumes: {volumes:#?}");

    Ok(volumes)
}

/// Identify volumes inside a each root folder.
/// A volume is a folder that contains a slides subfolder (or the chosen keyword).
/// This subfolder contains the folders whose names will have to match the name of other volumes.
fn identify_volumes(root: &Path, keyword: &str) -> Result<HashMap<String, Volume>> {
    let mut volumes = HashMap::new();

    let root_str = root.to_string_lossy();

    // Implies .exists()
    if !root.is_dir() {
        bail!("{root_str} is not a folder");
    }

    let entries = root.read_dir();
    if entries.is_err() {
        bail!("{root_str} cannot be read");
    }

    println!("{root_str}");

    // Analyze the contents of the root folder
    for entry in entries?.flatten() {
        let entry_path = entry.path();
        let file_type = entry.file_type();
        if let Ok(file_type) = file_type {
            if file_type.is_dir() {
                let slides_path = entry_path.join(keyword);
                if slides_path.exists() {
                    let name = entry_path
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .to_string();
                    volumes.insert(name.clone(), Volume::new(name, keyword, entry_path));
                }
            }
        }
    }

    Ok(volumes)
}

fn identify_slides(volume: &mut Volume) -> Result<()> {
    let subfolders = volume.path.join(&volume.keyword).read_dir();

    if subfolders.is_err() {
        bail!("Unable to read the folder: {volume:?}");
    }

    for entry in subfolders?.flatten() {
        let entry_metadata = entry.metadata();
        if entry_metadata.is_err() {
            continue;
        }
        if entry_metadata.unwrap().is_dir() {
            let entry_fullpath = entry.path();
            let entry_name = entry_fullpath
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string();

            // Try to fetch the slide configuration if any
            let slide_conf = {
                let slide_conf =
                    config::read_slide_config(entry_fullpath.join(DEFAULT_SLIDE_CONFIG_FILE));
                match slide_conf {
                    Ok(s) => s.route,
                    Err(_) => None,
                }
            };

            volume.add_slide(Slide::new(entry_name, entry_fullpath, slide_conf));
        }
    }

    Ok(())
}

fn build_syncjobs(volumes: &mut HashMap<String, Volume>) -> Result<Vec<SyncJob>> {
    let mut syncjobs = Vec::new();

    for src_name in volumes.keys() {
        for (dst_name, slide) in &volumes[src_name].slides {
            if src_name == dst_name {
                continue;
            }

            // If the destination volume is available, its a direct slide
            if volumes.contains_key(dst_name) {
                syncjobs.push(SyncJob {
                    src: src_name.to_owned(),
                    dst: dst_name.to_owned(),
                    issue: dst_name.to_owned(),
                });
                continue;
            }

            match &slide.or_else {
                // If the slide has a default route, and the default route is available, its a indirect slide
                Some(default_route) => {
                    if volumes.contains_key(default_route) {
                        syncjobs.push(SyncJob {
                            src: src_name.to_owned(),
                            dst: default_route.to_owned(),
                            issue: dst_name.to_owned(),
                        });
                        continue;
                    }
                    println!("default_route {default_route} not available");
                }
                _ => {
                    println!("{dst_name} not available and no default route");
                }
            }
        }
    }

    // Create the slides that are missing in the destination volumes
    for syncjob in &syncjobs {
        if !volumes[&syncjob.dst].slides.contains_key(&syncjob.issue) {
            volumes
                .get_mut(&syncjob.dst)
                .unwrap()
                .create_slide(&syncjob.issue)?;
        }
    }

    Ok(syncjobs)
}

//TODO: Implement the sync function
//TODO: Implement the forward function. Figure out how to sort the syncs to minimize the number of operations.
//TODO: Implement the tidy-up function. In each volume we should process the slide of the volume to move the contents to other places inside the volume

async fn execute_syncjobs(volumes: &HashMap<String, Volume>, syncjobs: &[SyncJob]) -> Result<()> {
    let mut handles = Vec::new();

    // TODO: Measure the next block
    {
        for syncjob in syncjobs {
            println!(
                "Syncing {:?} -{:?}-> {:?}",
                syncjob.src, syncjob.issue, syncjob.dst
            );
            let src = volumes[&syncjob.src].slides[&syncjob.issue].path.clone();
            let dst = volumes[&syncjob.dst].slides[&syncjob.issue].path.clone();

            let handle = tokio::spawn(async move {
                if let Err(e) = sync_slide(&src, &dst).await {
                    bail!("Error syncing {:?} -> {:?}: {:?}", src, dst, e);
                }
                Ok(())
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.await??;
        }
    }

    Ok(())
}

async fn sync_slide(src: &PathBuf, dst: &PathBuf) -> Result<()> {
    println!("Syncing slide {:?} -> {:?}", src, dst);

    //TODO: Move this to cli parameters
    let request = MoveRequest {
        colision: ColisionPolicy::Fail,
        safe: false,
        checked: Some(Algorithm::CRC32),
        retries: 5,
    };

    let entries = src.read_dir();
    if entries.is_err() {
        bail!("{src:?} cannot be read");
    }

    for entry in entries?.flatten() {
        let entry_path = entry.path();
        let file_type = entry.file_type();
        if let Ok(file_type) = file_type {
            if file_type.is_dir() {
                let dst = dst.join(entry.file_name());
                // // Create the slide if it does not exist
                // if !dst.exists() {
                //     std::fs::create_dir_all(&dst)?;
                // }
                fs::sync(&entry_path, &dst, &request).await?;
                continue;
            }
            println!("Warning: {} is not a directory", entry_path.display());
        }
    }

    Ok(())
}


#[cfg(test)]
mod tests;