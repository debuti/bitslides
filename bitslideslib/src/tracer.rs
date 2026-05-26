use std::path::PathBuf;

use anyhow::{bail, Result};
use chrono::Local;
use tokio::{
    fs::{File, OpenOptions},
    io::AsyncWriteExt,
    sync::mpsc::{self, Sender},
};

#[derive(PartialEq, Eq)]
pub enum Sink<'a> {
    StdOut,
    File(&'a PathBuf),
}

/// Tracer abstraction
///
/// The tracer is a logging utility that asynchronously writes trace messages to a file.
/// It uses a channel-based approach to avoid blocking the main execution flow when writing logs.
///
/// The messages are written on the sending end, using the tracer resources, and sent via a [`mpsc::channel::<String>`].
///
pub struct Tracer {
    tx: Option<Sender<String>>,
    author: Option<String>,
}

impl Tracer {
    pub async fn new(sinks: &[Sink<'_>]) -> Result<Self> {
        if sinks.is_empty() {
            // The user may want to disable tracing by not providing any sink
            return Ok(Self {
                tx: None,
                author: None,
            });
        }

        let (tx, mut rx) = mpsc::channel::<String>(crate::config::TRACE_CHANNEL_CAPACITY);

        let stdout = sinks.iter().filter(|x| Sink::StdOut == **x).count() > 0;

        let mut files = {
            let mut result: Vec<File> = Vec::new();

            for sink in sinks {
                if let Sink::File(path) = sink {
                    result.push(
                        OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(path)
                            .await?,
                    );
                }
            }

            result
        };

        // Drop the JoinHandle. The async task is now free to die when it finishes its job

        // FIXME: Retrieve the handle to control its lifetime and ensure it is properly shutdown (see https://github.com/debuti/bitslides/pull/5#discussion_r3299838669)
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if stdout {
                    println!("{msg}");
                }
                for file in &mut files {
                    let _ = file.write_all(msg.as_bytes()).await;
                    let _ = file.write_all(b"\n").await;
                }
            }
        });

        Ok(Self {
            tx: Some(tx),
            author: None,
        })
    }

    // FIXME: Consider returning a FunctionalTracer meaning that only a Tracer that has been acquired via
    //        annotate_author can be used for actual tracing
    pub fn annotate_author(&self, author: String) -> Self {
        Self {
            tx: self.tx.clone(),
            author: Some(author),
        }
    }

    fn compose_log_message(&self, operation: &str, details: &str) -> Result<String> {
        let Some(author) = &self.author else {
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
