use clap::{value_parser, Arg, ArgAction, ArgMatches, Command};
use std::path::PathBuf;

const APP_NAME: &str = env!("CARGO_PKG_NAME");
const APP_VERS: &str = env!("CARGO_PKG_VERSION");

fn default_config_files() -> Vec<PathBuf> {
    let mut paths = vec![PathBuf::from("/etc/synchers/default.conf")];

    if let Some(home_dir) = dirs::home_dir() {
        paths.push(home_dir.join(".synchers/default.conf"));
    }

    paths
}

pub fn cli() -> ArgMatches {
    Command::new(APP_NAME)
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
        .get_matches()
}
