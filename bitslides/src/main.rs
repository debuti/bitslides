use anyhow::{bail, Result};
use bitslideslib::{enough, slide, Algorithm, CollisionPolicy, GlobalConfig, RootsetConfig};
use chrono::prelude::*;
use config::DEFAULT_KEYWORD;
use std::path::PathBuf;

#[cfg(not(test))]
use anyhow::anyhow;
#[cfg(not(test))]
use log::LevelFilter;
#[cfg(not(test))]
use simplelog::{ColorChoice, Config, TermLogger, TerminalMode};

mod cli;
mod config;

/// Generates the trace path from the given format.
///
fn generate_trace_path(trace_fmt: &str) -> Option<PathBuf> {
    let now: DateTime<Local> = Local::now();
    let trace = PathBuf::from(
        trace_fmt
            .replace("%Y", &format!("{:04}", &now.year()))
            .replace("%m", &format!("{:02}", &now.month()))
            .replace("%d", &format!("{:02}", &now.day()))
            .replace("%H", &format!("{:02}", &now.hour()))
            .replace("%M", &format!("{:02}", &now.minute()))
            .replace("%S", &format!("{:02}", &now.second())),
    );
    let trace = std::path::absolute(trace).ok()?;

    if !trace.exists() {
        let trace_parent = trace.parent().unwrap();
        if !trace_parent.exists() {
            log::error!("{trace:?}: Neither trace or parent folder does exist");
            return None;
        }
    }

    Some(trace)
}

/// Processes all configuration files and returns a list of `RootsetConfig` instances.
///
fn process_all_configs(
    config_paths: Vec<&PathBuf>,
) -> Result<(Vec<RootsetConfig>, Option<PathBuf>)> {
    let mut success = false;
    let mut rootsets = Vec::new();
    let mut trace = None;

    for config_path in config_paths {
        log::info!("Loading configuration from: {config_path:?}...");

        if config_path.exists() {
            match config::Config::new(config_path) {
                Ok(config) => {
                    let keyword = config.keyword.unwrap_or(DEFAULT_KEYWORD.to_owned());
                    let roots = config
                        .roots
                        .into_iter()
                        .map(|x| {
                            if x.contains("$") {
                                unimplemented!("Environment variables not supported yet");
                            }
                            let x = PathBuf::from(x);
                            if x.is_absolute() {
                                x
                            } else {
                                PathBuf::from(config_path.parent().unwrap()).join(x)
                            }
                        })
                        .collect::<Vec<PathBuf>>();

                    rootsets.push(RootsetConfig { keyword, roots });

                    // Yeah, only the trace of the last config file that defines it will prevail
                    if let Some(trace_fmt) = config.trace {
                        trace = generate_trace_path(&trace_fmt);
                    }
                }
                Err(e) => {
                    log::error!("{config_path:?}: Invalid config format: {e}");
                    continue;
                }
            };
        } else {
            log::error!("{config_path:?}: Config not found");
            continue;
        }

        success = true;
    }

    if !success {
        bail!("No valid configuration file found");
    }

    Ok((rootsets, trace))
}

/// Main function with arguments.
///
/// This function gathers information and calls the bitslideslib fn.
///
async fn main_w_args(
    args: &[String],
    shutdown_signal: tokio::sync::oneshot::Receiver<()>,
) -> Result<()> {
    let matches = cli::cli().get_matches_from(args);

    // Get the configuration files
    let config_files = matches
        .get_many::<PathBuf>("config")
        .expect("No configuration file found among provided/defaults.");

    let dry_run = matches.get_flag("dry-run");
    let non_safe = matches.get_flag("non-safe");
    let retries = matches.get_one::<u8>("retries").unwrap();

    // Initialize the logging framework if not already done
    #[cfg(not(test))]
    {
        let verbosity = *matches.get_one::<u8>("verbose").unwrap_or(&0);
        // Initialize the logging framework
        TermLogger::init(
            match verbosity {
                0 => LevelFilter::Error,
                1 => LevelFilter::Warn,
                2 => LevelFilter::Info,
                3 => LevelFilter::Debug,
                _ => LevelFilter::Trace,
            },
            Config::default(),
            TerminalMode::Stderr,
            ColorChoice::Auto,
        )
        .map_err(|_| anyhow!("Unable to initialize log"))?;

        if dry_run && verbosity < 2 {
            bail!("Dry-run mode is enabled, but the verbosity level is too low to see the output");
        }
    }

    let (rootsets, trace) = process_all_configs(config_files.into_iter().collect())?;

    let keep_alive = slide(GlobalConfig {
        rootsets,
        dry_run,
        trace,
        // FIXME: This should be configurable
        check: Some(Algorithm::BLAKE),
        // FIXME: This should be configurable
        collision: CollisionPolicy::Fail,
        safe: !non_safe,
        retries: *retries,
    })
    .await?;

    // Wait for shutdown signal (either from Ctrl+C handler or test)
    shutdown_signal.await?;

    enough(keep_alive).await
}

/// Entry point of the application.
///
#[tokio::main]
async fn main() -> Result<()> {
    // Collect args
    let args = std::env::args().collect::<Vec<_>>();

    // Create a oneshot channel for shutdown signal
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

    // Install custom Ctrl+C handler (cross-platform, not using tokio's signal)
    // Wrap the sender in an Option so we can take it in the FnMut closure
    let shutdown_tx = std::sync::Mutex::new(Some(shutdown_tx));
    ctrlc::set_handler(move || {
        log::info!("Received Ctrl+C, shutting down...");
        if let Some(tx) = shutdown_tx.lock().unwrap().take() {
            let _ = tx.send(());
        }
    })
    .expect("Failed to set Ctrl+C handler");

    // Await on main with shutdown signal
    main_w_args(&args, shutdown_rx).await
}

#[cfg(test)]
mod tests;
