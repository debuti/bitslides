use crate::config::GlobalConfig;
use crate::fs;
use crate::fs::MoveStrategy;
use crate::Volume;
use crate::{Rootset, Tracer, Volumes};
use crate::{SyncJob, SyncJobMeta, SyncJobs};
use anyhow::{anyhow, bail, Result};
use notify::{Event, EventKind};
use std::fs::canonicalize;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

/// Core task creation
///
/// The core receives events and, if needed, triggers commands. It also
/// spawns as many syncjob tasks as needed.
///
pub(crate) fn core(
    tracer: &Tracer,
    mut event_rx: mpsc::Receiver<Event>,
    command_tx: mpsc::Sender<String>,
    config: GlobalConfig,
) {
    let move_req = MoveStrategy {
        collision: config.collision,
        safe: config.safe,
        check: config.check,
        retries: config.retries,
    };

    let tracer = tracer.annotate_author("Core".to_string());

    // FIXME: Retrieve the handle to control its lifetime and ensure it is properly shutdown (see https://github.com/debuti/bitslides/pull/5#discussion_r3299838669)
    tokio::spawn(async move {
        let rootsets = config.rootsets;

        // Analyze current volumes
        let mut volumes = volume_discovery(&rootsets);

        // Now analyze the volumes to generate the sync jobs
        let syncjobs = build_syncjobs(&mut volumes)?;

        log::debug!("Initial syncjobs: {syncjobs:#?}");

        let mut syncjob_triggers: Vec<(PathBuf, mpsc::Sender<()>)> = vec![];

        for mut syncjob in syncjobs {
            log::debug!("Syncing {:?}", syncjob);
            let trace = tracer.annotate_author(format!("{syncjob:?}"));
            let move_req = move_req.clone();
            let Some(trigger) = syncjob.take_trigger() else {
                bail!("No trigger found for sync job {syncjob:?}");
            };
            syncjob_triggers.push((syncjob.src.clone(), trigger));

            command_tx
                .send(format!("watchr {}", syncjob.src.display()))
                .await?;

            // Spawn a new tokio async task for this syncjob
            // Drop the JoinHandle. The async task is now free to die when it finishes its job

            // Syncjob loop
            // FIXME: Retrieve the handle to control its lifetime and ensure it is properly shutdown (see https://github.com/debuti/bitslides/pull/5#discussion_r3299838669)
            tokio::spawn(async move {
                loop {
                    if let Err(e) = sync_slide(&syncjob, config.dry_run, &trace, &move_req).await {
                        bail!(
                            "Error syncing {:?} -> {:?}: {:?}",
                            syncjob.src,
                            syncjob.dst,
                            e
                        );
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
        'event_loop: while let Some(event) = event_rx.recv().await {
            let event_paths: Vec<PathBuf> = event.paths.iter().flat_map(canonicalize).collect();

            match event.kind {
                EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_) => {
                    // See if the path is a volume (New/Removed volumes, for example, if a usb stick is connected to the system)
                    for rootset in &rootsets {
                        // Every rootset contains a 1-N roots, which volumes will all share the same keyword (or otherwise wont be recognized as volumes)
                        for root in &rootset.roots {
                            for event_path in &event_paths {
                                // New/removed volume
                                if is_direct_child_folder(root, event_path) && event_path.is_dir() {
                                    let volume = Volume::from_path(event_path, &rootset.keyword);

                                    if let Some(volume) = volume {
                                        // Check if the volume is already identified
                                        if volumes.contains(&volume) {
                                            let _ = tracer
                                            .async_log(
                                                "Event",
                                                &format!(
                                                    "volume change identified {}, but already identified. Skipping",
                                                    event_path.display()
                                                ),
                                            )
                                            .await;

                                            // Skip this volume. Glitch?
                                            continue;
                                        }

                                        let _ = tracer
                                            .async_log(
                                                "Event",
                                                &format!(
                                                    "volume change identified {}",
                                                    event_path.display()
                                                ),
                                            )
                                            .await;

                                        // Add the new volume to the current volumes
                                        volumes.insert(volume);

                                        let new_syncjobs = build_syncjobs(&mut volumes)?;

                                        // TODO:
                                        // * Rerun volume discovery (on the new rootset) && syncjobs build (over new volumes)
                                        // * Add new syncjobs to database and trigger them
                                        // * Add a test for this event
                                    }
                                    continue;
                                }
                                // New/removed root
                                if root == event_path && event_path.is_dir() {}
                            }
                        }
                    }

                    // See if the path is a slide (New/Removed slides)
                    for volume in &volumes {
                        let slides_path = volume.slides_path();
                        for event_path in &event_paths {
                            if is_direct_child_folder(&slides_path, event_path)
                                && event_path.is_dir()
                            {
                                let _ = tracer
                                    .async_log(
                                        "Event",
                                        &format!(
                                            "slides change identified {}",
                                            event_path.display()
                                        ),
                                    )
                                    .await;
                                // TODO:
                                // * Rerun syncjobs build (over the new volume)
                                // * Add new syncjobs to db and trigger
                            }
                        }
                    }

                    // See if the path is a syncjob (New/Changed/Removed data)
                    for (path, trigger) in &syncjob_triggers {
                        // Check if any event path is within the watched directory
                        for event_path in &event_paths {
                            if event_path.starts_with(path) {
                                let _ = tracer
                                    .async_log(
                                        "Event",
                                        &format!("launching {}", event_path.display()),
                                    )
                                    .await;
                                if trigger.capacity() > 0 {
                                    let _ = trigger.send(()).await;
                                    continue 'event_loop;
                                }
                                let _ = tracer.async_log("Event", "skipped!").await;
                                continue 'event_loop;
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

fn is_direct_child_folder(root: &Path, event_path: &Path) -> bool {
    if let Ok(relative_path) = event_path.strip_prefix(root) {
        let mut components = relative_path.components();

        match (components.next(), components.next()) {
            // Exactly one component exists
            (Some(_), None) => true,
            _ => false,
        }
    } else {
        false
    }
}

/// Discover volumes in the rootsets
fn volume_discovery(rootsets: &[Rootset]) -> Volumes {
    let mut result = Volumes::new();

    // Analyze each rootset to extract volumes and slides
    for rootset in rootsets {
        let some_volumes = rootset.into_volumes();
        result.extend(some_volumes);
    }

    log::debug!("Volumes for all configs: {result:#?}");
    result
}

/// Compose the sync jobs from the volume information.
///
/// This function will create the sync jobs based on the identified slides.
///
fn build_syncjobs(volumes: &mut Volumes) -> Result<SyncJobs> {
    let mut syncjobmetas = Vec::new();

    for src_name in volumes.names() {
        // Skip disabled volumes
        if volumes[src_name].disabled {
            continue;
        }

        for slide in &volumes[src_name].slides {
            let dst_name = &slide.name;

            if src_name == dst_name {
                continue;
            }
            log::debug!("Evaluating routes from {src_name} to {dst_name}");

            // If the destination volume is available, its a direct slide
            if volumes.contains_name(dst_name) && !volumes[dst_name].disabled {
                syncjobmetas.push(SyncJobMeta::new(src_name, dst_name, dst_name));
                log::debug!(" + Added direct route from {src_name} to {dst_name}");
                continue;
            }

            match &slide.or_else {
                // If the slide has a default route, and the default route is available, its a indirect slide
                Some(def_route_name) => {
                    if volumes.contains_name(def_route_name) && !volumes[def_route_name].disabled {
                        syncjobmetas.push(SyncJobMeta::new(src_name, def_route_name, dst_name));
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
    for syncjobmeta in &syncjobmetas {
        if !volumes[&syncjobmeta.via]
            .slides
            .contains_name(&syncjobmeta.dst)
        {
            volumes
                .get_mut(&syncjobmeta.via)
                .ok_or_else(|| anyhow!("Volume not found"))?
                .create_slide(&syncjobmeta.dst)?;
        }
    }

    let mut syncjobs = SyncJobs::new();
    for syncjobmeta in syncjobmetas {
        syncjobs.insert(syncjobmeta.activate(volumes)?);
    }
    Ok(syncjobs)
}

/// Sync the contents of a slide.
///
async fn sync_slide(
    syncjob: &SyncJob,
    dry_run: bool,
    tracer: &Tracer,
    move_req: &MoveStrategy,
) -> Result<()> {
    log::info!("Syncing {:?}", syncjob);

    let entries = syncjob.src.read_dir();
    if entries.is_err() {
        bail!("{:?} cannot be read", syncjob.src);
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
            let dst = syncjob.dst.join(entry.file_name());
            fs::sync(&entry_path, &dst, dry_run, tracer, move_req).await?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {

    use super::*;
    pub(crate) use crate::tests::setup;
    use crate::Rootset;

    /// Test the building of sync jobs between volumes
    #[test]
    fn test_build_syncjobs() {
        // Prerequisite: Setup the test context
        let ctx = setup().unwrap();

        // Prerequisite: Identify the volumes in the root folders
        let mut volumes = {
            let rootset_config = Rootset {
                keyword: "slides".into(),
                roots: ctx.roots,
            };
            rootset_config.into_volumes()
        };

        // Action: Call build_syncjobs operation with the identified volumes
        let syncjobs = build_syncjobs(&mut volumes).unwrap();

        #[cfg(false)]
        {
            println!("Syncjobs:");
            for syncjob in &syncjobs {
                println!("  {:?}", syncjob);
            }
        }

        #[rustfmt::skip]
        let expected_syncjobs =[
            SyncJobMeta::new("foo", "bar", "bar").activate(&volumes).unwrap(),
            SyncJobMeta::new("foo", "baz", "baz").activate(&volumes).unwrap(),
            SyncJobMeta::new("bar", "foo", "foo").activate(&volumes).unwrap(),
            SyncJobMeta::new("bar", "baz", "baz").activate(&volumes).unwrap(),
            SyncJobMeta::new("baz", "foo", "foo").activate(&volumes).unwrap(),
            SyncJobMeta::new("baz", "bar", "bar").activate(&volumes).unwrap(),
            SyncJobMeta::new("els", "foo", "foo").activate(&volumes).unwrap(),
            SyncJobMeta::new("els", "bar", "bar").activate(&volumes).unwrap(),
            SyncJobMeta::new("els", "baz", "baz").activate(&volumes).unwrap(),
            // Indirect syncjobs
            SyncJobMeta::new("baz", "bar", "qux_").activate(&volumes).unwrap(),
        ];

        // Check: The result should match the length and content of the expected sync jobs
        assert_eq!(syncjobs.len(), expected_syncjobs.len());
        for expected_syncjob in expected_syncjobs {
            assert!(
                syncjobs.contains(&expected_syncjob),
                "Missing {:?}",
                expected_syncjob
            );
        }

        // Check: The sync jobs don't contain disabled volumes
        assert!(!syncjobs.contains(
            &SyncJobMeta::new("disabled", "foo", "foo")
                .activate(&volumes)
                .unwrap()
        ));
    }
}
