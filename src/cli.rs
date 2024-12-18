use clap::{value_parser, Arg, ArgAction, ArgMatches, Command};
use std::path::PathBuf;

const APP_NAME: &str = env!("CARGO_PKG_NAME");
const APP_VERS: &str = env!("CARGO_PKG_VERSION");

/// Returns a list of default configuration files.
fn default_config_files() -> Vec<PathBuf> {
    let mut paths = vec![PathBuf::from("/etc/synchers/default.conf")];

    if let Some(home_dir) = dirs::home_dir() {
        paths.push(home_dir.join(".synchers/default.conf"));
    }

    paths
}

/// Parses command line arguments and returns a `clap::ArgMatches` instance.
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
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .help("Prints verbose output (more verbose with multiple -v)")
                .action(ArgAction::Count)
                .value_parser(value_parser!(u8))
                .required(false),
        )
        .arg(
            Arg::new("dry-run")
                .short('n')
                .long("dry-run")
                .help("Performs a dry run without making any changes")
                .action(ArgAction::SetTrue)
                .required(false),
        )
        .get_matches()
}
