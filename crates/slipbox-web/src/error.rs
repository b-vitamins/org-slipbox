use slipbox_daemon_client::DaemonClientError;

/// Failure from a [`ReadingBridge`](crate::ReadingBridge) request.
#[derive(Debug, thiserror::Error)]
pub enum ReadingBridgeError {
    /// The daemon rejected or failed the request, carried unchanged.
    #[error(transparent)]
    Daemon(#[from] DaemonClientError),
    /// A prior request panicked holding the daemon pipe, so its request framing
    /// may be desynchronized and the bridge fails closed.
    #[error("read-only slipbox session is poisoned and can no longer serve requests")]
    Poisoned,
}
