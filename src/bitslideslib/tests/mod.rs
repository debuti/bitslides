mod common;

use crate::CollisionPolicy;

use super::*;
use checksums::{hash_file, Algorithm};
use pretty_assertions::assert_eq;

use common::setup;

/// Test the identification of volumes inside a root folder
#[test]
fn test_identify_volumes() {
    // Prerequisite: Setup the test context
    let ctx = setup().unwrap();

    // Action: Call identify_volumes operation with the 1st root folder and the keyword "slides"
    let volumes = identify_volumes(&ctx.roots[0], "slides").unwrap();

    // Check: The result should contain 2 volumes
    assert_eq!(volumes.len(), 2);

    // Check: The result should contain the volumes "foo" and "bar"
    for volume in ["foo", "bar"] {
        assert!(volumes.contains_key(volume) && volumes[volume].path.exists());
    }
}

/// Test the identification of slides inside a volume
#[test]
fn test_identify_slides() {
    // Prerequisite: Setup the test context
    let ctx = setup().unwrap();

    // Prerequisite: Create a volume object for the "foo" volume
    let mut volume = Volume::new("foo".to_string(), "slides", ctx.roots[0].join("foo"));

    // Action: Call identify_slides operation with the volume object
    identify_slides(&mut volume).unwrap();

    // Check: The volume should contain 3 slides
    assert_eq!(volume.slides.len(), 3);

    // Check: The volume should contain the slides "foo", "bar" and "baz"
    for slide in ["foo", "bar", "baz"] {
        assert!(volume.slides.contains_key(slide) && volume.slides[slide].path.exists());
    }
}

/// Test the identification of volumes and slides inside a set of root folders
#[test]
fn test_identify_env() {
    // Prerequisite: Setup the test context
    let ctx = setup().unwrap();

    // Action: Call identify_env operation with the keyword "slides" and the root folders
    let volumes: HashMap<String, Volume> = identify_env("slides", &ctx.roots).unwrap();

    // Check: The result should contain 4 volumes
    assert_eq!(volumes.len(), 4);

    // Check: The result should contain the volumes "foo" and "bar"
    for volume in ["foo", "bar"] {
        assert!(volumes.contains_key(volume));

        // Check: The volume should contain 3 slides
        assert_eq!(volumes[volume].slides.len(), 3);

        // Check: The volume should contain the slides "foo", "bar" and "baz"
        for slide in ["foo", "bar", "baz"] {
            assert!(volumes[volume].slides.contains_key(slide));
        }
    }
    assert!(volumes["foo"].slides["bar"].or_else.is_none());
    assert!(volumes["foo"].slides["baz"].or_else.is_none());

    // Check: The result should contain the volumes "baz" and "els"
    for volume in ["baz", "els"] {
        assert!(volumes.contains_key(volume));

        // Check: The volume should contain 4 slides
        assert_eq!(volumes[volume].slides.len(), 4);

        // Check: The volume should contain the slides "foo", "bar", "baz" and "qux"
        for slide in ["foo", "bar", "baz", "qux"] {
            assert!(volumes[volume].slides.contains_key(slide));
        }
    }
    assert!(volumes["baz"].slides["qux"].or_else.is_some());
    assert!(volumes["baz"].slides["foo"].or_else.is_some());
}

/// Test the building of sync jobs between volumes
#[rustfmt::skip]
#[test]
fn test_build_syncjobs() {
    // Prerequisite: Setup the test context
    let ctx = setup().unwrap();

    // Prerequisite: Identify the volumes in the root folders
    let mut volumes: HashMap<String, Volume> = identify_env("slides", &ctx.roots).unwrap();

    // Action: Call build_syncjobs operation with the identified volumes
    let syncjobs = build_syncjobs(&mut volumes).unwrap();

    let expected_syncjobs =[
        SyncJob {src: "foo".to_owned(), dst: "bar".to_owned(), issue: "bar".to_owned(), },
        SyncJob {src: "foo".to_owned(), dst: "baz".to_owned(), issue: "baz".to_owned(), },
        SyncJob {src: "bar".to_owned(), dst: "foo".to_owned(), issue: "foo".to_owned(), },
        SyncJob {src: "bar".to_owned(), dst: "baz".to_owned(), issue: "baz".to_owned(), },
        SyncJob {src: "baz".to_owned(), dst: "foo".to_owned(), issue: "foo".to_owned(), },
        SyncJob {src: "baz".to_owned(), dst: "bar".to_owned(), issue: "bar".to_owned(), },
        SyncJob {src: "els".to_owned(), dst: "foo".to_owned(), issue: "foo".to_owned(), },
        SyncJob {src: "els".to_owned(), dst: "bar".to_owned(), issue: "bar".to_owned(), },
        SyncJob {src: "els".to_owned(), dst: "baz".to_owned(), issue: "baz".to_owned(), },
        // Indirect syncjobs
        SyncJob {src: "baz".to_owned(), dst: "bar".to_owned(), issue: "qux".to_owned(), },
    ];

    // Check: The result should match the length and content of the expected sync jobs
    assert_eq!(syncjobs.len(), expected_syncjobs.len());
    for expected_syncjob in expected_syncjobs {
        assert!(syncjobs.contains(&expected_syncjob), "Missing {:?}", expected_syncjob);
    }
}

/// Test the execution of sync jobs between volumes
#[tokio::test]
async fn test_execute_syncjobs() {
    // Prerequisite: Setup the test context
    let ctx = setup().unwrap();

    // Prerequisite: Identify the volumes in the root folders
    let mut volumes: HashMap<String, Volume> = identify_env("slides", &ctx.roots).unwrap();

    // Prerequisite: Build the sync jobs between the volumes
    let syncjobs = build_syncjobs(&mut volumes).unwrap();

    // Action: Execute the sync jobs
    {
        let move_req = MoveRequest {
            collision: CollisionPolicy::Fail,
            safe: false,
            check: None,
            retries: 5,
        };
        execute_syncjobs(&volumes, &syncjobs, false, &move_req)
            .await
            .unwrap();
    }

    // Check: The slides should be synchronized correctly
    {
        // Check: The source slide should not have any contents
        {
            let src = &volumes["foo"].slides["bar"].path;
            let file = src.join("media").join("bigfile");
            assert!(!file.exists());
        }

        // Check: The destination slide should have the contents of the source slide
        {
            let dst = &volumes["bar"].slides["bar"].path;
            let file = dst.join("media").join("bigfile");
            let expected = "F1C9645DBC14EFDDC7D8A322685F26EB";
            assert!(
                file.exists()
                    && file.is_file()
                    && file.metadata().unwrap().len() == 1024 * 1024 * 10
                    && hash_file(&file, Algorithm::MD5) == expected,
                "bigfile checksum: {} expected: {}",
                hash_file(&file, Algorithm::MD5),
                expected
            );
        }

        // Check: The source slide should not have any contents
        {
            let src = &volumes["bar"].slides["foo"].path;
            let file = src.join("photos").join("photo1.jpg");
            assert!(!file.exists());
        }

        // Check: The destination slide should have the contents of the source slide
        {
            let dst = &volumes["foo"].slides["foo"].path;
            let file = dst.join("photos").join("photo1.jpg");
            let expected = "92AB673D915A94DCF187720E8AC0D608";
            assert!(
                file.exists()
                    && file.is_file()
                    && file.metadata().unwrap().len() == 1024 * 16
                    && hash_file(&file, Algorithm::MD5) == expected,
                "bigfile checksum: {} expected: {}",
                hash_file(&file, Algorithm::MD5),
                expected
            );
        }
    }
}

/// Test the execution of sync jobs between volumes with a missing source (i.e. The user deleted a source slide)
#[tokio::test]
#[ignore]
async fn test_execute_syncjobs_with_missing_source() {
    // Prerequisite: Setup the test context
    let ctx = setup().unwrap();

    // Prerequisite: Identify the volumes in the root folders
    let mut volumes = identify_env("slides", &ctx.roots).unwrap();

    // Prerequisite: Build the sync jobs between the volumes
    let syncjobs = build_syncjobs(&mut volumes).unwrap();

    // Remove a source slide to simulate a missing source
    {
        let missing_slide = volumes
            .get_mut("foo")
            .unwrap()
            .slides
            .remove("bar")
            .unwrap();
        std::fs::remove_dir_all(missing_slide.path).unwrap();
    }

    // Execute the sync jobs
    let result = {
        let move_req = MoveRequest {
            collision: CollisionPolicy::Fail,
            safe: false,
            check: None,
            retries: 5,
        };
        execute_syncjobs(&volumes, &syncjobs, false, &move_req).await
    };

    // Verify that the sync jobs failed due to the missing source
    assert!(
        result.is_err(),
        "Expected error due to missing source slide"
    );
}
