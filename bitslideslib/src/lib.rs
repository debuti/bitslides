use anyhow::{bail, Result};
use fs::MoveStrategy;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
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
mod token;

pub use config::{Algorithm, CollisionPolicy, GlobalConfig, RootsetConfig};
pub use token::Token;

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
        let some_volumes = rootset_config.identify_env();
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
                    log::info!(
                        "\"{dst_name}\" and default route \"{def_route_name}\" not available"
                    );
                }
                _ => {
                    log::info!("\"{dst_name}\" not available and no default route");
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
    let mut watcher_db = Vec::new();
    for syncjob in syncjobs.iter_mut() {
        let path = volumes[&syncjob.src].slides[&syncjob.dst]
            .path
            .canonicalize()?;
        let trigger = if let Some(trigger) = syncjob.take_trigger() {
            trigger
        } else {
            bail!("No trigger found for sync job {:?}", syncjob);
        };
        watcher_db.push((path, trigger));
    }

    let mut watcher = {
        let tracer = tracer.annotate_author("Watcher".to_string());
        tracer.async_log("Init", "Starting slides sync...").await?;

        notify::recommended_watcher(
            move |res: std::result::Result<notify::Event, notify::Error>| {
                if let Ok(event) = res {
                    match event.kind {
                        EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_) => {
                            let _ = tracer
                                .sync_log("Event", &format!("Filesystem event: {:?} ", event));
                            for (path, trigger) in &watcher_db {
                                // Check if any event path is within the watched directory
                                for event_path in &event.paths {
                                    let event_path = event_path.canonicalize();
                                    if let Ok(event_path) = event_path {
                                        let _deleteme = tracer.sync_log(
                                            "Event",
                                            &format!("launching {}", event_path.display()),
                                        );
                                        // FIXME: Maybe this doesnt work
                                        if event_path.starts_with(path) {
                                            if trigger.capacity() > 0 {
                                                let _deleteme =
                                                    tracer.sync_log("Event", "launched");
                                                let _ = trigger.blocking_send(());
                                            }
                                            // Otherwise skip this event, its ok
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            },
        )?
    };

    // TODO: Measure the next block
    {
        let mut handles = Vec::new();

        for mut syncjob in syncjobs.into_iter() {
            log::debug!("Syncing {:?}", syncjob);
            let src = volumes[&syncjob.src].slides[&syncjob.dst].path.clone();
            let dst = volumes[&syncjob.via].slides[&syncjob.dst].path.clone();
            let mut trace = tracer.annotate_author(format!("{:?}", syncjob));
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
                    if syncjob.borrow_receiver().recv().await.is_none() {
                        return Ok(());
                    }
                }
            });
            handles.push(handle);
        }

        Ok((watcher, handles))
    }

    // The anonymous tracer will be dropped here
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
