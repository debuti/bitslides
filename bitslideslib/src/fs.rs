use anyhow::{bail, Result};
use std::path::{Path, PathBuf};

use crate::tracer::Tracer;

use super::{
    config::{Algorithm, CollisionPolicy},
};

/// Move request parameters.
///
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct MoveStrategy {
    /// What to do in case of a file collision
    pub collision: CollisionPolicy,
    /// If true, create a .wip file in the destination and move the file there
    pub safe: bool,
    /// If true, perform a checksum with the provided algorithm of the file before and after moving it
    pub check: Option<Algorithm>,
    /// Number of retries in case of a failure (checksum mismatch, etc)
    pub retries: u8,
}

/// Delete all empty folders inside a path, leave the path root untouched.
///
async fn delete_empty_folders(root: &Path) -> Result<()> {
    /// Recursively delete empty folders, including the root folder.
    ///
    async fn try_delete_empty_folders(root: &Path) -> Result<()> {
        /// Add an exception to the list of exceptions.
        ///
        fn add_exception(exceptions: &mut Vec<PathBuf>, item: &Path) -> Result<()> {
            let item = item.canonicalize()?;

            for exception in &mut *exceptions {
                if exception.starts_with(&item) {
                    // The folder is already recorded
                    return Ok(());
                }
                if item.starts_with(&*exception) {
                    // The folder contains an exception
                    *exception = item;
                    return Ok(());
                }
            }

            exceptions.push(item);

            Ok(())
        }

        /// Check if a path is an exception.
        ///
        fn is_exception(exceptions: &Vec<PathBuf>, item: &Path) -> bool {
            let item = match item.canonicalize() {
                Ok(p) => p,
                Err(_) => return false,
            };
            for exception in exceptions {
                if exception.starts_with(&item) {
                    return true;
                }
            }
            false
        }

        let mut stack = vec![root.to_owned()];
        let mut exceptions: Vec<PathBuf> = vec![];

        // Iterate
        'main: while !stack.is_empty() {
            let mut is_empty = true;

            let current = stack.pop().unwrap();

            // Read the directory
            if let Ok(mut read_dir) = tokio::fs::read_dir(&current).await {
                while let Ok(Some(entry)) = read_dir.next_entry().await {
                    let path = entry.path();

                    is_empty = false;

                    if path.is_dir() {
                        if is_exception(&exceptions, &path) {
                            continue;
                        }
                        stack.push(current);
                        stack.push(path);
                        continue 'main;
                    } else {
                        add_exception(&mut exceptions, &current)?;
                    }
                }
            }
            if is_empty {
                tokio::fs::remove_dir(current).await?;
            }
        }
        Ok(())
    }

    // Fill the jobs queue with all the top-level directories
    if let Ok(mut read_dir) = tokio::fs::read_dir(&root).await {
        while let Ok(Some(entry)) = read_dir.next_entry().await {
            let path = entry.path();
            if path.is_dir() {
                try_delete_empty_folders(&path).await?;
            }
        }
    }

    Ok(())
}

/// Recursively move the contents of one directory to another.
///
pub async fn sync<U: AsRef<Path>, V: AsRef<Path>>(
    from: U,
    to: V,
    dry_run: bool,
    tracer: &Tracer,
    request: &MoveStrategy,
) -> Result<()> {
    let from = PathBuf::from(from.as_ref());
    let to = PathBuf::from(to.as_ref());

    // dbg!(&from, &to);

    let input_root_length = from.components().count();
    let output_root = to;

    let mut jobs = vec![from.clone()];

    while let Some(job) = jobs.pop() {
        log::debug!("process: {:?}", &job);

        // Compose the destination
        let dst = {
            let src = job
                .components()
                .skip(input_root_length)
                .collect::<PathBuf>();

            if src.components().count() == 0 {
                output_root.clone()
            } else {
                output_root.join(&src)
            }
        };

        // Check if the destination exists, otherwise create it
        if std::fs::metadata(&dst).is_err() {
            log::info!("Mkdir: {:?}", dst);
            tracer.log("MKDIR", &format!("{:?}", &dst)).await?;

            if !dry_run {
                std::fs::create_dir_all(&dst)?;
            }
        }

        log::debug!("read_dir: {:?}", &job);
        let mut read_dir = tokio::fs::read_dir(job).await?;
        while let Some(entry) = read_dir.next_entry().await? {
            log::debug!("item: {:?}", &entry);

            let src = entry.path();

            if src.is_dir() {
                jobs.push(src);
                continue;
            }

            match src.file_name() {
                Some(filename) => {
                    log::info!("Move: {:?} -> {:?}", &src, &dst);
                    tracer
                        .log("MV", &format!("{:?} -> {:?}", &src, &dst))
                        .await?;

                    let dst = dst.join(filename);
                    if !dry_run {
                        move_file(&src, &dst, request, checksums::hash_file).await?;
                    }
                }
                None => {
                    log::warn!("failed: {:?}", src);
                }
            }
        }
    }

    if !dry_run {
        delete_empty_folders(&from).await
    } else {
        Ok(())
    }
}

/// Move a single file from one location to another.
///
async fn move_file<F>(
    src_file: &PathBuf,
    mut dst_file: &PathBuf,
    request: &MoveStrategy,
    hash_file: F,
) -> Result<()>
where
    F: Fn(&Path, Algorithm) -> String,
{
    let mut dst_ = None;

    /* Handle a possible collision */
    {
        if dst_file.exists() {
            match request.collision {
                CollisionPolicy::Skip => {
                    return Ok(());
                }
                CollisionPolicy::Fail => {
                    bail!("File already exists: {:?}", dst_file);
                }
                CollisionPolicy::Rename { ref suffix } => {
                    dst_ = Some({
                        let mut new_dst = dst_file.to_path_buf();
                        new_dst.set_extension(suffix);
                        new_dst
                    });
                }
                CollisionPolicy::Overwrite => {
                    // Do nothing. The file will be overwritten by the copy operation
                }
            }
        }

        if let Some(ref new_dst) = dst_ {
            dst_file = new_dst;
        }
    }

    let checksum_src = if let Some(algorithm) = request.check {
        let checksum_src = hash_file(src_file, algorithm);
        log::debug!("Checksum(src): {:?}", checksum_src);
        Some((algorithm, checksum_src))
    } else {
        None
    };

    let wip = if request.safe {
        // FIXME: If the file is photo.jpg the wip needs to be .photo.jpg.wip
        &dst_file.with_extension("wip")
    } else {
        dst_file
    };

    let mut retry_count = 0;
    // <= because the first attempt is not a retry
    while retry_count <= request.retries {
        log::debug!("Moving {:?} -> {:?}", src_file, wip);

        // TODO: Optimize this copy to be able to resume the copy if it fails
        tokio::fs::copy(src_file, wip).await?;

        // Check that the file was copied correctly
        if let Some((algorithm, ref checksum_src)) = checksum_src {
            let checksum_wip = hash_file(wip, algorithm);
            log::debug!("Checksum(wip): {:?}", checksum_wip);
            if checksum_src != &checksum_wip {
                retry_count += 1;
                continue;
            }
        }

        if request.safe {
            tokio::fs::rename(wip, dst_file).await?;
        }

        tokio::fs::remove_file(src_file).await?;

        return Ok(());
    }

    if wip.exists() {
        tokio::fs::remove_file(wip).await?;
    }

    bail!("Failed to move file {:?} after maximum retries", src_file);
}

#[cfg(test)]
mod tests;
