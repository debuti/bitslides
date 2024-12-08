use std::fs::File;
use std::io::Result;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use crate::DEFAULT_SLIDE_CONFIG_FILE;

pub struct TestContext {
    #[allow(dead_code)]
    pub temp_dir: tempfile::TempDir,
    pub roots: Vec<PathBuf>,
}

struct TestFolder {
    folders: &'static [(&'static str, TestFolder)],
    files: &'static [(&'static str, &'static [u8])],
}

fn install_scenario(scenario: &TestFolder, tempdir: tempfile::TempDir) -> Result<TestContext> {
    fn install_folder(folder: &TestFolder, parent: &Path) -> Result<()> {
        std::fs::create_dir_all(&parent)?;
        for (folder_name, folder) in folder.folders {
            install_folder(folder, &parent.join(folder_name))?;
        }
        for (file_name, file_contents) in folder.files {
            let mut file = File::create(parent.join(&file_name))?;
            file.write_all(&file_contents)?;
        }
        Ok(())
    }

    let mut roots = vec![];
    for (folder_name, folder) in scenario.folders {
        let root = tempdir.path().join(folder_name);
        install_folder(folder, &root)?;
        roots.push(root);
    }

    Ok(TestContext {
        temp_dir: tempdir,
        roots: roots,
    })
}

pub(crate) fn setup() -> Result<TestContext> {
    const FOO: TestFolder = TestFolder {
        folders: &[(
            "slides",
            TestFolder {
                folders: &[
                    (
                        "foo",
                        TestFolder {
                            folders: &[],
                            files: &[],
                        },
                    ),
                    (
                        "bar",
                        TestFolder {
                            folders: &[(
                                "media",
                                TestFolder {
                                    folders: &[],
                                    files: &[("bigfile", &[0u8; 1024 * 1024 * 10])],
                                },
                            )],
                            files: &[
                                // Broken slide config file
                                (DEFAULT_SLIDE_CONFIG_FILE, "buffer: 1".as_bytes()),
                            ],
                        },
                    ),
                    (
                        "baz",
                        TestFolder {
                            folders: &[],
                            files: &[
                                // Broken slide config file
                                (DEFAULT_SLIDE_CONFIG_FILE, "route:".as_bytes()),
                            ],
                        },
                    ),
                ],
                files: &[("not_a_slide", "abcdef".as_bytes())],
            },
        )],
        files: &[("not_a_slide", "abcdef".as_bytes())],
    };
    const BAR: TestFolder = TestFolder {
        folders: &[(
            "slides",
            TestFolder {
                folders: &[
                    (
                        "foo",
                        TestFolder {
                            folders: &[],
                            files: &[],
                        },
                    ),
                    (
                        "bar",
                        TestFolder {
                            folders: &[],
                            files: &[],
                        },
                    ),
                    (
                        "baz",
                        TestFolder {
                            folders: &[],
                            files: &[],
                        },
                    ),
                ],
                files: &[("not_a_slide", "abcdef".as_bytes())],
            },
        )],
        files: &[("not_a_slide", "abcdef".as_bytes())],
    };
    const BAZ: TestFolder = TestFolder {
        folders: &[(
            "slides",
            TestFolder {
                folders: &[
                    (
                        "foo",
                        TestFolder {
                            folders: &[],
                            files: &[(DEFAULT_SLIDE_CONFIG_FILE, "route: bar".as_bytes())],
                        },
                    ),
                    (
                        "bar",
                        TestFolder {
                            folders: &[],
                            files: &[],
                        },
                    ),
                    (
                        "baz",
                        TestFolder {
                            folders: &[],
                            files: &[],
                        },
                    ),
                    (
                        "qux",
                        TestFolder {
                            folders: &[],
                            files: &[(DEFAULT_SLIDE_CONFIG_FILE, "route: bar".as_bytes())],
                        },
                    ),
                ],
                files: &[],
            },
        )],
        files: &[],
    };
    const ELS: TestFolder = TestFolder {
        folders: &[(
            "slides",
            TestFolder {
                folders: &[
                    (
                        "foo",
                        TestFolder {
                            folders: &[],
                            files: &[],
                        },
                    ),
                    (
                        "bar",
                        TestFolder {
                            folders: &[],
                            files: &[],
                        },
                    ),
                    (
                        "baz",
                        TestFolder {
                            folders: &[],
                            files: &[],
                        },
                    ),
                    (
                        "qux",
                        TestFolder {
                            folders: &[],
                            files: &[],
                        },
                    ),
                ],
                files: &[],
            },
        )],
        files: &[],
    };

    const ROOT0: TestFolder = TestFolder {
        folders: &[
            ("foo", FOO),
            ("bar", BAR),
            (
                // Not a volume
                "dude",
                TestFolder {
                    folders: &[(
                        "not_a_keyword",
                        TestFolder {
                            folders: &[],
                            files: &[],
                        },
                    )],
                    files: &[],
                },
            ),
            (
                // Not a volume
                "edud",
                TestFolder {
                    folders: &[],
                    files: &[],
                },
            ),
        ],
        files: &[(
            "this-is-a-random-file",
            "definetely-not-a-volume".as_bytes(),
        )],
    };
    const ROOT1: TestFolder = TestFolder {
        folders: &[("baz", BAZ), ("els", ELS)],
        files: &[],
    };

    const SCENARIO: TestFolder = TestFolder {
        folders: &[("root0", ROOT0), ("root1", ROOT1)],
        files: &[],
    };
    let tempdir = tempfile::tempdir()?;

    install_scenario(&SCENARIO, tempdir)
}
