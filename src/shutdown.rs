use tokio::sync::broadcast;

/// Listen for the server shutdown signal.
///
/// Shutdown is signaled using a `broadcast::Receiver`. Only a single
/// value is ever sent. Once a value has been sent via the broadcast
/// channel, the server should shutdown.
///
/// The `shutdown` struct listens for the signal and tracks that the signal
/// has been received. Callers may query for whether the shutdown signal has
/// been received or not.
#[derive(Debug)]
pub(crate) struct Shutdown {
    shutdown: bool,

    notify: broadcast::Receiver<()>,
}

impl Shutdown {
    /// Creates a new `Shutdown` backed by the given `broadcast::Receiver`.
    pub(crate) fn new(notify: broadcast::Receiver<()>) -> Shutdown {
        Shutdown {
            shutdown: false,
            notify,
        }
    }

    /// Returns `true` if shutdown signal has been received.
    pub(crate) fn is_shutdown(&self) -> bool {
        self.shutdown
    }

    /// Receive the shutdown notice, waiting if necessary.
    pub(crate) async fn recv(&mut self) {
        // If the shutdown signal has already been received,
        // then return immediately.
        if self.shutdown {
            return;
        }

        // Can't receive a "lag error" as only one value is ever sent.
        let _ = self.notify.recv().await;

        // Remember that the signal has been received.
        self.shutdown = true;
    }
}
