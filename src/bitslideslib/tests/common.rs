use log::LevelFilter;
use simplelog::{ColorChoice, Config, TermLogger, TerminalMode};
use std::fs::File;
use std::io::Result;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use super::DEFAULT_SLIDE_CONFIG_FILE;

/// A structure representing the context for tests, which includes a temporary directory
/// and a collection of root paths.
pub struct TestContext {
    #[allow(dead_code)]
    /// A temporary directory used during the test. This field is marked with `#[allow(dead_code)]` to suppress warnings about it not being used.
    pub temp_dir: tempfile::TempDir,
    /// A vector of `PathBuf` representing the root paths used in the test.
    pub roots: Vec<PathBuf>,
}

/// A structure representing a test folder used in unit tests.
///
/// This structure contains a list of subfolders and files, each represented
/// as a tuple with the folder or file name and its contents.
struct TestFolder {
    /// A static slice of tuples where each tuple contains the name of the subfolder and a `TestFolder` instance representing the subfolder.
    folders: &'static [(&'static str, TestFolder)],
    /// A static slice of tuples where each tuple contains the name of the file and a static byte slice representing the file's contents.
    files: &'static [(&'static str, &'static [u8])],
}

impl std::fmt::Display for TestFolder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt_with_indent(f, 0)
    }
}

impl TestFolder {
    /// Helper function to format with an indentation level.
    fn fmt_with_indent(&self, f: &mut std::fmt::Formatter<'_>, indent: usize) -> std::fmt::Result {
        let indent_str = " ".repeat(indent);
        for (file_name, file_contents) in self.files {
            writeln!(
                f,
                "{}- {} ({} b)",
                indent_str,
                file_name,
                file_contents.len()
            )?;
        }
        for (folder_name, folder) in self.folders {
            writeln!(f, "{}- {}", indent_str, folder_name)?;
            folder.fmt_with_indent(f, indent + 2)?;
        }
        Ok(())
    }
}

/// Installs a test scenario into a temporary directory and returns a `TestContext` containing the
/// temporary directory and the root paths of the installed folders.
///
/// # Arguments
///
/// * `scenario` - A reference to a `TestFolder` structure that defines the folder and file
///   hierarchy to be installed.
/// * `tempdir` - A `tempfile::TempDir` instance representing the temporary directory where the
///   scenario will be installed.
///
/// # Returns
///
/// A `Result` containing a `TestContext` with the temporary directory and the root paths of the
/// installed folders, or an `io::Result` error if any file system operation fails.
///
/// # Errors
///
/// This function will return an error if any file system operation (such as creating directories
/// or writing files) fails.
///
/// # Example
///
/// ```rust
/// let tempdir = tempfile::tempdir()?;
/// let scenario = TestFolder {
///     folders: &[
///         ("example", TestFolder {
///             folders: &[],
///             files: &[("file.txt", b"content")],
///         }),
///     ],
///     files: &[],
/// };
/// let context = install_scenario(&scenario, tempdir)?;
/// ```
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
    let _ = TermLogger::init(
        LevelFilter::Trace,
        Config::default(),
        TerminalMode::Stderr,
        ColorChoice::Auto,
    );

    const FOO: TestFolder = TestFolder {
        folders: &[
            (
                "not_a_slides_folder",
                TestFolder {
                    folders: &[],
                    files: &[],
                },
            ),
            (
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
            ),
        ],
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
                            folders: &[(
                                "photos",
                                TestFolder {
                                    folders: &[],
                                    files: &[("photo1.jpg", &[0xffu8; 16 * 1024])],
                                },
                            )],
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
                    (
                        "quux",
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
                    (
                        "quux",
                        TestFolder {
                            folders: &[],
                            files: &[(
                                DEFAULT_SLIDE_CONFIG_FILE,
                                "route: not-found-lol".as_bytes(),
                            )],
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

    // Print the scenario for debugging purposes
    println!("{}", SCENARIO);

    install_scenario(&SCENARIO, tempdir)
}
