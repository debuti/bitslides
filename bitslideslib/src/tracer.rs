use std::path::PathBuf;

use anyhow::{bail, Result};
use chrono::Local;
use tokio::{
    fs::OpenOptions,
    io::AsyncWriteExt,
    sync::mpsc::{self, Sender},
    task::JoinHandle,
};

/// Tracer abstraction
///
/// The tracer is a logging utility that asynchronously writes trace messages to a file.
/// It uses a channel-based approach to avoid blocking the main execution flow when writing logs.
///
pub struct Tracer {
    tx: Option<Sender<String>>,
    author: Option<String>,
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
                        author: None,
                    },
                    Some(handle),
                ))
            }
            // The user may want to disable tracing by not providing a path
            None => Ok((
                Self {
                    tx: None,
                    author: None,
                },
                None,
            )),
        }
    }

    pub fn annotate_author(&self, author: String) -> Self {
        Self {
            tx: self.tx.clone(),
            author: Some(author),
        }
    }

    fn compose_log_message(&self, operation: &str, details: &str) -> Result<String> {
        let author = if let Some(author) = &self.author {
            author
        } else {
            bail!("Tracer author not set")
        };
        Ok(format!(
            "[{}] [{}] {} {}",
            Local::now().format("%Y-%m-%d %H:%M:%S"),
            author,
            operation,
            details
        ))
    }

    pub async fn async_log(&self, operation: &str, details: &str) -> Result<()> {
        if let Some(tx) = &self.tx {
            tx.send(self.compose_log_message(operation, details)?)
                .await?;
        }
        Ok(())
    }

    pub fn sync_log(&self, operation: &str, details: &str) -> Result<()> {
        if let Some(tx) = &self.tx {
            tx.blocking_send(self.compose_log_message(operation, details)?)?;
        }
        Ok(())
    }
}
