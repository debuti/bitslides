use std::path::PathBuf;

use anyhow::Result;
use chrono::Local;
use tokio::{
    fs::OpenOptions,
    io::AsyncWriteExt,
    sync::mpsc::{self, Sender},
    task::JoinHandle,
};

use crate::bitslideslib::syncjob::SyncJob;

/// Tracer abstraction
///
/// The tracer is a logging utility that asynchronously writes trace messages to a file.
/// It uses a channel-based approach to avoid blocking the main execution flow when writing logs.
///
pub struct Tracer {
    tx: Option<Sender<String>>,
    syncjob_str: Option<String>,
}

impl Tracer {
    const CHANNEL_SIZE: usize = 32;

    pub async fn new(path: &Option<&PathBuf>) -> Result<(Self, Option<JoinHandle<()>>)> {
        match path {
            Some(trace_path) => {
                let mut file = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(trace_path)
                    .await?;

                let (tx, mut rx) = mpsc::channel::<String>(Self::CHANNEL_SIZE);

                let handle = tokio::spawn(async move {
                    while let Some(msg) = rx.recv().await {
                        let _ = file.write_all(msg.as_bytes()).await;
                        let _ = file.write_all(b"\n").await;
                    }
                });

                Ok((
                    Self {
                        tx: Some(tx),
                        syncjob_str: None,
                    },
                    Some(handle),
                ))
            }
            None => Ok((
                Self {
                    tx: None,
                    syncjob_str: None,
                },
                None,
            )),
        }
    }

    pub fn annotate_syncjob(&self, syncjob: &SyncJob) -> Self {
        Self {
            tx: self.tx.clone(),
            syncjob_str: Some(format!("{:?}", syncjob)),
        }
    }

    pub async fn log(&self, operation: &str, details: &str) -> Result<()> {
        if let Some(tx) = &self.tx {
            tx.send(format!(
                "[{}] [{}] {} {}",
                Local::now().format("%Y-%m-%d %H:%M:%S"),
                if let Some(syncjob) = &self.syncjob_str {
                    syncjob
                } else {
                    "unknown"
                },
                operation,
                details
            ))
            .await?;
        }
        Ok(())
    }
}
