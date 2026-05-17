use super::CollisionPolicy;
use super::Rootset;
use std::path::PathBuf;

pub use checksums::Algorithm;

/// Global configuration
///
/// This configuration is used to define the global settings of the library.
///
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct GlobalConfig {
    /// List of rootset configurations
    pub rootsets: Vec<Rootset>,
    /// If true, do not perform any filesystem operation
    pub dry_run: bool,
    /// If provided, the path to a file where to write the trace
    pub trace: Option<PathBuf>,
    /// If provided, the algorithm to use for checksumming
    pub check: Option<Algorithm>,
    /// What to do in case of a file collision
    pub collision: CollisionPolicy,
    /// If true, enable a secure algorithm for moving files
    pub safe: bool,
    /// Number of retries in case of a failure (checksum mismatch, etc)
    pub retries: u8,
}
