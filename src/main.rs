use anyhow::{anyhow, bail, Result};
use bitslides::{slide, Algorithm, CollisionPolicy, GlobalConfig, RootsetConfig};
use config::DEFAULT_KEYWORD;
use log::LevelFilter;
use simplelog::{ColorChoice, Config, TermLogger, TerminalMode};
use std::path::PathBuf;

mod cli;
mod config;

/// Processes all configuration files and returns a list of `RootsetConfig` instances.
///
fn process_all_configs(config_files: Vec<&PathBuf>) -> Result<Vec<RootsetConfig>> {
    let mut success = false;
    let mut result = Vec::new();

    for config_path in config_files {
        log::info!("Loading configuration from: {config_path:?}...");

        if config_path.exists() {
            if let Ok(config) = config::read_config(config_path.to_str().unwrap()) {
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

                result.push(RootsetConfig { keyword, roots });
            } else {
                log::error!("{config_path:?}: Invalid config format");
                continue;
            }
        } else {
            log::error!("{config_path:?}: Config not found");
            continue;
        };

        success = true;
    }

    if !success {
        bail!("No valid configuration file found");
    }

    Ok(result)
}

async fn main_w_args(args: &[String]) -> Result<()> {
    let matches = cli::cli().get_matches_from(args);

    // Get the configuration files
    let config_files = matches
        .get_many::<PathBuf>("config")
        .expect("No configuration file found among provided/defaults.");

    let dry_run = matches.get_flag("dry-run");
    let non_safe = matches.get_flag("non-safe");
    let retries = matches.get_one::<u8>("retries").unwrap();

    // Initialize the logging framework
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

    let rootsets = process_all_configs(config_files.into_iter().collect())?;

    slide(GlobalConfig {
        rootsets,
        dry_run,
        // FIXME: This should be configurable
        check: Some(Algorithm::BLAKE),
        // FIXME: This should be configurables
        collision: CollisionPolicy::Fail,
        safe: !non_safe,
        retries: *retries,
    })
    .await
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    main_w_args(&args).await
}

#[cfg(test)]
mod tests {

    use tempfile::tempdir;

    use crate::main_w_args;

    #[tokio::test]
    async fn test_main_dummy_environment() {
        let temp_dir = tempdir().unwrap();
        let config_file = temp_dir.path().join("config.yml");
        let config_content = r#"
# Example configuration file
keyword: "slides"
roots:
- "volume1"
- "volume2""#;
        std::fs::write(&config_file, config_content).unwrap();

        let args = vec!["bitslides", "-c", config_file.to_str().unwrap()];

        assert!(main_w_args(
            args.into_iter()
                .map(|x| x.to_owned())
                .collect::<Vec<String>>()
                .as_slice(),
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

        assert!(main_w_args(
            args.into_iter()
                .map(|x| x.to_owned())
                .collect::<Vec<String>>()
                .as_slice(),
        )
        .await
        .is_err());
    }

    #[tokio::test]
    async fn test_main_missing_config() {
        let args = vec!["bitslides", "-c", "not-to-be-found"];

        assert!(main_w_args(
            args.into_iter()
                .map(|x| x.to_owned())
                .collect::<Vec<String>>()
                .as_slice(),
        )
        .await
        .is_err());
    }
}
