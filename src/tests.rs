use super::*;
use std::fs::File;
use std::io::Write;

struct TestContext {
    temp_dir: tempfile::TempDir,
    roots: [PathBuf; 2],
}

fn setup() -> Result<TestContext> {
    let temp_dir = tempfile::tempdir()?;

    let roots = [temp_dir.path().join("root0"), temp_dir.path().join("root1")];

    //TODO: Add .slide.yml files to some places

    /* Create some folders that are not a volume */
    {
        {
            let volume_dir = roots[0].join("dude");
            let slides_dir = volume_dir.join("not_a_keyword");
            std::fs::create_dir_all(&slides_dir)?;
        }

        {
            let volume_dir = roots[0].join("edud");
            std::fs::create_dir_all(&volume_dir)?;
        }
    }

    for volume in ["foo", "bar"] {
        let volume_dir = roots[0].join(volume);
        let slides_dir = volume_dir.join("slides");
        for slide in ["foo", "bar", "baz"] {
            std::fs::create_dir_all(slides_dir.join(slide))?;
        }
        /* Create some file in the slides folder */
        File::create(slides_dir.join("not_a_slide"))?;
    }

    /* Add some broken slide config files */
    {
        File::create(
            roots[0].join(
                ["foo", "slides", "bar", DEFAULT_SLIDE_CONFIG_FILE]
                    .iter()
                    .collect::<PathBuf>(),
            ),
        )?
        .write("buffer: 1".as_bytes())?;
        File::create(
            roots[0].join(
                ["foo", "slides", "baz", DEFAULT_SLIDE_CONFIG_FILE]
                    .iter()
                    .collect::<PathBuf>(),
            ),
        )?
        .write("route:".as_bytes())?;
    }

    for volume in ["baz", "els"] {
        let volume_dir = roots[1].join(volume);
        let slides_dir = volume_dir.join("slides");
        for slide in ["foo", "bar", "baz", "qux"] {
            std::fs::create_dir_all(slides_dir.join(slide))?;
        }
    }

    /* Add a correct default route */
    {
        /* Maaningful since the volume is not available */
        File::create(
            roots[1].join(
                ["baz", "slides", "qux", DEFAULT_SLIDE_CONFIG_FILE]
                    .iter()
                    .collect::<PathBuf>(),
            ),
        )?
        .write("route: bar".as_bytes())?;
        /* Maaningless since the volume is available */
        File::create(
            roots[1].join(
                ["baz", "slides", "foo", DEFAULT_SLIDE_CONFIG_FILE]
                    .iter()
                    .collect::<PathBuf>(),
            ),
        )?
        .write("route: bar".as_bytes())?;
    }

    Ok(TestContext { temp_dir, roots })
}

#[test]
fn test_identify_volumes() {
    let ctx = setup().unwrap();

    let volumes = identify_volumes(&ctx.roots[0], "slides").unwrap();

    assert_eq!(volumes.len(), 2);
    for volume in ["foo", "bar"] {
        assert!(volumes.contains_key(volume) && volumes[volume].path.exists());
    }
}

#[test]
fn test_identify_slides() {
    let ctx = setup().unwrap();

    let mut volume = Volume::new("foo".to_string(), ctx.roots[0].join("foo"));

    identify_slides(&mut volume, "slides").unwrap();

    assert_eq!(volume.slides.len(), 3);
    for slide in ["foo", "bar", "baz"] {
        assert!(volume.slides.contains_key(slide) && volume.slides[slide].path.exists());
    }
}

#[test]
fn test_process_config() {
    let ctx = setup().unwrap();

    let volumes: HashMap<String, Volume> = process_config("slides", &ctx.roots).unwrap();

    assert_eq!(volumes.len(), 4);
    for volume in ["foo", "bar"] {
        assert!(volumes.contains_key(volume));
        assert_eq!(volumes[volume].slides.len(), 3);
        for slide in ["foo", "bar", "baz"] {
            assert!(volumes[volume].slides.contains_key(slide));
        }
    }
    assert!(volumes["foo"].slides["bar"].or_else.is_none());
    assert!(volumes["foo"].slides["baz"].or_else.is_none());

    for volume in ["baz", "els"] {
        assert!(volumes.contains_key(volume));
        assert_eq!(volumes[volume].slides.len(), 4);
        for slide in ["foo", "bar", "baz", "qux"] {
            assert!(volumes[volume].slides.contains_key(slide));
        }
    }
    assert!(volumes["baz"].slides["qux"].or_else.is_some());
    assert!(volumes["baz"].slides["foo"].or_else.is_some());
}

#[rustfmt::skip]
#[test]
fn test_build_syncjobs() {
    let ctx = setup().unwrap();

    let volumes: HashMap<String, Volume> = process_config("slides", &ctx.roots).unwrap();
    let syncjobs = build_syncjobs(&volumes);

    let expected_syncjobs =[
        SyncJob {src: "foo", dst: "bar", issue: "bar", },
        SyncJob {src: "foo", dst: "baz", issue: "baz", },
        SyncJob {src: "bar", dst: "foo", issue: "foo", },
        SyncJob {src: "bar", dst: "baz", issue: "baz", },
        SyncJob {src: "baz", dst: "foo", issue: "foo", },
        SyncJob {src: "baz", dst: "bar", issue: "bar", },
        SyncJob {src: "els", dst: "foo", issue: "foo", },
        SyncJob {src: "els", dst: "bar", issue: "bar", },
        SyncJob {src: "els", dst: "baz", issue: "baz", },
        // Indirect syncjobs
        SyncJob {src: "baz", dst: "qux", issue: "bar", },
    ];
    assert_eq!(syncjobs.len(), expected_syncjobs.len());
    for expected_syncjob in expected_syncjobs {
        assert!(syncjobs.contains(&expected_syncjob), "Missing {:?}", expected_syncjob);
    }
}
