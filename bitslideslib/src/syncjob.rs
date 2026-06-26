use std::hash::Hash;
use std::{collections::HashSet, fmt::Debug};

use std::path::PathBuf;
use tokio::sync::mpsc;

use anyhow::{anyhow, Result};

use crate::Volumes;

/// [`SyncJob`] representation.
///
/// A syncjob defines a source and a final destination, optionally passing via another volume.
/// Although it is optional, the value has to be provided to help the algorithm
///
#[derive(PartialEq, Eq, Hash)]
pub struct SyncJobMeta {
    /// Source volume
    pub(crate) src: String,
    /// Proxy volume (intermediate staging volume).
    ///
    /// To indicate that no separate proxy is desired and that the sync should be
    /// treated as a direct `src -> dst` operation, set this equal to `dst`.
    pub(crate) via: String,
    /// Destination volume
    pub(crate) dst: String,
}

impl SyncJobMeta {
    /// Creates a new [`SyncJobMeta`] with the given source, proxy and destination volumes.
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
    ///
    pub(crate) fn new(src: &str, via: &str, dst: &str) -> Self {
        Self {
            src: src.to_string(),
            via: via.to_string(),
            dst: dst.to_string(),
        }
    }

    pub(crate) fn activate(self, volumes: &Volumes) -> Result<SyncJob> {
        // Calculate the syncjob source path
        let src = volumes
            .get(&self.src)
            .and_then(|v| v.slides.get(&self.dst))
            .map(|s| s.path.clone())
            .ok_or_else(|| anyhow!("Invalid syncjob src path: {:?}", &self.src))?;

        // Calculate the syncjob destination path
        let dst = volumes
            .get(&self.via)
            .and_then(|v| v.slides.get(&self.dst))
            .map(|s| s.path.clone())
            .ok_or_else(|| anyhow!("Invalid syncjob dst path: {:?}", &self.dst))?;

        let (tx, rx) = mpsc::channel(crate::config::SYNCJOB_CHANNEL_CAPACITY);

        Ok(SyncJob {
            meta: self,
            src,
            dst,
            inner: SyncJobInner { tx: Some(tx), rx },
        })
    }
}

/// [`SyncJobMeta`] [`Debug`] implementation.
///
impl Debug for SyncJobMeta {
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

/// Internal structure holding the synchronization trigger channel.
///
/// There is one sender and one receiver per [`SyncJob`]. The sender is used to trigger
/// synchronization events from the notification system, while the receiver listens for these triggers.
///
struct SyncJobInner {
    tx: Option<tokio::sync::mpsc::Sender<()>>,
    rx: tokio::sync::mpsc::Receiver<()>,
}

/// [`SyncJobInner`] [`Debug`] implementation.
///
impl Debug for SyncJobInner {
    fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Ok(())
    }
}

/// [`SyncJobInner`] [`PartialEq`] implementation.
///
impl PartialEq for SyncJobInner {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

/// [`SyncJobInner`] [`Eq`] implementation.
///
impl Eq for SyncJobInner {}

/// [`SyncJobInner`] [`Hash`] implementation.
///
impl Hash for SyncJobInner {
    fn hash<H: std::hash::Hasher>(&self, _: &mut H) {}
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct SyncJob {
    meta: SyncJobMeta,
    pub src: PathBuf,
    pub dst: PathBuf,
    /// Implementation details
    inner: SyncJobInner,
}

impl SyncJob {
    /// Takes the trigger sender from the sync job.
    ///
    /// This method consumes the sender, allowing external components to trigger synchronization events.
    ///
    /// # Returns
    ///
    /// An `Option` containing the `Sender<()>` if it was available, or `None` if it has already been taken.
    ///
    pub(crate) const fn take_trigger(&mut self) -> Option<tokio::sync::mpsc::Sender<()>> {
        self.inner.tx.take()
    }

    /// Borrows a mutable reference to the receiver.
    ///
    /// This allows external components to listen for synchronization triggers.
    ///
    /// # Returns
    ///
    /// A mutable reference to the `Receiver<()>`.
    ///
    pub(crate) const fn borrow_receiver(&mut self) -> &mut tokio::sync::mpsc::Receiver<()> {
        &mut self.inner.rx
    }
}

// FIXME: impl iterator on this. Also provide a sort
// method to sort the syncjobs by sync order
#[derive(Debug)]
pub struct SyncJobs(HashSet<SyncJob>);

impl SyncJobs {
    pub fn new() -> Self {
        Self(HashSet::new())
    }

    pub fn insert(&mut self, syncjob: SyncJob) {
        self.0.insert(syncjob);
    }

    // pub fn len(&self) -> usize {
    //     self.0.len()
    // }

    // pub fn contains(&self, syncjob: &SyncJob) -> bool {
    //     self.0.contains(syncjob)
    // }
}

impl IntoIterator for SyncJobs {
    type Item = SyncJob;
    type IntoIter = std::collections::hash_set::IntoIter<SyncJob>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}
