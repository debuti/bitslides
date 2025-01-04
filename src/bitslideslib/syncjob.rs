use std::fmt::Debug;

/// SyncJob representation.
///
#[derive(PartialEq, Clone)]
pub struct SyncJob {
    /// Source volume
    pub src: String,
    /// Destination volume
    pub dst: String,
    /// Issue volume
    pub issue: String,
}

/// SyncJob Debug implementation.
///
impl Debug for SyncJob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} -{}-> {}", self.src, self.issue, self.dst)
    }
}

// FIXME: Move to a owned type (struct {inner: Vec<SyncJob>}) and impl iterator on it. Also provide a sort
// method to sort the syncjobs by sync order
pub type SyncJobs = Vec<SyncJob>;
