use std::path::PathBuf;
use std::sync::Mutex;

use slipbox_core::{
    BacklinksParams, BacklinksResult, ForwardLinksParams, ForwardLinksResult, GlossaryDueParams,
    GlossaryDueResult, GlossaryTermParams, GlossaryTermResult, ListGlossaryTermsParams,
    ListGlossaryTermsResult, NodeFromIdParams, NodeFromKeyParams, NodeFromTitleOrAliasParams,
    NodeRecord, NoteContextParams, NoteContextResult, PingInfo, RandomNodeResult, ReflinksParams,
    ReflinksResult, SearchGlossaryParams, SearchGlossaryResult, SearchNodeContentParams,
    SearchNodeContentResult, SearchNodesParams, SearchNodesResult, StatusInfo,
    UnlinkedReferencesParams, UnlinkedReferencesResult,
};
use slipbox_daemon_client::{DaemonClient, DaemonClientError, DaemonServeConfig};

use crate::error::ReadingBridgeError;

/// A read-only view over a slipbox, backed by a spawned `slipbox serve` daemon.
///
/// [`spawn`](Self::spawn) forces [`DaemonServeConfig::read_only`] on, and the
/// method surface below is a strict allowlist of read-only methods. Every method
/// takes `&self` and serializes through an internal [`Mutex`], the single stdio
/// pipe's serialization point, so one `Arc<ReadingBridge>` serves every client. A
/// poisoned mutex is not recovered from.
pub struct ReadingBridge {
    session: Mutex<Session>,
}

/// The daemon session: the live client, plus what it takes to start another.
struct Session {
    client: Option<DaemonClient>,
    program: PathBuf,
    config: DaemonServeConfig,
    /// Whether some daemon in this session has ever completed a call. Until one
    /// has, a failure is the configuration's, not a lost child's.
    answered: bool,
}

impl Session {
    /// The live client, spawning a daemon first if the session has none.
    fn client(&mut self) -> Result<&mut DaemonClient, DaemonClientError> {
        if self.client.is_none() {
            self.client = Some(DaemonClient::spawn(self.program.clone(), &self.config)?);
        }
        Ok(self
            .client
            .as_mut()
            .expect("the session just spawned a client"))
    }
}

impl ReadingBridge {
    /// Spawn a read-only daemon and wrap it as a reading bridge. `program` is the
    /// `slipbox` executable to run; `config` provides the root, database, and
    /// discovery scope, with its read-only flag forced on and the forced
    /// configuration kept for any later respawn.
    pub fn spawn(
        program: impl Into<PathBuf>,
        config: DaemonServeConfig,
    ) -> Result<Self, ReadingBridgeError> {
        let program = program.into();
        let config = config.read_only(true);
        let client = DaemonClient::spawn(program.clone(), &config)?;
        Ok(Self {
            session: Mutex::new(Session {
                client: Some(client),
                program,
                config,
                answered: false,
            }),
        })
    }

    /// Run one daemon call under the serialization lock. A call that fails
    /// because the pipe is gone discards the dead client and runs once more
    /// against a fresh daemon, so `call` must be repeatable.
    fn with_client<T>(
        &self,
        call: impl Fn(&mut DaemonClient) -> Result<T, DaemonClientError>,
    ) -> Result<T, ReadingBridgeError> {
        let mut session = self
            .session
            .lock()
            .map_err(|_| ReadingBridgeError::Poisoned)?;

        match call(session.client()?) {
            Ok(value) => {
                session.answered = true;
                Ok(value)
            }
            // Only a daemon that has answered before is respawned: one that
            // never answered is refusing the configuration, and a fresh child
            // would refuse it identically.
            Err(error) if session.answered && lost_the_daemon(&error) => {
                session.client = None;
                let retried = call(session.client()?).map_err(ReadingBridgeError::Daemon)?;
                Ok(retried)
            }
            Err(error) => Err(ReadingBridgeError::Daemon(error)),
        }
    }

    /// Check daemon liveness and report the served root identity.
    pub fn ping(&self) -> Result<PingInfo, ReadingBridgeError> {
        self.with_client(DaemonClient::ping)
    }

    /// Report the served root, database path, and derived-index counts.
    pub fn status(&self) -> Result<StatusInfo, ReadingBridgeError> {
        self.with_client(DaemonClient::status)
    }

    /// Search indexed note records.
    pub fn search_nodes(
        &self,
        params: &SearchNodesParams,
    ) -> Result<SearchNodesResult, ReadingBridgeError> {
        self.with_client(|client| client.search_nodes(params))
    }

    /// Search indexed note bodies, ranked, with a highlighted excerpt per hit.
    /// A separate index from [`search_nodes`](Self::search_nodes), which matches
    /// note metadata.
    pub fn search_node_content(
        &self,
        params: &SearchNodeContentParams,
    ) -> Result<SearchNodeContentResult, ReadingBridgeError> {
        self.with_client(|client| client.search_node_content(params))
    }

    /// Return one random indexed note.
    pub fn random_node(&self) -> Result<RandomNodeResult, ReadingBridgeError> {
        self.with_client(DaemonClient::random_node)
    }

    /// Resolve a note by exact Org ID.
    pub fn node_from_id(
        &self,
        params: &NodeFromIdParams,
    ) -> Result<Option<NodeRecord>, ReadingBridgeError> {
        self.with_client(|client| client.node_from_id(params))
    }

    /// Resolve a note by exact slipbox key.
    pub fn node_from_key(
        &self,
        params: &NodeFromKeyParams,
    ) -> Result<Option<NodeRecord>, ReadingBridgeError> {
        self.with_client(|client| client.node_from_key(params))
    }

    /// Resolve notes by exact title or alias.
    pub fn node_from_title_or_alias(
        &self,
        params: &NodeFromTitleOrAliasParams,
    ) -> Result<Option<NodeRecord>, ReadingBridgeError> {
        self.with_client(|client| client.node_from_title_or_alias(params))
    }

    /// Read a source slice and immediate relation context for an indexed note.
    pub fn note_context(
        &self,
        params: &NoteContextParams,
    ) -> Result<NoteContextResult, ReadingBridgeError> {
        self.with_client(|client| client.note_context(params))
    }

    /// Return incoming links for an indexed note.
    pub fn backlinks(
        &self,
        params: &BacklinksParams,
    ) -> Result<BacklinksResult, ReadingBridgeError> {
        self.with_client(|client| client.backlinks(params))
    }

    /// Return outgoing links for an indexed note.
    pub fn forward_links(
        &self,
        params: &ForwardLinksParams,
    ) -> Result<ForwardLinksResult, ReadingBridgeError> {
        self.with_client(|client| client.forward_links(params))
    }

    /// Return links to the references an indexed note carries.
    pub fn reflinks(&self, params: &ReflinksParams) -> Result<ReflinksResult, ReadingBridgeError> {
        self.with_client(|client| client.reflinks(params))
    }

    /// Find unlinked mention candidates for an indexed note's references.
    pub fn unlinked_references(
        &self,
        params: &UnlinkedReferencesParams,
    ) -> Result<UnlinkedReferencesResult, ReadingBridgeError> {
        self.with_client(|client| client.unlinked_references(params))
    }

    /// List indexed glossary terms.
    pub fn list_glossary_terms(
        &self,
        params: &ListGlossaryTermsParams,
    ) -> Result<ListGlossaryTermsResult, ReadingBridgeError> {
        self.with_client(|client| client.list_glossary_terms(params))
    }

    /// Search indexed glossary terms.
    pub fn search_glossary(
        &self,
        params: &SearchGlossaryParams,
    ) -> Result<SearchGlossaryResult, ReadingBridgeError> {
        self.with_client(|client| client.search_glossary(params))
    }

    /// List glossary terms due for review as of a given day.
    pub fn glossary_due(
        &self,
        params: &GlossaryDueParams,
    ) -> Result<GlossaryDueResult, ReadingBridgeError> {
        self.with_client(|client| client.glossary_due(params))
    }

    /// Resolve one glossary term by slipbox key.
    pub fn glossary_term(
        &self,
        params: &GlossaryTermParams,
    ) -> Result<GlossaryTermResult, ReadingBridgeError> {
        self.with_client(|client| client.glossary_term(params))
    }

    /// Shut the daemon down cleanly, consuming the bridge. A session whose
    /// daemon is already gone reports success.
    pub fn shutdown(self) -> Result<(), ReadingBridgeError> {
        let session = self
            .session
            .into_inner()
            .map_err(|_| ReadingBridgeError::Poisoned)?;
        match session.client {
            Some(client) => client.shutdown().map_err(ReadingBridgeError::Daemon),
            None => Ok(()),
        }
    }
}

/// True when a failure means the daemon pipe can no longer carry requests: the
/// child is gone, or its response framing is out of step with ours. Every other
/// failure is an answer, leaving the session usable.
fn lost_the_daemon(error: &DaemonClientError) -> bool {
    matches!(
        error,
        DaemonClientError::ConnectionClosed
            | DaemonClientError::DaemonExited { .. }
            | DaemonClientError::UnexpectedEof
            | DaemonClientError::ReadResponse { .. }
            | DaemonClientError::WriteRequest { .. }
            | DaemonClientError::ResponseIdMismatch { .. }
    )
}

#[cfg(test)]
mod tests {
    use slipbox_daemon_client::DaemonClientError;

    use super::{ReadingBridge, lost_the_daemon};

    #[test]
    fn reading_bridge_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ReadingBridge>();
    }

    #[test]
    fn a_gone_daemon_is_recoverable() {
        assert!(lost_the_daemon(&DaemonClientError::ConnectionClosed));
        assert!(lost_the_daemon(&DaemonClientError::UnexpectedEof));
        assert!(lost_the_daemon(&DaemonClientError::ResponseIdMismatch {
            expected: "1".to_owned(),
            actual: "2".to_owned(),
        }));
    }

    #[test]
    fn a_daemon_that_answered_is_not_a_lost_session() {
        assert!(!lost_the_daemon(
            &DaemonClientError::MissingResponsePayload {
                method: "slipbox/status"
            }
        ));
        assert!(!lost_the_daemon(&DaemonClientError::MalformedResult {
            method: "slipbox/status",
            source: serde_json::from_str::<u32>("nope").expect_err("not a number"),
        }));
    }
}
