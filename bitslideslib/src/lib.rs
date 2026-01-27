use anyhow::{bail, Result};
use fs::MoveStrategy;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use slide::Slide;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use syncjob::{SyncJob, SyncJobs};
use volume::Volume;

#[cfg(target_os = "windows")]
use std::ffi::CStr;

use tracer::Tracer;

pub mod config;
mod fs;
mod slide;
mod syncjob;
mod tracer;
mod volume;

pub use config::{Algorithm, CollisionPolicy, GlobalConfig, RootsetConfig};

const DEFAULT_SLIDE_CONFIG_FILE: &str = ".slide.yml";

#[allow(dead_code)]
pub struct Token {
    /// Watcher OS task handle. Dropped first to force the syncjob tasks to end.
    watcher: RecommendedWatcher,
    handles: Vec<tokio::task::JoinHandle<Result<()>>>,
    tracer: Option<tokio::task::JoinHandle<()>>,
}

impl Token {
    fn new(
        watcher: RecommendedWatcher,
        handles: Vec<tokio::task::JoinHandle<Result<()>>>,
        tracer: Option<tokio::task::JoinHandle<()>>,
    ) -> Self {
        Self {
            watcher,
            handles,
            tracer,
        }
    }
}

pub async fn enough(token: Token) -> Result<()> {
    // TODO: Ideally this should be happening in the Drop impl for Token. But that wont let us control the results of the awaited tasks.

    let watcher = token.watcher;
    let handles = token.handles;
    let tracer = token.tracer;

    // Drop the watcher first, so that the mpsc channels can be closed
    // and the syncjob tasks can finish
    drop(watcher);

    // Await all the handles. When every syncjob task finishes, its
    // tracer mpsc channel will be closed
    for handle in handles {
        let _ = handle.await?;
    }

    // Await the tracer if any
    if let Some(tracer) = tracer {
        tracer.await?;
    }

    Ok(())
}

/// Monitor all the slides.
///
/// This function will take the input `config`, identify the volumes and slides,
/// and execute the sync jobs. Returns a Result indicating success or failure.
///
pub async fn slide(config: GlobalConfig) -> Result<Token> {
    log::debug!("Config: {config:#?}");

    // Maybe a tracer task handle
    let (trace, tracer) = Tracer::new(&config.trace.as_ref()).await?;

    let mut volumes = HashMap::new();

    // Analyze each rootset to extract volumes and slides
    for rootset_config in config.rootsets {
        let some_volumes = identify_env(&rootset_config.keyword, &rootset_config.roots);
        match some_volumes {
            Ok(v) => volumes.extend(v),
            Err(_) => log::warn!("Error processing some volumes"),
        }
    }

    log::debug!("Volumes for all configs: {volumes:#?}");

    // Now analyze the volumes to generate the sync jobs
    let syncjobs = build_syncjobs(&mut volumes)?;

    log::debug!("Sync jobs: {syncjobs:#?}");

    let move_req = MoveStrategy {
        collision: config.collision,
        safe: false,
        check: config.check,
        retries: 5,
    };

    let (watcher, handles) =
        execute_syncjobs(&volumes, syncjobs, config.dry_run, trace, &move_req).await?;

    Ok(Token::new(watcher, handles, tracer))
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

    // Identify volumes
    {
        // Identify the volumes in each root
        for root in roots {
            match identify_volumes(root, keyword) {
                Ok(v) => volumes.extend(v),
                Err(e) => log::warn!("{e}"),
            }
        }

        // Under Windows we may have volumes as drives (e. C:, D:, etc)
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
        // Skip disabled volumes
        if volumes[src_name].disabled {
            continue;
        }

        for (dst_name, slide) in &volumes[src_name].slides {
            if src_name == dst_name {
                continue;
            }
            log::debug!("Evaluating routes from {src_name} to {dst_name}");

            // If the destination volume is available, its a direct slide
            if volumes.contains_key(dst_name) && !volumes[dst_name].disabled {
                syncjobs.push(SyncJob::new(src_name, dst_name, dst_name));
                log::debug!(" + Added direct route from {src_name} to {dst_name}");
                continue;
            }

            match &slide.or_else {
                // If the slide has a default route, and the default route is available, its a indirect slide
                Some(def_route_name) => {
                    if volumes.contains_key(def_route_name) && !volumes[def_route_name].disabled {
                        syncjobs.push(SyncJob::new(src_name, def_route_name, dst_name));
                        log::debug!(" + Added indirect route from {src_name} to {dst_name} via {def_route_name}");
                        continue;
                    }
                    log::info!("default_route {def_route_name} not available");
                }
                _ => {
                    log::info!("{dst_name} not available and no default route");
                }
            }
        }
    }

    // Create the slides that are missing in the destination volumes
    for syncjob in &syncjobs {
        if !volumes[&syncjob.via].slides.contains_key(&syncjob.dst) {
            volumes
                .get_mut(&syncjob.via)
                .unwrap()
                .create_slide(&syncjob.dst)?;
        }
    }

    Ok(syncjobs)
}

/// Execute the sync jobs.
///
/// This function will execute the sync jobs, ideally, in parallel.
///
async fn execute_syncjobs(
    volumes: &HashMap<String, Volume>,
    mut syncjobs: SyncJobs,
    dry_run: bool,
    tracer: Tracer,
    move_req: &MoveStrategy,
) -> Result<(RecommendedWatcher, Vec<tokio::task::JoinHandle<Result<()>>>)> {
    tracer.log("Init", "Starting slides sync...").await?;

    let watcher_db = syncjobs
        .iter_mut()
        .map(|syncjob| {
            let path = volumes[&syncjob.src].slides[&syncjob.dst].path.clone();
            let trigger = syncjob.take_trigger().expect("No trigger found");
            (path, trigger)
        })
        .collect::<Vec<_>>();

    let mut watcher = notify::recommended_watcher(
        move |res: std::result::Result<notify::Event, notify::Error>| {
            if let Ok(event) = res {
                match event.kind {
                    EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_) => {
                        for (path, trigger) in &watcher_db {
                            // Check if any event path is within the watched directory
                            for event_path in &event.paths {
                                if event_path.starts_with(path) {
                                    let _ = trigger.blocking_send(());
                                    break;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        },
    )?;

    // TODO: Measure the next block
    {
        let mut handles = Vec::new();

        for mut syncjob in syncjobs.into_iter() {
            log::debug!("Syncing {:?}", syncjob);
            let src = volumes[&syncjob.src].slides[&syncjob.dst].path.clone();
            let dst = volumes[&syncjob.via].slides[&syncjob.dst].path.clone();
            let mut trace = tracer.annotate_syncjob(&syncjob);
            let move_req = move_req.clone();

            watcher.watch(&src, RecursiveMode::Recursive)?;

            // Spawn a new tokio async task for this syncjob
            let handle = tokio::spawn(async move {
                loop {
                    if let Err(e) =
                        sync_slide(&syncjob, &src, &dst, dry_run, &mut trace, &move_req).await
                    {
                        bail!("Error syncing {:?} -> {:?}: {:?}", src, dst, e);
                    }
                    // None is received when the mpsc::Sender is dropped
                    if syncjob.inner.rx.recv().await.is_none() {
                        return Ok(());
                    }
                }
            });
            handles.push(handle);
        }

        Ok((watcher, handles))
    }
}

/// Sync the contents of a slide.
///
async fn sync_slide(
    syncjob: &SyncJob,
    src: &PathBuf,
    dst: &Path,
    dry_run: bool,
    tracer: &mut Tracer,
    move_req: &MoveStrategy,
) -> Result<()> {
    log::info!("Syncing {:?}", syncjob);

    let entries = src.read_dir();
    if entries.is_err() {
        bail!("{src:?} cannot be read");
    }

    // Sync every folder inside the slide
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
            fs::sync(&entry_path, &dst, dry_run, tracer, move_req).await?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests;
