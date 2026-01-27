use std::fmt::Debug;

use tokio::sync::mpsc;

/// SyncJob representation.
///
/// A syncjob defines a source and a final destination, optionally passing via another volume.
/// Although it is optional, the value has to be provided to help the algorithm
///
pub(crate) struct SyncJob {
    /// Source volume
    pub(crate) src: String,
    /// Proxy volume (intermediate staging volume).
    ///
    /// To indicate that no separate proxy is desired and that the sync should be
    /// treated as a direct `src -> dst` operation, set this equal to `dst`.
    pub(crate) via: String,
    /// Destination volume
    pub(crate) dst: String,
    /// Implementation details
    inner: SyncJobInner,
}

/// Internal structure holding the synchronization trigger channel.
///
/// There is one sender and one receiver per SyncJob. The sender is used to trigger
/// synchronization events from the notification system, while the receiver listens for these triggers.
///
struct SyncJobInner {
    tx: Option<tokio::sync::mpsc::Sender<()>>,
    rx: tokio::sync::mpsc::Receiver<()>,
}

impl SyncJob {
    /// Creates a new [`SyncJob`] with the given source, proxy and destination volumes.
    ///
    /// # Parameters
    ///
    /// - `src`: The source volume from which data will be synchronized.
    /// - `via`: The intermediate (proxy) volume used during synchronization.
    /// - `dst`: The final destination volume to which data will be synchronized.
    ///
    /// # Returns
    ///
    /// A [`SyncJob`] instance initialized with the provided volumes and an
    /// internal trigger channel used to coordinate synchronization.
    pub(crate) fn new(src: &str, via: &str, dst: &str) -> Self {
        let (tx, rx) = mpsc::channel(1);
        Self {
            src: src.to_string(),
            via: via.to_string(),
            dst: dst.to_string(),
            inner: SyncJobInner { tx: Some(tx), rx },
        }
    }

    /// Takes the trigger sender from the sync job.
    ///
    /// This method consumes the sender, allowing external components to trigger synchronization events.
    ///
    /// # Returns
    ///
    /// An `Option` containing the `Sender<()>` if it was available, or `None` if it has already been taken.
    pub(crate) fn take_trigger(&mut self) -> Option<tokio::sync::mpsc::Sender<()>> {
        self.inner.tx.take()
    }

    /// Borrows a mutable reference to the receiver.
    ///
    /// This allows external components to listen for synchronization triggers.
    ///
    /// # Returns
    ///
    /// A mutable reference to the `Receiver<()>`.
    pub(crate) fn borrow_receiver(&mut self) -> &mut tokio::sync::mpsc::Receiver<()> {
        &mut self.inner.rx
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
