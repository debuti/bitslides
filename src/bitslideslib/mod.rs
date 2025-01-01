use anyhow::{bail, Result};
use chrono::prelude::*;
use config::GlobalConfig;
use fs::MoveRequest;
use slide::Slide;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use syncjob::{SyncJob, SyncJobs};
use tokio::{
    fs::OpenOptions,
    io::AsyncWriteExt,
    sync::mpsc::{self, Sender},
};
use volume::Volume;

pub mod config;
mod fs;
mod slide;
mod syncjob;
mod volume;

const DEFAULT_SLIDE_CONFIG_FILE: &str = ".slide.yml";

/// Execute all the slides.
///
/// This function will take the input `config`, identify the volumes and slides,
/// and execute the sync jobs. Returns a Result indicating success or failure.
///
pub async fn slide(config: GlobalConfig) -> Result<()> {
    log::debug!("Config: {config:#?}");

    let mut tracer = None;

    let mut volumes = HashMap::new();

    for rootset_config in config.rootsets {
        let some_volumes = identify_env(&rootset_config.keyword, &rootset_config.roots);
        match some_volumes {
            Ok(v) => volumes.extend(v),
            Err(_) => log::warn!("Error processing some volumes"),
        }
    }

    log::debug!("Volumes for all configs: {volumes:#?}");

    let syncjobs = build_syncjobs(&mut volumes)?;

    log::debug!("Sync jobs: {syncjobs:#?}");

    let trace = match config.trace {
        Some(trace) => {
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(trace)
                .await?;
            let (tx, mut rx) = mpsc::channel::<Option<String>>(32);
            // Handle the traces in a separate task
            tracer = Some(tokio::spawn(async move {
                while let Some(message) = rx.recv().await {
                    let message = match message {
                        Some(m) => {
                            format!("[{}] {}\n", Local::now().format("%Y-%m-%d %H:%M:%S"), m)
                        }
                        None => "".to_owned(),
                    };
                    // Best effort
                    let _ = file.write_all(message.as_bytes()).await;
                }
            }));
            Some(tx)
        }
        None => None,
    };

    let move_req = MoveRequest {
        collision: config.collision,
        safe: false,
        check: config.check,
        retries: 5,
    };

    let result = execute_syncjobs(&volumes, &syncjobs, config.dry_run, trace, &move_req).await;

    if let Some(tracer) = tracer {
        tracer.await?;
    }

    result
}

/// Tidy up the volumes.
///
/// This function traverses the slides of each volume and applies the rules defined in the .slide.yml file.
///
pub async fn tidy_up() {
    unimplemented!();
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

/// Gather information about the environment.
///
/// This function will identify the volumes and slides for each volume in the current system.
///
pub fn identify_env(keyword: &str, roots: &[PathBuf]) -> Result<HashMap<String, Volume>> {
    let mut volumes: HashMap<String, Volume> = HashMap::new();

    // Identify the volumes in each root
    for root in roots {
        match identify_volumes(root, keyword) {
            Ok(v) => volumes.extend(v),
            Err(e) => log::warn!("{e}"),
        }
    }

    // Identify the slides of each volume
    for (_, volume) in volumes.iter_mut() {
        match identify_slides(volume) {
            Ok(_) => {}
            Err(e) => log::warn!("{e}"),
        }
    }

    Ok(volumes)
}

/// Compose the sync jobs from the volume information.
///
/// This function will create the sync jobs based on the identified slides.
///
fn build_syncjobs(volumes: &mut HashMap<String, Volume>) -> Result<SyncJobs> {
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
                    log::info!("default_route {default_route} not available");
                }
                _ => {
                    log::info!("{dst_name} not available and no default route");
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

/// Execute the sync jobs.
///
/// This function will execute the sync jobs in parallel.
///
async fn execute_syncjobs(
    volumes: &HashMap<String, Volume>,
    syncjobs: &SyncJobs,
    dry_run: bool,
    tracer: Option<Sender<Option<String>>>,
    move_req: &MoveRequest,
) -> Result<()> {
    let mut handles = Vec::new();

    if let Some(tracer) = &tracer {
        tracer
            .send(Some("Starting slides sync...".to_owned()))
            .await?;
    }

    // TODO: Measure the next block
    {
        for syncjob in syncjobs {
            log::debug!("Syncing {:?}", syncjob);
            let syncjob = syncjob.clone();
            let src = volumes[&syncjob.src].slides[&syncjob.issue].path.clone();
            let dst = volumes[&syncjob.dst].slides[&syncjob.issue].path.clone();
            let trace = tracer.clone();
            let move_req = move_req.clone();
            let handle = tokio::spawn(async move {
                if let Err(e) = sync_slide(&syncjob, &src, &dst, dry_run, trace, &move_req).await {
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

    if let Some(tracer) = &tracer {
        tracer.send(None).await?;
    }

    Ok(())
}

/// Sync the contents of a slide.
///
async fn sync_slide(
    syncjob: &SyncJob,
    src: &PathBuf,
    dst: &Path,
    dry_run: bool,
    tracer: Option<Sender<Option<String>>>,
    move_req: &MoveRequest,
) -> Result<()> {
    log::info!("Syncing {:?}", syncjob);

    let tracer = tracer.map(|t| (t, syncjob));

    let entries = src.read_dir();
    if entries.is_err() {
        bail!("{src:?} cannot be read");
    }

    for entry in entries?.flatten() {
        let entry_path = entry.path();
        let file_type = entry.file_type();
        if let Ok(file_type) = file_type {
            // The slide should only contain directories or config files
            if !file_type.is_dir() {
                log::warn!("{} is not a directory", entry_path.display());
                continue;
            }
            let dst = dst.join(entry.file_name());
            fs::sync(&entry_path, &dst, dry_run, &tracer, move_req).await?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests;
