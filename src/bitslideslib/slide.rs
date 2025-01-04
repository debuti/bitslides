use std::path::PathBuf;

/// Slide representation.
///
#[derive(Debug)]
pub struct Slide {
    /// Name of the destination volume
    pub name: String,
    /// Path to the slide. Ex. /path/to/volumes/foo/slides/bar
    pub path: PathBuf,
    /// Name of the default route towards the destination volume
    pub or_else: Option<String>,
}

/// Slide implementation.
///
impl Slide {
    /// Create a new slide.
    ///
    pub fn new(name: String, path: PathBuf, or_else: Option<String>) -> Self {
        Self {
            name,
            path,
            or_else,
        }
    }
}

#[cfg(any())]
impl std::fmt::Display for Slide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.or_else.is_none() {
            write!(f, "{}", self.name,)
        } else {
            write!(f, "{} (->{})", self.name, self.or_else.as_ref().unwrap())
        }
    }
}
