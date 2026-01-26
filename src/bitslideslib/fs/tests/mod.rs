use crate::bitslideslib::tracer;

use super::*;

use std::fs::{self, File};
use std::io::Write;
use tempfile::{tempdir, TempDir};

/// Test that a empty folder structure is completely removed, but for the root folder.
#[tokio::test]
async fn test_delete_empty_folders_everything_to_ashes() {
    let temp_dir = tempdir().unwrap();
    // Prerequisite: Create directory structure
    {
        // root
        // ├── a
        // │   |── c
        // │   └── d
        // │       └── e
        // │           └── f
        // └── b
        fs::create_dir_all(&temp_dir.path().join("a").join("c")).unwrap();
        fs::create_dir_all(&temp_dir.path().join("a").join("d").join("e").join("f")).unwrap();
        fs::create_dir_all(&temp_dir.path().join("b")).unwrap();
    }

    // Action: Delete empty folders
    delete_empty_folders(&temp_dir.path()).await.unwrap();

    // Check: Verify the folders that should remain
    {
        assert!(temp_dir.path().exists());
    }
    // Check: Verify the folders that should have been removed
    {
        assert!(!temp_dir.path().join("a").exists());
        assert!(!temp_dir.path().join("b").exists());
    }
}

/// Test that an almost empty folder structure can't be completely removed.
#[tokio::test]
async fn test_delete_empty_folders_but_cant_completely() {
    let temp_dir = tempdir().unwrap();
    // Prerequisite: Create directory structure
    {
        // root
        // ├── a
        // │   |── b
        // │   |── c
        // │   |── d
        // │   |   └── file.txt
        // │   └── e
        // │       └── f
        // │           |── file.txt
        // │           |── g
        // │           └── h
        // |               └── file.txt
        // |── i
        // |   └── j
        // |       └── file.txt
        // └── k
        fs::create_dir_all(&temp_dir.path().join("a").join("b")).unwrap();
        fs::create_dir_all(&temp_dir.path().join("a").join("c")).unwrap();
        fs::create_dir_all(&temp_dir.path().join("a").join("d")).unwrap();
        File::create(&temp_dir.path().join("a").join("d").join("file.txt")).unwrap();
        fs::create_dir_all(&temp_dir.path().join("a").join("e").join("f").join("g")).unwrap();
        fs::create_dir_all(&temp_dir.path().join("a").join("e").join("f").join("h")).unwrap();
        File::create(
            &temp_dir
                .path()
                .join("a")
                .join("e")
                .join("f")
                .join("file.txt"),
        )
        .unwrap();
        File::create(
            &temp_dir
                .path()
                .join("a")
                .join("e")
                .join("f")
                .join("h")
                .join("file.txt"),
        )
        .unwrap();
        fs::create_dir_all(&temp_dir.path().join("i").join("j")).unwrap();
        File::create(&temp_dir.path().join("i").join("j").join("file.txt")).unwrap();
        fs::create_dir_all(&temp_dir.path().join("k")).unwrap();
    }

    // Action: Delete empty folders
    delete_empty_folders(&temp_dir.path()).await.unwrap();

    // Check: Verify the folders that should remain
    {
        assert!(temp_dir.path().exists());
        assert!(temp_dir
            .path()
            .join("a")
            .join("d")
            .join("file.txt")
            .exists());
        assert!(temp_dir
            .path()
            .join("a")
            .join("e")
            .join("f")
            .join("file.txt")
            .exists());
        assert!(temp_dir
            .path()
            .join("a")
            .join("e")
            .join("f")
            .join("h")
            .join("file.txt")
            .exists());
        assert!(temp_dir
            .path()
            .join("i")
            .join("j")
            .join("file.txt")
            .exists());
    }
    // Check: Verify the folders that should have been removed
    {
        assert!(!temp_dir.path().join("a").join("b").exists());
        assert!(!temp_dir.path().join("a").join("c").exists());
        assert!(!temp_dir
            .path()
            .join("a")
            .join("e")
            .join("f")
            .join("g")
            .exists());
        assert!(!temp_dir.path().join("k").exists());
    }
}

/// Test that a file is copied from the source to the destination directory with different strategies.
#[tokio::test]
async fn test_sync_directory() {
    // root
    // ├── src
    // │   └── test.txt
    // └── dest
    let temp_dir = tempdir().unwrap();
    let src_dir = temp_dir.path().join("src");
    let dest_dir = temp_dir.path().join("dest");

    let requests = [
        MoveStrategy {
            collision: CollisionPolicy::Fail,
            safe: false,
            check: None,
            retries: 1,
        },
        MoveStrategy {
            collision: CollisionPolicy::Overwrite,
            safe: false,
            check: Some(Algorithm::CRC32),
            retries: 1,
        },
        MoveStrategy {
            collision: CollisionPolicy::Skip,
            safe: true,
            check: None,
            retries: 1,
        },
        MoveStrategy {
            collision: CollisionPolicy::Rename {
                suffix: "bro".to_owned(),
            },
            safe: true,
            check: Some(Algorithm::CRC64),
            retries: 1,
        },
    ];

    let (tracer, handle) = tracer::Tracer::new(&Some(&PathBuf::from("/dev/null")))
        .await
        .unwrap();

    for request in &requests {
        // Create source directory structure
        fs::create_dir_all(&src_dir).unwrap();
        let src_file_path = src_dir.join("test.txt");
        let mut src_file = File::create(&src_file_path).unwrap();
        writeln!(src_file, "Hello, world!").unwrap();

        // Perform copy
        sync(&src_dir, &dest_dir, false, &tracer, request)
            .await
            .unwrap();
        println!("---");

        // Verify destination directory structure
        let dest_file_path = dest_dir.join("test.txt");
        assert!(dest_file_path.exists());
        let content = fs::read_to_string(dest_file_path).unwrap();
        assert_eq!(content, "Hello, world!\n");

        // Clean up
        fs::remove_dir_all(&temp_dir).unwrap();
    }

    handle.unwrap().await.unwrap();
}

/// Test that nothing happens when the source directory is empty.
#[tokio::test]
async fn test_sync_empty_directory() {
    let (tracer, handle) = tracer::Tracer::new(&Some(&PathBuf::from("/dev/null")))
        .await
        .unwrap();

    // root
    // ├── src
    // └── dest
    let temp_dir = tempdir().unwrap();
    let src_dir = temp_dir.path().join("src");
    let dest_dir = temp_dir.path().join("dest");

    // Prerequisite: Create empty source directory
    fs::create_dir(&src_dir).unwrap();

    // Check: Verify destination directory now exists
    assert!(!dest_dir.exists());

    // Action: Sync
    sync(
        &src_dir,
        &dest_dir,
        false,
        &tracer,
        &MoveStrategy {
            collision: CollisionPolicy::Fail,
            safe: false,
            check: Some(Algorithm::CRC32),
            retries: 5,
        },
    )
    .await
    .unwrap();

    // Check: Verify destination directory now exists
    assert!(dest_dir.exists());

    // Check: The destination directory is empty
    {
        let entries = fs::read_dir(&dest_dir).unwrap();
        assert_eq!(entries.count(), 0);
    }

    handle.unwrap().await.unwrap();
}

/// Test that a file belonging to a nested directory is copied from the source to the destination directory.
#[tokio::test]
async fn test_sync_nested_directories() {
    let (tracer, handle) = tracer::Tracer::new(&Some(&PathBuf::from("/dev/null")))
        .await
        .unwrap();

    // root
    // ├── src
    // │   └── nested
    // │       └── test.txt
    // └── dest
    let temp_dir = tempdir().unwrap();
    let src_dir = temp_dir.path().join("src");
    let nested_dir = src_dir.join("nested");
    let dest_dir = temp_dir.path().join("dest");

    // Prerequisite: Create nested directory structure
    fs::create_dir_all(&nested_dir).unwrap();
    let src_file_path = nested_dir.join("test.txt");
    let mut src_file = File::create(&src_file_path).unwrap();
    writeln!(src_file, "Nested file").unwrap();

    // Action: Sync
    sync(
        &src_dir,
        &dest_dir,
        false,
        &tracer,
        &MoveStrategy {
            collision: CollisionPolicy::Fail,
            safe: false,
            check: Some(Algorithm::CRC32),
            retries: 5,
        },
    )
    .await
    .unwrap();

    // Check: Verify destination
    {
        let dest_nested_dir = dest_dir.join("nested");
        let dest_file_path = dest_nested_dir.join("test.txt");

        // Check: Verify destination directory structure
        assert!(dest_file_path.exists());

        // Check: Verify destination file content
        {
            let content = fs::read_to_string(dest_file_path).unwrap();
            assert_eq!(content, "Nested file\n");
        }
    }

    // Check: Verify source directory structure is unchanged
    assert!(!src_file_path.exists());
    assert!(!nested_dir.exists());

    handle.unwrap().await.unwrap();
}

/// Setup the environment for testing all move_file permutations.
fn setup_move_file() -> (TempDir, PathBuf, PathBuf) {
    let tmp_dir = tempdir().unwrap();
    let src_dir = tmp_dir.path().join("src");
    let dst_dir = tmp_dir.path().join("dst");

    // Prerequisite: Create source directory structure
    fs::create_dir_all(&src_dir).unwrap();
    let src_file = src_dir.join("test.txt");
    write!(File::create(&src_file).unwrap(), "source").unwrap();

    fs::create_dir_all(&dst_dir).unwrap();
    let dst_file = dst_dir.join("test.txt");

    (tmp_dir, src_file, dst_file)
}

/// Test move_file if there is a collision and the policy is set to fail.
#[tokio::test]
async fn test_move_file_collision_fail() {
    // Prerequisite: Setup environment
    let (_tmp_dir, src_file, dst_file) = setup_move_file();

    // Prerequisite: Create a file in the destination directory
    File::create(&dst_file).unwrap();

    // Action: Move file with collision policy set to fail
    let result = move_file(
        &src_file,
        &dst_file,
        &MoveStrategy {
            collision: CollisionPolicy::Fail,
            safe: false,
            check: None,
            retries: 5,
        },
        checksums::hash_file,
    )
    .await;

    // Check: The operation failed and nothing changed
    assert!(
        result.is_err()
            && result
                .unwrap_err()
                .to_string()
                .contains("File already exists")
    );
    assert!(
        src_file.exists() && fs::read_to_string(&src_file).unwrap() == "source".to_owned(),
        "src_file contents: {:?}",
        fs::read_to_string(&src_file).unwrap()
    );
    assert!(
        dst_file.exists() && fs::read_to_string(&dst_file).unwrap() == "".to_owned(),
        "dst_file contents: {:?}",
        fs::read_to_string(&dst_file).unwrap()
    );
}

/// Test move_file if there is a collision and the policy is set to skip.
#[tokio::test]
async fn test_move_file_collision_skip() {
    // Prerequisite: Setup environment
    let (_tmp_dir, src_file, dst_file) = setup_move_file();

    // Prerequisite: Create a file in the destination directory
    File::create(&dst_file).unwrap();

    // Action: Move file with collision policy set to skip
    let result = move_file(
        &src_file,
        &dst_file,
        &MoveStrategy {
            collision: CollisionPolicy::Skip,
            safe: false,
            check: None,
            retries: 5,
        },
        checksums::hash_file,
    )
    .await;

    // Check: The operation succeeded and nothing changed
    assert!(result.is_ok());
    assert!(
        src_file.exists() && fs::read_to_string(&src_file).unwrap() == "source".to_owned(),
        "src_file contents: {:?}",
        fs::read_to_string(&src_file).unwrap()
    );
    assert!(
        dst_file.exists() && fs::read_to_string(&dst_file).unwrap() == "".to_owned(),
        "dst_file contents: {:?}",
        fs::read_to_string(&dst_file).unwrap()
    );
}

/// Test move_file if there is a collision and the policy is set to overwrite.
#[tokio::test]
async fn test_move_file_collision_overwrite() {
    // Prerequisite: Setup environment
    let (_tmp_dir, src_file, dst_file) = setup_move_file();

    // Prerequisite: Create a file in the destination directory
    File::create(&dst_file).unwrap();

    // Action: Move file with collision policy set to overwrite
    let result = move_file(
        &src_file,
        &dst_file,
        &MoveStrategy {
            collision: CollisionPolicy::Overwrite,
            safe: false,
            check: None,
            retries: 5,
        },
        checksums::hash_file,
    )
    .await;

    // Check: The operation succeeded and the destination file was overwritten
    assert!(result.is_ok());
    assert!(!src_file.exists());
    assert!(
        dst_file.exists() && fs::read_to_string(&dst_file).unwrap() == "source".to_owned(),
        "dst_file contents: {:?}",
        fs::read_to_string(&dst_file).unwrap()
    );
}

/// Test move_file if there is a collision and the policy is set to rename.
#[tokio::test]
async fn test_move_file_collision_rename() {
    // Prerequisite: Setup environment
    let (_tmp_dir, src_file, dst_file) = setup_move_file();

    // Prerequisite: Create a file in the destination directory
    File::create(&dst_file).unwrap();

    // Action: Move file with collision policy set to rename
    let result = move_file(
        &src_file,
        &dst_file,
        &MoveStrategy {
            collision: CollisionPolicy::Rename {
                suffix: "test".to_owned(),
            },
            safe: false,
            check: None,
            retries: 5,
        },
        checksums::hash_file,
    )
    .await;

    // Check: The operation succeeded and the destination file was untouched but another file was created
    assert!(result.is_ok());
    assert!(!src_file.exists());
    assert!(
        dst_file.exists() && fs::read_to_string(&dst_file).unwrap() == "".to_owned(),
        "dst_file contents: {:?}",
        fs::read_to_string(&dst_file).unwrap()
    );
    let new_dst_file = {
        let mut new_dst = dst_file.to_path_buf();
        new_dst.set_extension("test");
        new_dst
    };
    assert!(
        new_dst_file.exists() && fs::read_to_string(&new_dst_file).unwrap() == "source".to_owned(),
        "new_dst_file contents: {:?}",
        fs::read_to_string(&new_dst_file).unwrap()
    );
}

/// Test move_file if the safe flag is set.
#[tokio::test]
async fn test_move_file_safe() {
    // Prerequisite: Setup environment
    let (_tmp_dir, src_file, dst_file) = setup_move_file();

    // Action: Move file with safe mode enabled
    let result = move_file(
        &src_file,
        &dst_file,
        &MoveStrategy {
            collision: CollisionPolicy::Fail,
            safe: true,
            check: None,
            retries: 5,
        },
        checksums::hash_file,
    )
    .await;

    // FIXME: Find a way to test that the WIP file was created

    // Check: The operation succeeded
    assert!(result.is_ok());
    assert!(!src_file.exists());
    assert!(
        dst_file.exists() && fs::read_to_string(&dst_file).unwrap() == "source".to_owned(),
        "dst_file contents: {:?}",
        fs::read_to_string(&dst_file).unwrap()
    );
}

static TEST_HASH_FILE_COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

fn test_hash_file_count(path: &Path, _algo: Algorithm) -> String {
    println!("Hashing: {:?}", path);
    TEST_HASH_FILE_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    "test".to_owned()
}

/// Test move_file if checksum is requested.
#[tokio::test]
async fn test_move_file_check() {
    // Prerequisite: Setup environment
    let (_tmp_dir, src_file, dst_file) = setup_move_file();

    // Prerequisite: Reset the number of calls to the hashing function
    TEST_HASH_FILE_COUNT.store(0, std::sync::atomic::Ordering::SeqCst);

    // Action: Move file with safe mode enabled
    let result = move_file(
        &src_file,
        &dst_file,
        &MoveStrategy {
            collision: CollisionPolicy::Fail,
            safe: false,
            check: Some(Algorithm::MD5),
            retries: 0,
        },
        test_hash_file_count,
    )
    .await;

    // Check: The hashing function was called
    assert!(
        TEST_HASH_FILE_COUNT.load(std::sync::atomic::Ordering::SeqCst) == 2,
        "Actual: {}. Expected: 2",
        TEST_HASH_FILE_COUNT.load(std::sync::atomic::Ordering::SeqCst)
    );

    // Check: The operation succeeded
    assert!(result.is_ok());
    assert!(!src_file.exists());
    assert!(
        dst_file.exists() && fs::read_to_string(&dst_file).unwrap() == "source".to_owned(),
        "dst_file contents: {:?}",
        fs::read_to_string(&dst_file).unwrap()
    );
}

static TEST_HASH_FILE_NASTY_RESULTS: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(0);

fn test_hash_file_nasty_results(_path: &Path, _algo: Algorithm) -> String {
    let result = TEST_HASH_FILE_NASTY_RESULTS
        .load(std::sync::atomic::Ordering::SeqCst)
        .to_string();
    TEST_HASH_FILE_NASTY_RESULTS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    result
}

/// Test move_file if the operation fails and retries are requested.
#[tokio::test]
async fn test_move_file_check_failed() {
    // Prerequisite: Setup environment
    let (_tmp_dir, src_file, dst_file) = setup_move_file();

    // Prerequisite: Reset the number of calls to the hashing function
    TEST_HASH_FILE_NASTY_RESULTS.store(0, std::sync::atomic::Ordering::SeqCst);

    // Action: Move file with safe mode enabled
    let result = move_file(
        &src_file,
        &dst_file,
        &MoveStrategy {
            collision: CollisionPolicy::Fail,
            safe: false,
            check: Some(Algorithm::MD5),
            retries: 5,
        },
        test_hash_file_nasty_results,
    )
    .await;

    // Check: The operation failed and nothing changed
    assert!(result.is_err() && result.unwrap_err().to_string().contains("maximum retries"));
    assert!(
        src_file.exists() && fs::read_to_string(&src_file).unwrap() == "source".to_owned(),
        "src_file contents: {:?}",
        fs::read_to_string(&src_file).unwrap()
    );
    assert!(!dst_file.exists());
}

//TODO: Check that after moving a file inside a folder and leaving the folder empty, the folder is removed
