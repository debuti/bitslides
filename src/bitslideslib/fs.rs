use anyhow::{bail, Result};
use checksums::hash_file;
use std::path::{Path, PathBuf};

/// Recursively copy the contents of one directory to another.
pub async fn sync<U: AsRef<Path>, V: AsRef<Path>>(
    from: U,
    to: V,
    request: &MoveRequest,
) -> Result<()> {
    let from = PathBuf::from(from.as_ref());
    let to = PathBuf::from(to.as_ref());

    let mut jobs = Vec::new();

    let input_root_length = from.components().count();
    let output_root = to;

    jobs.push(from);

    while let Some(job) = jobs.pop() {
        println!("process: {:?}", &job);

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
            println!(" mkdir: {:?}", dst);
            std::fs::create_dir_all(&dst)?;
        }

        for entry in std::fs::read_dir(job)? {
            let src = entry?.path();

            if src.is_dir() {
                jobs.push(src);
                continue;
            }

            match src.file_name() {
                Some(filename) => {
                    let dst = dst.join(filename);
                    println!("  copy: {:?} -> {:?}", &src, &dst);
                    // std::fs::copy(&path, &dest_path)?;
                    move_file(&src, &dst, &request).await?;
                }
                None => {
                    //TODO: Print warning
                    println!("failed: {:?}", src);
                }
            }
        }
    }

    Ok(())
}

// /// Sync the contents of the source folder to the destination folder. Recursively
// async fn sync(src: &PathBuf, dst: &PathBuf) -> Result<()> {
//     println!("Sync {:?} -> {:?}", src, dst);

//     // let entries = src.read_dir();
//     // if entries.is_err() {
//     //     bail!("{src:?} cannot be read");
//     // }

//     // for entry in entries?.flatten() {
//     //     let entry_path = entry.path();
//     //     let file_type = entry.file_type();
//     //     if let Ok(file_type) = file_type {
//     //         if file_type.is_dir() {
//     //             let dst = dst.join(entry.file_name());
//     //             if !dst.exists() {
//     //                 std::fs::create_dir_all(&dst)?;
//     //             }
//     //             sync(&entry_path, &dst).await?;
//     //             continue;
//     //         }
//     //         println!("Warning: {} is not a directory", entry_path.display());
//     //     }
//     // }

//     Ok(())
// }

pub enum ColisionPolicy {
    /// Overwrite the destination file
    Overwrite,
    /// Skip the file
    Skip,
    /// Rename the file
    Rename { suffix: String },
    /// Fail the operation
    Fail,
}

pub use checksums::Algorithm;
pub struct MoveRequest {
    /// What to do in case of a file colision
    pub colision: ColisionPolicy,
    /// If true, create a .wip file in the destination and move the file there
    pub safe: bool,
    /// If true, perform a checksum with the provided algorithm of the file before and after moving it
    pub checked: Option<Algorithm>,
    /// Number of retries in case of a failure (checksum mismatch, etc)
    pub retries: u8,
}

async fn move_file(src: &PathBuf, mut dst: &PathBuf, _request: &MoveRequest) -> Result<()> {
    let mut dst_ = None;

    /* Handle a possible collision */
    {
        if dst.exists() {
            match _request.colision {
                ColisionPolicy::Skip => {
                    return Ok(());
                }
                ColisionPolicy::Fail => {
                    bail!("File already exists: {:?}", dst);
                }
                ColisionPolicy::Rename { ref suffix } => {
                    dst_ = Some({
                        let mut new_dst = dst.to_path_buf();
                        new_dst.set_extension(&suffix);
                        new_dst
                    });
                }
                ColisionPolicy::Overwrite => {
                    // Do nothing. The file will be overwritten by the copy operation
                }
            }
        }

        if dst_.is_some() {
            dst = dst_.as_ref().unwrap();
        }
    }

    let checksum_src = if let Some(algorithm) = _request.checked {
        let checksum_src = hash_file(src, algorithm);
        println!("Checksum(src): {:?}", checksum_src);
        Some((algorithm, checksum_src))
    } else {
        None
    };

    let wip = if _request.safe {
        // FIXME: If the file is photo.jpg the wip needs to be .photo.jpg.wip
        &dst.with_extension("wip")
    } else {
        dst
    };

    let mut retry_count = 0;
    while retry_count < _request.retries {
        println!("Moving {:?} -> {:?}", src, wip);

        // TODO: Optimize this copy to be able to resume the copy if it fails
        tokio::fs::copy(src, wip).await?;

        // Check that the file was copied correctly
        if let Some((algorithm, ref checksum_src)) = checksum_src {
            let checksum_wip = hash_file(src, algorithm);
            println!("Checksum(wip): {:?}", checksum_wip);
            if checksum_src != &checksum_wip {
                retry_count += 1;
                continue;
            }
        }

        if _request.safe {
            std::fs::rename(&wip, dst)?;
        }

        std::fs::remove_file(src)?;

        return Ok(());
    }

    bail!("Failed to move file \"{:?}\", maximum retries", src);
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs::{self, File};
    use std::io::Write;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_copy_directory() {
        let temp_dir = tempdir().unwrap();
        let src_dir = temp_dir.path().join("src");
        let dest_dir = temp_dir.path().join("dest");

        let requests = [
            MoveRequest {
                colision: ColisionPolicy::Fail,
                safe: false,
                checked: None,
                retries: 1,
            },
            MoveRequest {
                colision: ColisionPolicy::Overwrite,
                safe: false,
                checked: Some(Algorithm::CRC32),
                retries: 1,
            },
            MoveRequest {
                colision: ColisionPolicy::Skip,
                safe: true,
                checked: None,
                retries: 1,
            },
            MoveRequest {
                colision: ColisionPolicy::Rename { suffix: "bro".to_owned() },
                safe: true,
                checked: Some(Algorithm::CRC64),
                retries: 1,
            },
        ];

        for request in &requests {
            // Create source directory structure
            fs::create_dir_all(&src_dir).unwrap();
            let src_file_path = src_dir.join("test.txt");
            let mut src_file = File::create(&src_file_path).unwrap();
            writeln!(src_file, "Hello, world!").unwrap();

            // Perform copy
            sync(&src_dir, &dest_dir, request).await.unwrap();
            println!("---");

            // Verify destination directory structure
            let dest_file_path = dest_dir.join("test.txt");
            assert!(dest_file_path.exists());
            let content = fs::read_to_string(dest_file_path).unwrap();
            assert_eq!(content, "Hello, world!\n");

            // Clean up
            fs::remove_dir_all(&temp_dir).unwrap();
        }
    }

    #[tokio::test]
    async fn test_copy_empty_directory() {
        let temp_dir = tempdir().unwrap();
        let src_dir = temp_dir.path().join("src");
        let dest_dir = temp_dir.path().join("dest");

        // Create empty source directory
        fs::create_dir(&src_dir).unwrap();

        // Perform copy
        sync(
            &src_dir,
            &dest_dir,
            &MoveRequest {
                colision: ColisionPolicy::Fail,
                safe: false,
                checked: Some(Algorithm::CRC32),
                retries: 5,
            },
        )
        .await
        .unwrap();

        // Verify destination directory exists
        assert!(dest_dir.exists());
    }

    #[tokio::test]
    async fn test_copy_nested_directories() {
        let temp_dir = tempdir().unwrap();
        let src_dir = temp_dir.path().join("src");
        let nested_dir = src_dir.join("nested");
        let dest_dir = temp_dir.path().join("dest");

        // Create nested directory structure
        fs::create_dir_all(&nested_dir).unwrap();
        let src_file_path = nested_dir.join("test.txt");
        let mut src_file = File::create(&src_file_path).unwrap();
        writeln!(src_file, "Nested file").unwrap();

        // Perform copy
        sync(
            &src_dir,
            &dest_dir,
            &MoveRequest {
                colision: ColisionPolicy::Fail,
                safe: false,
                checked: Some(Algorithm::CRC32),
                retries: 5,
            },
        )
        .await
        .unwrap();

        // Verify destination directory structure
        let dest_nested_dir = dest_dir.join("nested");
        let dest_file_path = dest_nested_dir.join("test.txt");
        assert!(dest_file_path.exists());
        let content = fs::read_to_string(dest_file_path).unwrap();
        assert_eq!(content, "Nested file\n");
    }
}
