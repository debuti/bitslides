use anyhow::{bail, Result};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use tokio::task::JoinHandle;

mod cli;
mod config;

const DEFAULT_SLIDE_CONFIG_FILE: &str = ".slide.yml";
const DEFAULT_KEYWORD: &str = "Queues";

#[derive(Debug, PartialEq)]
struct SyncJob<'a> {
    src: &'a str,   //String,
    dst: &'a str,   //String,
    issue: &'a str, //String,
}

#[derive(Debug)]
struct Slide {
    /// Name of the destination volume
    name: String,
    /// Path to the slide. Ex. /path/to/volumes/foo/slides/bar
    path: PathBuf,
    /// Name of the default route towards the destination volume
    or_else: Option<String>,
}

impl Slide {
    fn new(name: String, path: PathBuf, or_else: Option<String>) -> Self {
        Self {
            name,
            path,
            or_else,
        }
    }
}

impl std::fmt::Display for Slide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.or_else.is_none() {
            write!(f, "{}", self.name,)
        } else {
            write!(f, "{} (->{})", self.name, self.or_else.as_ref().unwrap())
        }
    }
}

#[derive(Debug)]
struct Volume {
    /// Name of the volume
    name: String,
    /// Path to the volume root. Ex. /path/to/volumes/foo
    path: PathBuf,
    /// Slides that are part of the volume. Including the volume mailbox
    slides: HashMap<String, Slide>,
}

impl Volume {
    fn new(name: String, path: PathBuf) -> Self {
        Self {
            name,
            path,
            slides: HashMap::new(),
        }
    }

    fn add_slide(&mut self, slide: Slide) {
        self.slides.insert(slide.name.clone(), slide);
    }
}

impl std::fmt::Display for Volume {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)?;
        for (_, slide) in &self.slides {
            write!(f, "\n  - {}", slide)?;
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let matches = cli::cli();

    //
    let config_files = matches
        .get_many::<PathBuf>("config")
        .expect("No configuration file found among provided/defaults.");

    let volumes = process_all_configs(config_files.into_iter().collect())?;

    println!("Volumes for all configs: {volumes:#?}");
    println!("----");

    let syncjobs = build_syncjobs(&volumes);

    println!("Sync jobs: {syncjobs:#?}");

    execute_syncjobs(&volumes, &syncjobs).await
}

fn process_all_configs(config_files: Vec<&PathBuf>) -> Result<HashMap<String, Volume>> {
    let mut success = false;
    let mut volumes = HashMap::new();

    for config_path in config_files {
        print!("Loading configuration from: {:?}... ", config_path);

        if config_path.exists() {
            if let Ok(config) = config::read_global_config(config_path.to_str().unwrap()) {
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

                println!("Ok");
                let some_volumes = process_config(
                    &config.keyword.unwrap_or(DEFAULT_KEYWORD.to_owned()),
                    &roots,
                );
                if some_volumes.is_err() {
                    println!("Error processing the configuration {config_path:?}");
                    continue;
                }

                volumes.extend(some_volumes.unwrap());
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

    Ok(volumes)
}

fn process_config(keyword: &str, roots: &[PathBuf]) -> Result<HashMap<String, Volume>> {
    let mut volumes: HashMap<String, Volume> = HashMap::new();

    // Identify the volumes in each root
    for root in roots {
        match identify_volumes(root, keyword) {
            Ok(v) => volumes.extend(v),
            Err(e) => println!("{e}"),
        }
    }
    println!("Found volumes: {volumes:?}");

    // Identify the slides of each volume
    for (_, volume) in volumes.iter_mut() {
        match identify_slides(volume, keyword) {
            Ok(_) => {}
            Err(e) => {
                println!("{e}");
            }
        }
    }
    println!("Completed volumes: {volumes:#?}");

    Ok(volumes)
}

/// Identify volumes inside a each root folder.
/// A volume is a folder that contains a slides subfolder (or the chosen keyword).
/// This subfolder contains the folders whose names will have to match the name of other volumes.
fn identify_volumes(root: &Path, keyword: &str) -> Result<HashMap<String, Volume>> {
    let mut volumes = HashMap::new();

    let root_str = root.to_string_lossy();

    // Implies .exists()
    if !root.is_dir() {
        bail!("{root_str} is not a folder");
    }

    let entries = root.read_dir();
    if entries.is_err() {
        bail!("{root_str} cannot be read");
    }

    println!("{root_str}");

    // Analyze the contents of the root folder
    for entry in entries?.flatten() {
        let entry_path = entry.path();
        let file_type = entry.file_type();
        if let Ok(file_type) = file_type {
            if file_type.is_dir() {
                let slides_path = entry_path.join(keyword);
                if slides_path.exists() {
                    let name = entry_path
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .to_string();
                    volumes.insert(name.clone(), Volume::new(name, entry_path));
                }
            }
        }
    }

    Ok(volumes)
}

fn identify_slides(volume: &mut Volume, keyword: &str) -> Result<()> {
    let subfolders = volume.path.join(keyword).read_dir();

    if subfolders.is_err() {
        bail!("Unable to read the folder: {volume:?}");
    }

    for entry in subfolders?.flatten() {
        let entry_metadata = entry.metadata();
        if entry_metadata.is_err() {
            continue;
        }
        if entry_metadata.unwrap().is_dir() {
            let entry_fullpath = entry.path();
            let entry_name = entry_fullpath
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string();

            // Try to fetch the slide configuration if any
            let slide_conf = {
                let slide_conf =
                    config::read_slide_config(entry_fullpath.join(DEFAULT_SLIDE_CONFIG_FILE));
                match slide_conf {
                    Ok(s) => s.route,
                    Err(_) => None,
                }
            };

            volume.add_slide(Slide::new(entry_name, entry_fullpath, slide_conf));
        }
    }

    Ok(())
}

fn build_syncjobs(volumes: &HashMap<String, Volume>) -> Vec<SyncJob> {
    let mut syncjobs = Vec::new();

    for src_name in volumes.keys() {
        for (dst_name, slide) in &volumes[src_name].slides {
            if src_name == dst_name {
                continue;
            }

            // If the destination volume is available, its a direct slide
            if volumes.contains_key(dst_name) {
                syncjobs.push(SyncJob {
                    src: src_name,
                    dst: dst_name,
                    issue: dst_name,
                });
                continue;
            }

            match &slide.or_else {
                // If the slide has a default route, and the default route is available, its a indirect slide
                Some(default_route) => {
                    if volumes.contains_key(default_route) {
                        syncjobs.push(SyncJob {
                            src: src_name,
                            dst: default_route,
                            issue: dst_name,
                        });
                        continue;
                    }
                    println!("default_route {default_route} not available");
                }
                _ => {
                    println!("{dst_name} not available and no default route");
                }
            }
        }
    }

    syncjobs
}

//TODO: Implement the sync function
//TODO: Implement the forward function. Figure out how to sort the syncs to minimize the number of operations.
//TODO: Implement the tidy-up function. In each volume we should process the slide of the volume to move the contents to other places inside the volume

async fn execute_syncjobs(
    volumes: &HashMap<String, Volume>,
    syncjobs: &[SyncJob<'_>],
) -> Result<()> {
    let mut handles = Vec::new();

    for (_, volume) in volumes.iter() {
        println!("Volume: {volume}");
    }

    for syncjob in syncjobs {
        println!(
            "Syncing {:?} -{:?}-> {:?}",
            syncjob.src, syncjob.issue, syncjob.dst
        );
        let src = volumes[syncjob.src].slides[syncjob.issue].path.clone();
        let dst = volumes[syncjob.dst].slides[syncjob.issue].path.clone();

        let handle: JoinHandle<Result<()>> = tokio::spawn(async move {
            if let Err(e) = sync(&src, &dst).await {
                bail!("Error syncing {:?} -> {:?}: {:?}", src, dst, e);
            }
            Ok(())
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.await??;
    }

    Ok(())
}

async fn sync(src: &PathBuf, dst: &PathBuf) -> Result<()> {
    println!("Syncing {:?} -> {:?}", src, dst);
    Ok(())
}

#[cfg(test)]
mod tests;
