use anyhow::{bail, Result};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc::Sender;

use super::{
    config::{Algorithm, CollisionPolicy},
    syncjob::SyncJob,
};

/// Move request parameters.
/// 
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct MoveRequest {
    /// What to do in case of a file collision
    pub collision: CollisionPolicy,
    /// If true, create a .wip file in the destination and move the file there
    pub safe: bool,
    /// If true, perform a checksum with the provided algorithm of the file before and after moving it
    pub check: Option<Algorithm>,
    /// Number of retries in case of a failure (checksum mismatch, etc)
    pub retries: u8,
}

/// Recursively move the contents of one directory to another.
///
pub async fn sync<U: AsRef<Path>, V: AsRef<Path>>(
    from: U,
    to: V,
    dry_run: bool,
    tracer: &Option<(Sender<Option<String>>, &SyncJob)>,
    request: &MoveRequest,
) -> Result<()> {
    let from = PathBuf::from(from.as_ref());
    let to = PathBuf::from(to.as_ref());

    let mut jobs = Vec::new();

    let input_root_length = from.components().count();
    let output_root = to;

    jobs.push(from);

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

            if let Some((tracer, syncjob)) = &tracer {
                tracer
                    .send(Some(format!("[{:?}] MKDIR {:?}", syncjob, dst,)))
                    .await?;
            }

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
                    log::info!("Copy: {:?} -> {:?}", &src, &dst);
                    if let Some((tracer, syncjob)) = &tracer {
                        tracer
                            .send(Some(format!("[{:?}] CP {:?} -> {:?}", syncjob, src, dst,)))
                            .await?;
                    }
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

    Ok(())
}

/// Move a single file from one location to another.
///
async fn move_file<F>(
    src_file: &PathBuf,
    mut dst_file: &PathBuf,
    request: &MoveRequest,
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

        if dst_.is_some() {
            dst_file = dst_.as_ref().unwrap();
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
