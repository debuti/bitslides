#[derive(Debug, PartialEq)]
pub struct SyncJob {
    pub src: String,
    pub dst: String,
    pub issue: String,
}

// FIXME: Move to a owned type (struct {inner: Vec<SyncJob>}) and impl iterator on it. Also provide a sort
// method to sort the syncjobs by sync order
pub type SyncJobs = Vec<SyncJob>; 