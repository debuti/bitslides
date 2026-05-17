use std::path::PathBuf;

/// Set of roots
///
/// This configuration is used to define a set of root paths that will contain volumes, along with the keyword each root will use.
///
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct RootsetConfig {
    /// Keyword to use for this rootset
    pub keyword: String,
    /// List of root absolute paths that will contain volumes
    pub roots: Vec<PathBuf>,
}
