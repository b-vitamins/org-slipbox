//! Android ownership for one bounded native Git operation.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, OnceLock};

use slipbox_core::{GitBranch, RemoteUrl};
use slipbox_git::{AccessToken, FetchDisposition, FetchRequest, GitError, synchronize};
use slipbox_rpc::android_git::{
    GitDisposition, GitFetched, GitOutcome, GitRefusalReason, GitResponse, decode_git_request,
    encode_git_response,
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
