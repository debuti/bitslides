use std::path::PathBuf;

/// Slide representation.
///
#[derive(Debug, PartialEq)]
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
    pub const fn new(name: String, path: PathBuf, or_else: Option<String>) -> Self {
        Self {
            name,
            path,
            or_else,
        }
    }
}

#[cfg(false)]
impl std::fmt::Display for Slide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.or_else.is_none() {
            write!(f, "{}", self.name,)
        } else {
            write!(f, "{} (->{})", self.name, self.or_else.as_ref().unwrap())
        }
    }
}

impl crate::named_collection::Named for Slide {
    fn name(&self) -> &str {
        &self.name
    }
}


#[derive(Debug, PartialEq)]
pub struct Slides(crate::named_collection::NamedCollection<Slide>);

impl Slides {
    pub fn new() -> Self {
        Self(crate::named_collection::NamedCollection::new())
    }
}

impl std::ops::Deref for Slides {
    type Target = crate::named_collection::NamedCollection<Slide>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for Slides {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl IntoIterator for Slides {
    type Item = Slide;
    type IntoIter = std::collections::hash_map::IntoValues<String, Slide>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a Slides {
    type Item = &'a Slide;
    type IntoIter = std::collections::hash_map::Values<'a, String, Slide>;

    fn into_iter(self) -> Self::IntoIter {
        (&self.0).into_iter()
    }
}

impl<'a> IntoIterator for &'a mut Slides {
    type Item = &'a mut Slide;
    type IntoIter = std::collections::hash_map::ValuesMut<'a, String, Slide>;

    fn into_iter(self) -> Self::IntoIter {
        (&mut self.0).into_iter()
    }
}
