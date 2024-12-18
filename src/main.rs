use anyhow::{bail, Result};
use bitslides::{slide, GlobalConfig, RootsetConfig};
use config::DEFAULT_KEYWORD;
use std::path::PathBuf;

mod cli;
mod config;

#[tokio::main]
async fn main() -> Result<()> {
    let matches = cli::cli();

    // Get the configuration files
    let config_files = matches
        .get_many::<PathBuf>("config")
        .expect("No configuration file found among provided/defaults.");

    // FIXME: Use a logging framework
    let _verbose = matches.get_one::<u8>("verbose").unwrap_or(&0);

    let dry_run = matches.get_flag("dry-run");

    let rootsets = process_all_configs(config_files.into_iter().collect())?;

    slide(GlobalConfig {
        roots: rootsets,
        dry_run,
    })
    .await
}

fn process_all_configs(config_files: Vec<&PathBuf>) -> Result<Vec<RootsetConfig>> {
    let mut success = false;
    let mut result = Vec::new();

    for config_path in config_files {
        print!("Loading configuration from: {:?}... ", config_path);

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
                println!("Invalid config format");
                continue;
            }
        } else {
            println!("Config not found");
            continue;
        };

        success = true;
    }

    if !success {
        bail!("No valid configuration file found");
    }

    Ok(result)
}

