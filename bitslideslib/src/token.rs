/// Token for the user of the slide.
///
/// Keeps the relevant types alive, unless explicitly dropped
#[allow(dead_code)]
#[must_use = "The token must be held to ensure bitslides tasks are alive. Drop it to stop all bitslides tasks and free the memory"]
pub struct Token {
    pub(crate) cancellation_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

impl Token {
    pub(crate) const fn new(cancellation_tx: tokio::sync::oneshot::Sender<()>) -> Self {
        Self {
            cancellation_tx: Some(cancellation_tx),
        }
    }

    // // FIXME: If after the refactoring this is still dumb, Move this to a "New type" pattern and forget about it
    // pub async fn enough(self) -> Result<()> {
    //     // TODO: Ideally this should be happening in the Drop impl for Token.
    //     //       But that wont let us control the results of the awaited tasks.
    //     //
    //     //       Maybe that doesnt matter? Every task (core, syncjob) can log a message and die. Tracer is not even fallible

    //     // let handles = self.handles;
    //     // let tracer = self.tracer;

    //     // When the watcher is dropped, the OS task will be killed, and the mpsc channel will be closed,
    //     // then the core task will finish, and with it all the mpsc channels for the syncjobs,
    //     // then all the syncjobs will finish, cleaning the full memory

    //     let _ = self.cancellation_tx.send(());

    //     // // Await all the handles. When every syncjob task finishes, its
    //     // // tracer mpsc channel will be closed
    //     // for handle in handles {
    //     //     let _ = handle.await?;
    //     // }

    //     // // Await the tracer if any
    //     // if let Some(tracer) = tracer {
    //     //     tracer.await?;
    //     // }

    //     Ok(())
    // }
}

impl Drop for Token {
    fn drop(&mut self) {
        if let Some(cancellation_tx) = self.cancellation_tx.take() {
            // FIXME: Set and forget operation. We may want to wait until all the tasks are actually shutdown (See https://github.com/debuti/bitslides/pull/5#discussion_r3299838701)
            let _ = cancellation_tx.send(());
        }
    }
}
