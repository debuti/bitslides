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
    config_files: Vec<&PathBuf>,
) -> Result<(Vec<RootsetConfig>, Option<PathBuf>)> {
    let mut success = false;
    let mut rootsets = Vec::new();
    let mut trace = None;

    for config_path in config_files {
        log::info!("Loading configuration from: {config_path:?}...");

        if config_path.exists() {
            match config::read_config(config_path.to_str().unwrap()) {
                Ok(config) => {
                    let keyword = config.keyword.unwrap_or(DEFAULT_KEYWORD.to_owned());
                    let roots = config
                        .roots
                        .into_iter()
                        .map(|x| {
                            if x.contains("$") {
                                unimplemented!("Environment variables not supported yet");
                            }
                            PathBuf::from(x)
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
        let verbose = *matches.get_one::<u8>("verbose").unwrap_or(&0);
        // Initialize the logging framework
        TermLogger::init(
            match verbose {
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

        if dry_run && verbose < 2 {
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
mod tests {
    use tempfile::tempdir;

    use crate::main_w_args;

    #[tokio::test]
    async fn test_main_dummy_environment() {
        let temp_dir = tempdir().unwrap();
        // Use forward slashes in Windows
        let temp_dir_str = temp_dir.path().to_str().unwrap().replace("\\", "/");
        let config_file = temp_dir.path().join("config.yml");
        let config_content = format!(
            r#"
# Example configuration file
keyword: "slides"
roots:
- "root0"
- "root1"
trace: "{}/bitslides.%Y%M%d%H%M%S.log"
"#,
            temp_dir_str
        );
        std::fs::write(&config_file, config_content).unwrap();

        let args = vec!["bitslides", "-c", config_file.to_str().unwrap()];

        // Create a channel and spawn a task to send shutdown signal after a delay
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            let _ = shutdown_tx.send(());
        });

        assert!(main_w_args(
            args.into_iter()
                .map(|x| x.to_owned())
                .collect::<Vec<String>>()
                .as_slice(),
            shutdown_rx,
        )
        .await
        .is_ok());
    }

    #[tokio::test]
    async fn test_main_corrupted_config() {
        let temp_dir = tempdir().unwrap();
        let config_file = temp_dir.path().join("config.yml");
        let config_content = r#"Memento mori"#;
        std::fs::write(&config_file, config_content).unwrap();

        let args = vec!["bitslides", "-c", config_file.to_str().unwrap()];

        // Create a channel and spawn a task to send shutdown signal after a delay
        // (though the test will error out before reaching the signal wait)
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            let _ = shutdown_tx.send(());
        });

        assert!(main_w_args(
            args.into_iter()
                .map(|x| x.to_owned())
                .collect::<Vec<String>>()
                .as_slice(),
            shutdown_rx,
        )
        .await
        .is_err());
    }

    #[tokio::test]
    async fn test_main_missing_config() {
        let args = vec!["bitslides", "-c", "not-to-be-found"];

        // Create a channel and spawn a task to send shutdown signal after a delay
        // (though the test will error out before reaching the signal wait)
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            let _ = shutdown_tx.send(());
        });

        assert!(main_w_args(
            args.into_iter()
                .map(|x| x.to_owned())
                .collect::<Vec<String>>()
                .as_slice(),
            shutdown_rx,
        )
        .await
        .is_err());
    }

    #[tokio::test]
    async fn test_main_wrong_trace_config() {
        let temp_dir = tempdir().unwrap();
        let temp_dir_str = temp_dir.path().to_str().unwrap().replace("\\", "/");
        let config_file = temp_dir.path().join("config.yml");
        let config_content = format!(
            r#"
# Example configuration file
keyword: "slides"
roots:
- "root0"
- "root1"
trace: "{}/non-existing-folder/bitslides.log"
"#,
            temp_dir_str
        );
        std::fs::write(&config_file, config_content).unwrap();

        let args = vec!["bitslides", "-c", config_file.to_str().unwrap()];

        // Create a channel and spawn a task to send shutdown signal after a delay
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            let _ = shutdown_tx.send(());
        });

        let result = main_w_args(
            args.into_iter()
                .map(|x| x.to_owned())
                .collect::<Vec<String>>()
                .as_slice(),
            shutdown_rx,
        )
        .await;

        assert!(result.is_ok(), "Failed with: {}", result.unwrap_err());
    }
}
