#![warn(clippy::correctness)]
#![warn(clippy::suspicious)]
#![warn(clippy::complexity)]
#![warn(clippy::perf)]
#![warn(clippy::style)]
#![warn(clippy::pedantic)]
// #![warn(clippy::cargo)]
#![warn(clippy::unwrap_used)]
#![warn(clippy::expect_used)]
#![warn(clippy::indexing_slicing)]
#![warn(clippy::panic)]
#![warn(clippy::todo)]
#![warn(clippy::unimplemented)]

use anyhow::Result;
use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::path::PathBuf;
use syncjob::{SyncJobMeta, SyncJob, SyncJobs};
use tokio::{
    select,
    sync::{mpsc, oneshot},
};
use volume::{Volume, Volumes};

use tracer::Tracer;

mod config;
mod core;
mod fs;
mod named_collection;
mod rootset;
mod slide;
mod syncjob;
mod token;
mod tracer;
mod volume;

pub use config::{Algorithm, CollisionPolicy, GlobalConfig};
pub use rootset::Rootset;
pub use token::Token;

/// Watcher task creation
///
/// A watcher takes either a cancellation signal to turn the system off or a
/// command, and it emits events.
///
async fn watcher(
    tracer: &Tracer,
    event_tx: mpsc::Sender<Event>,
    mut command_rx: mpsc::Receiver<String>,
    mut cancellation_rx: oneshot::Receiver<()>,
) -> Result<()> {
    // Synchronous watcher (only one instance for all watched paths)
    let mut watcher = {
        let tracer = tracer.annotate_author("SyncWatcher".to_string());
        let _ = tracer.async_log("Init", "Starting slides sync...").await;

        notify::recommended_watcher(
            move |res: std::result::Result<notify::Event, notify::Error>| {
                if let Ok(event) = res {
                    match event.kind {
                        // FIXME: Should we listen to Remove events? (See https://github.com/debuti/bitslides/pull/5#discussion_r3299838685)
                        EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_) => {
                            let _ =
                                tracer.sync_log("Event", &format!("Filesystem event: {event:?} "));

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

    // Asynchronous watcher (wraps the sync watcher)
    {
        let tracer = tracer.annotate_author("AsyncWatcher".to_string());

        // FIXME: Retrieve the handle to control its lifetime and ensure it is properly shutdown (see https://github.com/debuti/bitslides/pull/5#discussion_r3299838669)
        tokio::spawn(
            // FIXME: Move this async task to a separate function and file if it grows more
            async move {
                loop {
                    select! {
                        // Command queue
                        command = command_rx.recv() => {
                           let _ = tracer.async_log("Command", &format!("Working on {command:?}...")).await;
                           if let Some(command) = command {
                               if command.starts_with("watchr ") {
                                   #[allow(clippy::expect_used)]
                                   let path_str = command.strip_prefix("watchr ").expect("Invalid command format");
                                   let path = PathBuf::from(path_str);
                                   if let Err(e) = watcher.watch(&path, RecursiveMode::Recursive) {
                                       let _ = tracer.async_log("Error", &format!("Failed to watch {}: {:?}", path.display(), e)).await;
                                   } else {
                                       let _ = tracer.async_log("Command", &format!("Watching {}...", path.display())).await;
                                   }
                               }
                               else {
                                   let _ = tracer.async_log("Command", &format!("Unknown command: {command:?}")).await;
                               }
                           } else {
                                break;
                           }
                        }

                        // Cancellation signal
                        _ = &mut cancellation_rx => {
                            let _= tracer.async_log("Shutdown", "Shutdown signal received, performing cleanup...").await;
                            break;
                        }
                    }
                }
            },
        );
    }

    Ok(())
}

/// Monitor all the rootsets, volumes and slides.
///
/// This function will take the input `config`, identify the volumes and slides,
/// and execute the sync jobs. Returns a Result indicating success (with a `Token`) or failure.
///
/// # Errors
///
/// This function errors out if any of the steps to set up the slides fails.
///
/// # Panics
///
/// This function will panic on developer errors, such as invalid command formats.
///
pub async fn slide(config: GlobalConfig) -> Result<Token> {
    log::debug!("Config: {config:#?}");

    let tracer = {
        let mut sinks = vec![tracer::Sink::StdOut];
        sinks.extend(config.trace.iter().map(tracer::Sink::File));
        Tracer::new(&sinks).await?
    };

    // Main channels
    let (cancellation_tx, cancellation_rx) = oneshot::channel::<()>();
    let (event_tx, event_rx) = mpsc::channel::<Event>(config::EVENT_CHANNEL_CAPACITY);
    // FIXME: Create an enum instead of raw Strings
    let (command_tx, command_rx) = mpsc::channel::<String>(config::COMMAND_CHANNEL_CAPACITY);

    // Async watcher
    watcher(&tracer, event_tx, command_rx, cancellation_rx).await?;

    // Core creation
    core::core(&tracer, event_rx, command_tx, config);

    Ok(Token::new(cancellation_tx))
}

// /// Tidy up the volumes.
// ///
// /// This function traverses the slides of each volume and applies the rules defined in the .slide.yml file.
// ///
// pub async fn tidy_up() {
//     unimplemented!();
//     /*
//      * TODO: Execute the tidy-up function
//      * 1. Read a .slide.yml file in foo/slides/foo folder
//      * 2. That file should have this structure
//      *  - rules:
//      *    - rule:
//      *      - regex: "^Media"
//      *      - operation: Move
//      *      - destination: "Media/Inbox" # Relative to volume root (mkdir -p if not existing)
//      *    - rule:
//      *      - regex: "^Photos/Mobile"
//      *      - operation: Move_to_new_dir
//      *      - params:
//      *        - 0: "%Y%M%D"
//      *      - destination: "Media/Photos"
//      */
// }

#[cfg(test)]
mod tests;
