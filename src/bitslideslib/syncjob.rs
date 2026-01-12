use std::fmt::Debug;

use tokio::sync::mpsc;

/// SyncJob representation.
///
/// A syncjob defines a source and a final destination, optionally passing via another volume.
/// Althought it is optional, the value has to be provided to help the algorithm
///
pub struct SyncJob {
    /// Source volume
    pub src: String,
    /// Proxy volume
    pub via: String,
    /// Destination volume
    pub dst: String,
    ///
    pub inner: SyncJobInner,
}

impl SyncJob {
    ///
    pub fn new(src: &str, via: &str, dst: &str) -> Self {
        let (tx, rx) = mpsc::channel(1000);
        Self {
            src: src.to_string(),
            via: via.to_string(),
            dst: dst.to_string(),
            inner: SyncJobInner { tx: Some(tx), rx: rx },
        }
    }

    pub fn take_trigger(&mut self) -> Option<tokio::sync::mpsc::Sender<()>> {
        self.inner.tx.take()
    }
}

/// SyncJob Debug implementation.
///
impl Debug for SyncJob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} -{}-> {}",
            self.src,
            if self.via == self.dst { "_" } else { &self.via },
            self.dst
        )
    }
}

impl PartialEq for SyncJob {
    fn eq(&self, other: &Self) -> bool {
        self.src == other.src && self.via == other.via && self.dst == other.dst
    }
}

// FIXME: Move to a owned type (struct {inner: Vec<SyncJob>}) and impl iterator on it. Also provide a sort
// method to sort the syncjobs by sync order
pub type SyncJobs = Vec<SyncJob>;

pub struct SyncJobInner {
    pub tx: Option<tokio::sync::mpsc::Sender<()>>,
    pub rx: tokio::sync::mpsc::Receiver<()>,
}
