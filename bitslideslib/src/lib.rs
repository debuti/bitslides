#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::unwrap_used)]
#![warn(clippy::expect_used)]
#![warn(clippy::indexing_slicing)]
#![warn(clippy::panic)]
#![warn(clippy::todo)]
#![warn(clippy::unimplemented)]

use anyhow::{anyhow, bail, Result};
use fs::MoveStrategy;
use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use syncjob::{SyncJob, SyncJobs};
use tokio::{
    select,
    sync::{mpsc, oneshot},
};
use volume::Volume;

#[cfg(target_os = "windows")]
use std::ffi::CStr;

use tracer::Tracer;

mod config;
mod fs;
mod rootset;
mod slide;
mod syncjob;
mod token;
mod tracer;
mod volume;

pub use config::{Algorithm, CollisionPolicy, GlobalConfig};
pub use rootset::Rootset;
pub use token::Token;

/// Monitor all the slides.
///
/// This function will take the input `config`, identify the volumes and slides,
/// and execute the sync jobs. Returns a Result indicating success or failure.
///
pub async fn slide(config: GlobalConfig) -> Result<Token> {
    log::debug!("Config: {config:#?}");

    let tracer = {
        let mut sinks = vec![tracer::Sink::StdOut];
        sinks.extend(config.trace.iter().map(tracer::Sink::File));
        Tracer::new(&sinks).await?
    };

    // Core mpsc channels
    let (cancellation_tx, mut cancellation_rx) = oneshot::channel::<()>();
    let (event_tx, mut event_rx) = mpsc::channel::<Event>(config::EVENT_CHANNEL_CAPACITY);
    let (command_tx, mut command_rx) = mpsc::channel::<String>(config::COMMAND_CHANNEL_CAPACITY);

    // Async watcher
    {
        // Watcher (only one instance for all watched pathss)
        let mut watcher = {
            let tracer = tracer.annotate_author("SyncWatcher".to_string());
            let _ = tracer.sync_log("Init", "Starting slides sync...");

            notify::recommended_watcher(
                move |res: std::result::Result<notify::Event, notify::Error>| {
                    if let Ok(event) = res {
                        match event.kind {
                            EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_) => {
                                let _ = tracer
                                    .sync_log("Event", &format!("Filesystem event: {event:?} "));

                                if event_tx.capacity() > 0 {
                                    let _ = tracer.sync_log("Event", "launched");
                                    // Blocking send because we are doing this from sync code
                                    let _ = event_tx.blocking_send(event);
                                } else {
                                    let _ = tracer.sync_log("Event", "ignored");
                                }
                            }
                            _ => {}
                        }
                    }
                },
            )?
        };
        {
            let tracer = tracer.annotate_author("AsyncWatcher".to_string());
            tokio::spawn(
                // FIXME: Move this async task to a separate function and file if it grows more
                async move {
                    loop {
                        select! {
                            command = command_rx.recv() => {
                               let _ = tracer.async_log("Command", &format!("Working on {command:?}...")).await;
                               if let Some(command) = command {
                                   if command.starts_with("watchr ") {
                                       let path_str = command.strip_prefix("watchr ").unwrap();
                                       let path = PathBuf::from(path_str);
                                       if let Err(e) = watcher.watch(&path, RecursiveMode::Recursive) {
                                           let _ = tracer.async_log("Error", &format!("Failed to watch {path:?}: {e:?}")).await;
                                       } else {
                                           let _ = tracer.async_log("Command", &format!("Watching {path:?}...")).await;
                                       }
                                   }
                                   else {
                                       let _ = tracer.async_log("Command", &format!("Unknown command: {command:?}")).await;
                                   }
                               } else {
                                    break;
                               }
                            }
                            _ = &mut cancellation_rx => {
                                let _= tracer.async_log("Shutdown", "Shutdown signal received, performing cleanup...").await;
                                break;
                            }
                        }
                    }
                },
            );
        }
    }

    // Core creation
    {
        let tracer = tracer.annotate_author("Core".to_string());

        tokio::spawn(async move {
            // Core initialization
            // FIXME: Delete this async_log
            tracer.async_log("Init", "I am alive").await?;

            let mut volumes = {
                let mut result = HashMap::new();

                // Analyze each rootset to extract volumes and slides
                for rootset_config in config.rootsets {
                    let some_volumes = rootset_config.into_volumes();
                    let Ok(v) = some_volumes else {
                        log::warn!("Error processing some volumes");
                        continue;
                    };
                    result.extend(v);
                }

                log::debug!("Volumes for all configs: {result:#?}");
                result
            };

            // Now analyze the volumes to generate the sync jobs
            let mut syncjobs = build_syncjobs(&mut volumes)?;

            log::debug!("Sync jobs: {syncjobs:#?}");

            let move_req = MoveStrategy {
                collision: config.collision,
                safe: false,
                check: config.check,
                retries: 5,
            };

            let watcher_db = {
                let mut watcher_db = Vec::new();
                for syncjob in &mut syncjobs {
                    let path = volumes[&syncjob.src].slides[&syncjob.dst]
                        .path
                        .canonicalize()?;

                    let Some(trigger) = syncjob.take_trigger() else {
                        bail!("No trigger found for sync job {syncjob:?}");
                    };

                    watcher_db.push((path, trigger));
                }
                watcher_db
            };

            for mut syncjob in syncjobs {
                log::debug!("Syncing {:?}", syncjob);
                let src = volumes[&syncjob.src].slides[&syncjob.dst].path.clone();
                let dst = volumes[&syncjob.via].slides[&syncjob.dst].path.clone();
                let trace = tracer.annotate_author(format!("{syncjob:?}"));
                let move_req = move_req.clone();

                command_tx.send(format!("watchr {}", src.display())).await?;

                // Spawn a new tokio async task for this syncjob
                // Drop the JoinHandle. The async task is now free to die when it finishes its job
                tokio::spawn(async move {
                    loop {
                        if let Err(e) =
                            sync_slide(&syncjob, &src, &dst, config.dry_run, &trace, &move_req)
                                .await
                        {
                            bail!("Error syncing {:?} -> {:?}: {:?}", src, dst, e);
                        }
                        // FIXME: Maybe use tokio::sync::notify here instead!
                        // None is received when the mpsc::Sender is dropped
                        if syncjob.borrow_receiver().recv().await.is_none() {
                            return Ok(());
                        }
                    }
                });
            }

            // Core main loop
            while let Some(event) = event_rx.recv().await {
                match event.kind {
                    EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_) => {
                        //TODO: See if the path is rootsets
                        //TODO: See if the path is slides
                        //TODO: See if the path is syncjobs

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
                                            let _deleteme = tracer.sync_log("Event", "launched");
                                            // Blocking send because we are doing this from sync code
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

            Ok::<(), anyhow::Error>(())
        });
    }

    Ok(Token::new(cancellation_tx))
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
                .ok_or_else(|| anyhow!("Volume not found"))?
                .create_slide(&syncjob.dst)?;
        }
    }

    Ok(syncjobs)
}

/// Sync the contents of a slide.
///
async fn sync_slide(
    syncjob: &SyncJob,
    src: &PathBuf,
    dst: &Path,
    dry_run: bool,
    tracer: &Tracer,
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
