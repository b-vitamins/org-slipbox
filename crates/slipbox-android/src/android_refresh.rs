//! Android ownership for complete source-scoped repository refreshes.

use std::path::PathBuf;
use std::sync::OnceLock;

use slipbox_git::AccessToken;
use slipbox_rpc::android_refresh as wire;
use slipbox_sync as domain;

static COORDINATOR: OnceLock<domain::RefreshCoordinator> = OnceLock::new();

#[must_use]
pub fn serve_refresh(request: &[u8], credential: Option<Vec<u8>>) -> Vec<u8> {
    let request = match wire::decode_refresh_request(request) {
        Ok(request) => request,
        Err(reason) => return refused(reason, wire::RefreshRetry::Never),
    };
    let credential = match credential.map(AccessToken::new).transpose() {
        Ok(credential) => credential,
        Err(_) => {
            return refused(
                wire::RefreshFailureReason::AuthorizationFailed,
                wire::RefreshRetry::Reauthorize,
            );
        }
    };
    let context = match domain::RefreshContext::new(
        request.operation,
        request.attempt,
        request.source,
        PathBuf::from(request.repository),
        PathBuf::from(request.store),
    ) {
        Ok(context) => context,
        Err(failure) => return refused_failure(failure),
    };
    let operation = context.operation();
    let outcome = coordinator().coordinate(context.clone(), |control| {
        domain::refresh_repository(&context, credential, control)
    });
    wire::encode_refresh_response(&wire::RefreshResponse::answered(
        operation,
        match outcome.disposition {
            domain::RefreshDisposition::Started => wire::RefreshDisposition::Started,
            domain::RefreshDisposition::Coalesced => wire::RefreshDisposition::Coalesced,
            domain::RefreshDisposition::Refused => {
                let failure = outcome
                    .status
                    .failure
                    .expect("a refused coordinated refresh carries a failure");
                return refused_failure(failure);
            }
        },
        map_status(outcome.status),
    ))
}

#[must_use]
pub fn serve_refresh_status(request: &[u8]) -> Vec<u8> {
    let response = match wire::decode_refresh_status_request(request) {
        Ok(request) => match coordinator().status(&request.source) {
            Some(status) => wire::RefreshStatusResponse {
                version: wire::REFRESH_PROTOCOL_VERSION,
                outcome: wire::RefreshStatusOutcome::Known {
                    status: Box::new(map_status(status)),
                },
            },
            None => wire::RefreshStatusResponse {
                version: wire::REFRESH_PROTOCOL_VERSION,
                outcome: wire::RefreshStatusOutcome::Idle {
                    source: request.source,
                },
            },
        },
        Err(reason) => wire::RefreshStatusResponse {
            version: wire::REFRESH_PROTOCOL_VERSION,
            outcome: wire::RefreshStatusOutcome::Refused { reason },
        },
    };
    wire::encode_refresh_status_response(&response)
}

#[must_use]
pub fn cancel_refresh(operation: i64) -> bool {
    coordinator().cancel(operation)
}

fn coordinator() -> &'static domain::RefreshCoordinator {
    COORDINATOR.get_or_init(domain::RefreshCoordinator::default)
}

fn refused(reason: wire::RefreshFailureReason, retry: wire::RefreshRetry) -> Vec<u8> {
    wire::encode_refresh_response(&wire::RefreshResponse::refused(reason, retry))
}

fn refused_failure(failure: domain::RefreshFailure) -> Vec<u8> {
    refused(map_reason(failure.reason), map_retry(failure.retry))
}

fn map_status(status: domain::RefreshStatus) -> wire::RefreshStatus {
    wire::RefreshStatus {
        source: status.source,
        operation: status.operation,
        state: match status.state {
            domain::RefreshState::Recovering => wire::RefreshState::Recovering,
            domain::RefreshState::Fetching => wire::RefreshState::Fetching,
            domain::RefreshState::Comparing => wire::RefreshState::Comparing,
            domain::RefreshState::Materializing => wire::RefreshState::Materializing,
            domain::RefreshState::Indexing => wire::RefreshState::Indexing,
            domain::RefreshState::Publishing => wire::RefreshState::Publishing,
            domain::RefreshState::Ready => wire::RefreshState::Ready,
            domain::RefreshState::Failed => wire::RefreshState::Failed,
            domain::RefreshState::Cancelled => wire::RefreshState::Cancelled,
        },
        fetched_revision: status.fetched_revision,
        ready_revision: status.ready_revision,
        ready_generation: status.ready_generation,
        progress: wire::RefreshProgress {
            completed: status.progress.completed,
            total: status.progress.total,
            unit: match status.progress.unit {
                domain::RefreshProgressUnit::None => wire::RefreshProgressUnit::None,
                domain::RefreshProgressUnit::Objects => wire::RefreshProgressUnit::Objects,
                domain::RefreshProgressUnit::Entries => wire::RefreshProgressUnit::Entries,
                domain::RefreshProgressUnit::Files => wire::RefreshProgressUnit::Files,
                domain::RefreshProgressUnit::Steps => wire::RefreshProgressUnit::Steps,
            },
        },
        failure: status.failure.map(|failure| wire::RefreshFailure {
            reason: map_reason(failure.reason),
            retry: map_retry(failure.retry),
        }),
    }
}

fn map_reason(reason: domain::RefreshFailureReason) -> wire::RefreshFailureReason {
    match reason {
        domain::RefreshFailureReason::RequestRefused => {
            wire::RefreshFailureReason::MalformedRequest
        }
        domain::RefreshFailureReason::OperationConflict => {
            wire::RefreshFailureReason::OperationConflict
        }
        domain::RefreshFailureReason::OperationsExhausted => {
            wire::RefreshFailureReason::OperationsExhausted
        }
        domain::RefreshFailureReason::AuthorizationRequired => {
            wire::RefreshFailureReason::AuthorizationRequired
        }
        domain::RefreshFailureReason::AuthorizationFailed => {
            wire::RefreshFailureReason::AuthorizationFailed
        }
        domain::RefreshFailureReason::TransportFailed => {
            wire::RefreshFailureReason::TransportFailed
        }
        domain::RefreshFailureReason::RepositoryInvalid => {
            wire::RefreshFailureReason::RepositoryInvalid
        }
        domain::RefreshFailureReason::BranchUnavailable => {
            wire::RefreshFailureReason::BranchUnavailable
        }
        domain::RefreshFailureReason::NotesFolderUnavailable => {
            wire::RefreshFailureReason::NotesFolderUnavailable
        }
        domain::RefreshFailureReason::UnsafeSource => wire::RefreshFailureReason::UnsafeSource,
        domain::RefreshFailureReason::InputExhausted => wire::RefreshFailureReason::InputExhausted,
        domain::RefreshFailureReason::RebuildRequired => {
            wire::RefreshFailureReason::RebuildRequired
        }
        domain::RefreshFailureReason::StorageFailed => wire::RefreshFailureReason::StorageFailed,
        domain::RefreshFailureReason::PublicationConflict => {
            wire::RefreshFailureReason::PublicationConflict
        }
        domain::RefreshFailureReason::Superseded => wire::RefreshFailureReason::Superseded,
        domain::RefreshFailureReason::Cancelled => wire::RefreshFailureReason::Cancelled,
        domain::RefreshFailureReason::Panicked => wire::RefreshFailureReason::Panicked,
    }
}

fn map_retry(retry: domain::RefreshRetry) -> wire::RefreshRetry {
    match retry {
        domain::RefreshRetry::Never => wire::RefreshRetry::Never,
        domain::RefreshRetry::Reauthorize => wire::RefreshRetry::Reauthorize,
        domain::RefreshRetry::Backoff { after_seconds } => {
            wire::RefreshRetry::Backoff { after_seconds }
        }
        domain::RefreshRetry::FreeStorage => wire::RefreshRetry::FreeStorage,
        domain::RefreshRetry::Rebuild => wire::RefreshRetry::Rebuild,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_and_credential_refusals_are_closed_and_contextual() {
        let malformed: wire::RefreshResponse =
            serde_json::from_slice(&serve_refresh(b"{}", None)).expect("response");
        assert_eq!(
            malformed.outcome,
            wire::RefreshOutcome::Refused(wire::RefreshFailure {
                reason: wire::RefreshFailureReason::MalformedRequest,
                retry: wire::RefreshRetry::Never,
            })
        );

        let credential = vec![0; 5_000];
        let response = serve_refresh(valid_request(), Some(credential));
        let response: wire::RefreshResponse = serde_json::from_slice(&response).expect("response");
        assert_eq!(
            response.outcome,
            wire::RefreshOutcome::Refused(wire::RefreshFailure {
                reason: wire::RefreshFailureReason::AuthorizationFailed,
                retry: wire::RefreshRetry::Reauthorize,
            })
        );
    }

    fn valid_request() -> &'static [u8] {
        br#"{"version":1,"operation":71,"attempt":0,"source":{"id":"0123456789abcdef0123456789abcdef","display_name":"Notes","provider":"generic_https","visibility":"public","remote":"https://example.com/notes.git","branch":"main","notes_folder":""},"repository":"/private/source/repository.git","store":"/private/source/store"}"#
    }
}
