//! Android ownership for one bounded native Git operation.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, OnceLock};

use slipbox_core::{GitBranch, NotesFolder, RemoteUrl, SourceId};
use slipbox_git::{
    AccessToken, FetchDisposition, FetchRequest, GitError, SnapshotDiagnosticReason,
    SnapshotDisposition, SnapshotError, SnapshotRequest, materialize, synchronize,
};
use slipbox_rpc::android_git::{
    GitDisposition, GitFetched, GitMaterialized, GitOutcome, GitRefusalReason, GitResponse,
    GitSnapshotDiagnostic, GitSnapshotDiagnosticReason, GitSnapshotDisposition, decode_git_request,
    decode_materialize_request, encode_git_response,
};

const MAX_ACTIVE_OPERATIONS: usize = 8;

static OPERATIONS: OnceLock<GitOperations> = OnceLock::new();

/// Serve one versioned request with an optional credential held only for this
/// call. All diagnostics are selected from the closed wire vocabulary.
#[must_use]
pub fn serve_synchronize(request: &[u8], credential: Option<Vec<u8>>) -> Vec<u8> {
    let credential = match credential.map(AccessToken::new).transpose() {
        Ok(credential) => credential,
        Err(error) => return refused(map_error(error)),
    };
    let request = match decode_git_request(request) {
        Ok(request) => request,
        Err(reason) => return refused(reason),
    };
    let remote = match RemoteUrl::parse(&request.remote) {
        Ok(remote) => remote,
        Err(_) => return refused(GitRefusalReason::MalformedRequest),
    };
    let branch = match GitBranch::parse(&request.branch) {
        Ok(branch) => branch,
        Err(_) => return refused(GitRefusalReason::MalformedRequest),
    };
    let fetch = match FetchRequest::new(remote, branch, PathBuf::from(&request.repository)) {
        Ok(fetch) => fetch,
        Err(error) => return refused(map_error(error)),
    };
    let operation = request.operation;
    let active = match operations().begin(operation) {
        Ok(active) => active,
        Err(reason) => return refused(reason),
    };
    let outcome = synchronize(&fetch, credential, active.cancelled(), |_| {});
    match outcome {
        Ok(outcome) => encode_git_response(&GitResponse::new(GitOutcome::Fetched(GitFetched {
            operation,
            disposition: match outcome.disposition {
                FetchDisposition::Cloned => GitDisposition::Cloned,
                FetchDisposition::Updated => GitDisposition::Updated,
                FetchDisposition::Unchanged => GitDisposition::Unchanged,
            },
            revision: outcome.revision,
            received_objects: outcome.received_objects,
        }))),
        Err(error) => refused(map_error(error)),
    }
}

/// Materialize one exact fetched revision into an immutable source-owned
/// candidate snapshot. No credential participates in this operation.
#[must_use]
pub fn serve_materialize(request: &[u8]) -> Vec<u8> {
    let request = match decode_materialize_request(request) {
        Ok(request) => request,
        Err(reason) => return refused(reason),
    };
    let source = match SourceId::parse(&request.source) {
        Ok(source) => source,
        Err(_) => return refused(GitRefusalReason::MalformedRequest),
    };
    let notes_folder = match NotesFolder::parse(&request.notes_folder) {
        Ok(notes_folder) => notes_folder,
        Err(_) => return refused(GitRefusalReason::MalformedRequest),
    };
    let snapshot = match SnapshotRequest::new(
        source,
        PathBuf::from(&request.repository),
        &request.revision,
        notes_folder,
        PathBuf::from(&request.snapshot),
    ) {
        Ok(snapshot) => snapshot,
        Err(error) => return refused(map_snapshot_error(error)),
    };
    let operation = request.operation;
    let active = match operations().begin(operation) {
        Ok(active) => active,
        Err(reason) => return refused(reason),
    };
    let outcome = materialize(&snapshot, active.cancelled(), |_| {});
    match outcome {
        Ok(outcome) => encode_git_response(&GitResponse::new(GitOutcome::Materialized(
            GitMaterialized {
                operation,
                disposition: match outcome.disposition {
                    SnapshotDisposition::Created => GitSnapshotDisposition::Created,
                    SnapshotDisposition::Existing => GitSnapshotDisposition::Existing,
                },
                revision: outcome.revision,
                entries: outcome.entries,
                files: outcome.files,
                org_files: outcome.org_files,
                assets: outcome.assets,
                bytes: outcome.bytes,
                diagnostics: outcome
                    .diagnostics
                    .into_iter()
                    .map(|diagnostic| GitSnapshotDiagnostic {
                        path: diagnostic.path,
                        reason: map_diagnostic(diagnostic.reason),
                    })
                    .collect(),
            },
        ))),
        Err(error) => refused(map_snapshot_error(error)),
    }
}

/// Request cancellation of one active operation. Unknown and already-finished
/// identities are defined as `false`.
#[must_use]
pub fn cancel(operation: i64) -> bool {
    operations().cancel(operation)
}

fn operations() -> &'static GitOperations {
    OPERATIONS.get_or_init(GitOperations::default)
}

fn refused(reason: GitRefusalReason) -> Vec<u8> {
    encode_git_response(&GitResponse::refused(reason))
}

fn map_error(error: GitError) -> GitRefusalReason {
    match error {
        GitError::DestinationRefused => GitRefusalReason::DestinationRefused,
        GitError::CredentialRefused => GitRefusalReason::CredentialRefused,
        GitError::Cancelled => GitRefusalReason::Cancelled,
        GitError::StorageFailed => GitRefusalReason::StorageFailed,
        GitError::RepositoryInvalid => GitRefusalReason::RepositoryInvalid,
        GitError::BranchUnavailable => GitRefusalReason::BranchUnavailable,
        GitError::TransportFailed => GitRefusalReason::TransportFailed,
    }
}

fn map_snapshot_error(error: SnapshotError) -> GitRefusalReason {
    match error {
        SnapshotError::LimitsRefused => GitRefusalReason::OutOfBounds,
        SnapshotError::DestinationRefused => GitRefusalReason::DestinationRefused,
        SnapshotError::DestinationOccupied => GitRefusalReason::DestinationOccupied,
        SnapshotError::StorageBoundary => GitRefusalReason::StorageBoundary,
        SnapshotError::OwnershipMismatch => GitRefusalReason::OwnershipMismatch,
        SnapshotError::RepositoryInvalid => GitRefusalReason::RepositoryInvalid,
        SnapshotError::RevisionRefused => GitRefusalReason::RevisionRefused,
        SnapshotError::RevisionUnavailable => GitRefusalReason::RevisionUnavailable,
        SnapshotError::NotesFolderUnavailable => GitRefusalReason::NotesFolderUnavailable,
        SnapshotError::UnsafePath => GitRefusalReason::UnsafePath,
        SnapshotError::ObjectInvalid => GitRefusalReason::ObjectInvalid,
        SnapshotError::InputExhausted => GitRefusalReason::InputExhausted,
        SnapshotError::DiagnosticsExhausted => GitRefusalReason::DiagnosticsExhausted,
        SnapshotError::Cancelled => GitRefusalReason::Cancelled,
        SnapshotError::StorageFailed => GitRefusalReason::StorageFailed,
    }
}

fn map_diagnostic(reason: SnapshotDiagnosticReason) -> GitSnapshotDiagnosticReason {
    match reason {
        SnapshotDiagnosticReason::EncryptedOrg => GitSnapshotDiagnosticReason::EncryptedOrg,
        SnapshotDiagnosticReason::UnsupportedEncoding => {
            GitSnapshotDiagnosticReason::UnsupportedEncoding
        }
        SnapshotDiagnosticReason::UnsupportedFormat => {
            GitSnapshotDiagnosticReason::UnsupportedFormat
        }
        SnapshotDiagnosticReason::OversizedInput => GitSnapshotDiagnosticReason::OversizedInput,
        SnapshotDiagnosticReason::Submodule => GitSnapshotDiagnosticReason::Submodule,
        SnapshotDiagnosticReason::LfsPointer => GitSnapshotDiagnosticReason::LfsPointer,
        SnapshotDiagnosticReason::ExternalFilter => GitSnapshotDiagnosticReason::ExternalFilter,
        SnapshotDiagnosticReason::SymlinkEscapes => GitSnapshotDiagnosticReason::SymlinkEscapes,
        SnapshotDiagnosticReason::SymlinkCycle => GitSnapshotDiagnosticReason::SymlinkCycle,
        SnapshotDiagnosticReason::SymlinkUnavailable => {
            GitSnapshotDiagnosticReason::SymlinkUnavailable
        }
    }
}

#[derive(Default)]
struct GitOperations {
    active: Mutex<BTreeMap<i64, Arc<AtomicBool>>>,
}

impl GitOperations {
    fn begin(&'static self, operation: i64) -> Result<ActiveOperation, GitRefusalReason> {
        let mut active = self
            .active
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if active.contains_key(&operation) {
            return Err(GitRefusalReason::OperationConflict);
        }
        if active.len() >= MAX_ACTIVE_OPERATIONS {
            return Err(GitRefusalReason::OperationsExhausted);
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        active.insert(operation, Arc::clone(&cancelled));
        Ok(ActiveOperation {
            owner: self,
            operation,
            cancelled,
        })
    }

    fn cancel(&self, operation: i64) -> bool {
        use std::sync::atomic::Ordering;

        let active = self
            .active
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let Some(cancelled) = active.get(&operation) else {
            return false;
        };
        cancelled.store(true, Ordering::Relaxed);
        true
    }
}

struct ActiveOperation {
    owner: &'static GitOperations,
    operation: i64,
    cancelled: Arc<AtomicBool>,
}

impl ActiveOperation {
    fn cancelled(&self) -> &AtomicBool {
        &self.cancelled
    }
}

impl Drop for ActiveOperation {
    fn drop(&mut self) {
        let mut active = self
            .owner
            .active
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        active.remove(&self.operation);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use super::*;

    #[test]
    fn active_operations_are_unique_bounded_and_cancellable() {
        let operations = Box::leak(Box::new(GitOperations::default()));
        let first = operations.begin(1).expect("first operation");
        assert_eq!(
            operations.begin(1).err(),
            Some(GitRefusalReason::OperationConflict)
        );
        let mut rest = Vec::new();
        for operation in 2..=MAX_ACTIVE_OPERATIONS as i64 {
            rest.push(operations.begin(operation).expect("operation slot"));
        }
        assert_eq!(
            operations.begin(99).err(),
            Some(GitRefusalReason::OperationsExhausted)
        );
        assert!(operations.cancel(1));
        assert!(first.cancelled().load(Ordering::Relaxed));
        assert!(!operations.cancel(99));
        drop(first);
        assert!(operations.begin(99).is_ok());
    }

    #[test]
    fn pre_cancelled_transport_answers_only_with_a_closed_reason() {
        let request = br#"{"version":1,"operation":451,"remote":"https://github.com/o/r.git","branch":"main","repository":"/tmp/slipbox-git-pre-cancelled"}"#;
        let active = operations().begin(451).expect("reserved operation");
        assert!(cancel(451));
        let fetch = FetchRequest::new(
            RemoteUrl::parse("https://github.com/o/r.git").expect("remote"),
            GitBranch::parse("main").expect("branch"),
            PathBuf::from("/tmp/slipbox-git-pre-cancelled"),
        )
        .expect("request");
        assert_eq!(
            synchronize(&fetch, None, active.cancelled(), |_| {}),
            Err(GitError::Cancelled)
        );
        drop(active);

        let decoded = decode_git_request(request).expect("the wire request remains valid");
        assert_eq!(decoded.operation, 451);
    }
}
