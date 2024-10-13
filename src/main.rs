use clap::{
    value_parser, Arg, ArgAction, Command,
};
use dirs::home_dir;
use serde::Deserialize;
use std::{fs, path::PathBuf};
use anyhow::Result;


const APP_NAME: &'static str = env!("CARGO_PKG_NAME");
const APP_VERS: &'static str = env!("CARGO_PKG_VERSION");

#[derive(Deserialize)]
struct Config {
    sync_interval: u64, // Sync interval in seconds
    source: String,     // Source folder
    target: String,     // Target folder
}


fn read_yaml_config(file_path: &str) -> Result<Config> {
    let file_content = fs::read_to_string(file_path)?;
    let config = serde_yaml::from_str(&file_content)?;
    Ok(config)
}

fn default_config_files() -> Vec<PathBuf> {
    let mut paths = vec![PathBuf::from("/etc/synchers/default.conf")];

    if let Some(home_dir) = home_dir() {
        paths.push(home_dir.join(".synchers/default.conf"));
    }

    paths

    // let result = paths.into_iter().filter(|p| p.exists()).collect();

    // result
}

#[tokio::main]
async fn main() {
    let mut run = false;

    let matches = Command::new(APP_NAME)
        .version(APP_VERS)
        .about("Synchronizes contents between locations")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("root_config")
                .help("Specify a custom config file")
                .action(ArgAction::Append)
                .value_parser(value_parser!(PathBuf))
                .default_values(
                    default_config_files()
                        .iter()
                        .filter_map(|p| p.to_str().map(|s| s.to_owned()))
                        .collect::<Vec<String>>(),
                )
                .required(false),
        )
        .get_matches();

    //
    let config_files = matches
        .get_many::<PathBuf>("config")
        .expect("No configuration file found among provided/defaults.")
        .into_iter();

    for config_path in config_files {
        println!("Loading configuration from: {:?}", config_path);
        let config;

        if config_path.exists() {
            if let Ok(read_config) = read_yaml_config(config_path.to_str().unwrap())
            {
                config = read_config;
            }
            else {
                println!(" Invalid config format");
                continue;
            }
        } else {
            println!(" Config not found");
            continue;
        };

        sync_folders(&config.source, &config.target, config.sync_interval).await;

        run = true;
    }

    if !run {
        panic!("No valid configuration file found");
    }
}

async fn sync_folders(source: &str, target: &str, interval: u64) {
    println!(
        "Syncing from {} to {} every {} seconds",
        source, target, interval
    );
    // Add sync logic here
}
