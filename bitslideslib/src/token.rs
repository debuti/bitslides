use anyhow::Result;
use notify::RecommendedWatcher;

/// Token for the user of the slide.
/// 
/// Keeps the relevant types alive, unless explicitly dropped
#[allow(dead_code)]
pub struct Token {
    /// Watcher OS task handle
    pub(crate) watcher: RecommendedWatcher,
}

impl Token {
    pub(crate) fn new(
        watcher: RecommendedWatcher,
    ) -> Self {
        Self {
            watcher,
        }
    }

    // FIXME: If after the refactoring this is still dumb, Move this to a "New type" pattern and forget about it
    pub async fn enough(self) -> Result<()> {
        // TODO: Ideally this should be happening in the Drop impl for Token. 
        //       But that wont let us control the results of the awaited tasks.
        //       
        //       Maybe that doesnt matter? Every task (core, syncjob) can log a message and die. Tracer is not even fallible

        let watcher = self.watcher;
        // let handles = self.handles;
        // let tracer = self.tracer;

        // Drop the watcher first, so that the mpsc channels can be closed
        // and the syncjob tasks can finish
        drop(watcher);

        // // Await all the handles. When every syncjob task finishes, its
        // // tracer mpsc channel will be closed
        // for handle in handles {
        //     let _ = handle.await?;
        // }

        // // Await the tracer if any
        // if let Some(tracer) = tracer {
        //     tracer.await?;
        // }

        Ok(())
    }
}
