use anyhow::{bail, Result};
use clap::{builder::Str, value_parser, Arg, ArgAction, Command};
use dirs::home_dir;
use serde::Deserialize;
use std::{fs, path::PathBuf};

const APP_NAME: &'static str = env!("CARGO_PKG_NAME");
const APP_VERS: &'static str = env!("CARGO_PKG_VERSION");

#[derive(Deserialize)]
struct Config {
    keyword: Option<String>,
    roots: Vec<PathBuf>,
}

#[derive(Debug)]

struct Location {
    name: String,
    path: PathBuf,
}

impl Location {
    fn new(name: String, path: PathBuf) -> Self {
        Self {
            name: name,
            path: path,
        }
    }
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
            if let Ok(read_config) = read_yaml_config(config_path.to_str().unwrap()) {
                config = read_config;
            } else {
                println!(" Invalid config format");
                continue;
            }
        } else {
            println!(" Config not found");
            continue;
        };

        identify_volumes(
            &config.keyword.unwrap_or("Queues".to_owned()),
            &config.roots,
        )
        .await;

        run = true;
    }

    if !run {
        panic!("No valid configuration file found");
    }
}

async fn identify_volumes(keyword: &str, roots: &[PathBuf]) {
    let mut volumes: Vec<Location> = Vec::new();

    for root in roots {
        let root_str = root.to_string_lossy();

        // Implies .exists()
        if !root.is_dir() {
            println!("{root_str} is not a folder");
            continue;
        }

        let entries = root.read_dir();

        if entries.is_err() {
            println!("{root_str} cannot be read");
            continue;
        }

        println!("{root_str}");

        // Analyze the environment
        for entry in entries.unwrap() {
            if let Ok(entry) = entry {
                let entry_path = entry.path();
                let file_type = entry.file_type();
                if let Ok(file_type) = file_type {
                    if file_type.is_dir() {
                        let queues_path = entry_path.join(keyword);
                        if queues_path.exists() {
                            volumes.push(Location::new(
                                entry_path
                                    .file_name()
                                    .unwrap()
                                    .to_string_lossy()
                                    .to_string(),
                                queues_path,
                            ));
                        }
                    }
                }
            }
        }
    }

    println!("Found volumes: {volumes:?}");

    for volume in &volumes {
        if let Ok(queues) = identify_queues(volume) {
            println!("{queues:?} identified");
        } else {
            println!("{volume:?} failed to sync");
        }
    }

    // for dst in &dsts {
    //     synco(src, dst)?;
    // }
}

fn identify_queues(src: &Location) -> Result<Vec<Location>> {
    let mut dsts = Vec::new();

    let subfolders = src.path.read_dir();

    if subfolders.is_err() {
        bail!("Unable to read the folder: {src:?}");
    }

    for entry in subfolders.unwrap() {
        if let Ok(entry) = entry {
            let entry_fullpath = entry.path();
            let entry_name = entry_fullpath
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string();
            println!("{entry_name}\t{entry_fullpath:?}");
            dsts.push(Location::new(entry_name, entry_fullpath));
        }
    }

    Ok(dsts)
}

fn synco(src: &Location, dst: &Location) -> Result<()> {
    println!("Syncing {src:?} -> {dst:?}");
    Ok(())
}
