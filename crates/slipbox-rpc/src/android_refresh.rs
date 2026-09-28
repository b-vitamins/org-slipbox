//! Versioned Android repository-refresh transport.
//!
//! The source record is the complete initiating identity. Credentials remain a
//! separate JNI argument and are never serialized into these documents.

use serde::{Deserialize, Serialize};
use slipbox_core::{GenerationId, SourceId, SourceRecord};

use crate::android::{ResponseEncoding, write_bounded};

pub const REFRESH_PROTOCOL_VERSION: u32 = 1;
pub const MAX_REFRESH_REQUEST_BYTES: usize = 32 * 1024;
pub const MAX_REFRESH_RESPONSE_BYTES: usize = 64 * 1024;
pub const MAX_REFRESH_TEXT_BYTES: usize = 4_096;
pub const MAX_REFRESH_ATTEMPTS: u8 = 3;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RefreshRequest {
    pub version: u32,
    pub operation: i64,
    pub attempt: u8,
    pub source: SourceRecord,
    pub repository: String,
    pub store: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RefreshStatusRequest {
    pub version: u32,
    pub source: SourceId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefreshResponse {
    pub version: u32,
    #[serde(flatten)]
    pub outcome: RefreshOutcome,
}

impl RefreshResponse {
    #[must_use]
    pub fn answered(
        operation: i64,
        disposition: RefreshDisposition,
        status: RefreshStatus,
    ) -> Self {
        Self {
            version: REFRESH_PROTOCOL_VERSION,
            outcome: RefreshOutcome::Answered(Box::new(RefreshAnswered {
                operation,
                disposition,
                status,
            })),
        }
    }

    #[must_use]
    pub fn refused(reason: RefreshFailureReason, retry: RefreshRetry) -> Self {
        Self {
            version: REFRESH_PROTOCOL_VERSION,
            outcome: RefreshOutcome::Refused(RefreshFailure { reason, retry }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum RefreshOutcome {
    Answered(Box<RefreshAnswered>),
    Refused(RefreshFailure),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RefreshAnswered {
    pub operation: i64,
    pub disposition: RefreshDisposition,
    pub status: RefreshStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RefreshDisposition {
    Started,
    Coalesced,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RefreshStatusResponse {
    pub version: u32,
    pub outcome: RefreshStatusOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum RefreshStatusOutcome {
    Known { status: Box<RefreshStatus> },
    Idle { source: SourceId },
    Refused { reason: RefreshFailureReason },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RefreshStatus {
    pub source: SourceRecord,
    pub operation: i64,
    pub state: RefreshState,
    pub fetched_revision: Option<String>,
    pub ready_revision: Option<String>,
    pub ready_generation: Option<GenerationId>,
    pub progress: RefreshProgress,
    pub failure: Option<RefreshFailure>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RefreshState {
    Recovering,
    Fetching,
    Comparing,
    Materializing,
    Indexing,
    Publishing,
    Ready,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RefreshProgress {
    pub completed: u64,
    pub total: u64,
    pub unit: RefreshProgressUnit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RefreshProgressUnit {
    None,
    Objects,
    Entries,
    Files,
    Steps,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RefreshFailure {
    pub reason: RefreshFailureReason,
    pub retry: RefreshRetry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RefreshFailureReason {
    UnsupportedVersion,
    MalformedRequest,
    OutOfBounds,
    OperationConflict,
    OperationsExhausted,
    ForegroundRefused,
    TlsInitializationFailed,
    AuthorizationRequired,
    AuthorizationFailed,
    RateLimited,
    TransportFailed,
    RepositoryInvalid,
    BranchUnavailable,
    NotesFolderUnavailable,
    UnsafeSource,
    InputExhausted,
    RebuildRequired,
    StorageFailed,
    PublicationConflict,
    Superseded,
    Cancelled,
    EncodingFailed,
    Panicked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "strategy", rename_all = "kebab-case")]
pub enum RefreshRetry {
    Never,
    Reauthorize,
    Backoff { after_seconds: u64 },
    FreeStorage,
    Rebuild,
}

pub fn decode_refresh_request(bytes: &[u8]) -> Result<RefreshRequest, RefreshFailureReason> {
    if bytes.len() > MAX_REFRESH_REQUEST_BYTES {
        return Err(RefreshFailureReason::OutOfBounds);
    }
    let request: RefreshRequest =
        serde_json::from_slice(bytes).map_err(|_| RefreshFailureReason::MalformedRequest)?;
    if request.version != REFRESH_PROTOCOL_VERSION {
        return Err(RefreshFailureReason::UnsupportedVersion);
    }
    if request.operation <= 0
        || request.attempt >= MAX_REFRESH_ATTEMPTS
        || request.repository.is_empty()
        || request.store.is_empty()
    {
        return Err(RefreshFailureReason::MalformedRequest);
    }
    if request.repository.len() > MAX_REFRESH_TEXT_BYTES
        || request.store.len() > MAX_REFRESH_TEXT_BYTES
    {
        return Err(RefreshFailureReason::OutOfBounds);
    }
    Ok(request)
}

pub fn decode_refresh_status_request(
    bytes: &[u8],
) -> Result<RefreshStatusRequest, RefreshFailureReason> {
    if bytes.len() > MAX_REFRESH_REQUEST_BYTES {
        return Err(RefreshFailureReason::OutOfBounds);
    }
    let request: RefreshStatusRequest =
        serde_json::from_slice(bytes).map_err(|_| RefreshFailureReason::MalformedRequest)?;
    if request.version != REFRESH_PROTOCOL_VERSION {
        return Err(RefreshFailureReason::UnsupportedVersion);
    }
    Ok(request)
}

#[must_use]
pub fn encode_refresh_response(response: &RefreshResponse) -> Vec<u8> {
    bounded(response, || {
        RefreshResponse::refused(RefreshFailureReason::EncodingFailed, RefreshRetry::Never)
    })
}

#[must_use]
pub fn encode_refresh_status_response(response: &RefreshStatusResponse) -> Vec<u8> {
    bounded(response, || RefreshStatusResponse {
        version: REFRESH_PROTOCOL_VERSION,
        outcome: RefreshStatusOutcome::Refused {
            reason: RefreshFailureReason::EncodingFailed,
        },
    })
}

fn bounded<T: Serialize>(value: &T, fallback: impl FnOnce() -> T) -> Vec<u8> {
    match write_bounded(value, MAX_REFRESH_RESPONSE_BYTES) {
        ResponseEncoding::Written(bytes) => bytes,
        ResponseEncoding::Oversized { .. } | ResponseEncoding::Failed { .. } => {
            serde_json::to_vec(&fallback()).expect("the fixed refresh refusal is serializable")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_is_versioned_bounded_and_carries_no_credential_bytes() {
        let valid = br#"{"version":1,"operation":7,"attempt":0,"source":{"id":"0123456789abcdef0123456789abcdef","display_name":"Notes","provider":"generic_https","visibility":"public","remote":"https://example.com/notes.git","branch":"main","notes_folder":""},"repository":"/private/source/repository.git","store":"/private/source/store"}"#;
        let request = decode_refresh_request(valid).expect("valid refresh request");
        assert_eq!(request.operation, 7);
        assert!(!String::from_utf8_lossy(valid).contains("access_token"));

        for invalid in [
            br#"{"version":2,"operation":7,"attempt":0,"source":{"id":"0123456789abcdef0123456789abcdef","display_name":"Notes","provider":"generic_https","visibility":"public","remote":"https://example.com/notes.git","branch":"main","notes_folder":""},"repository":"/private/source/repository.git","store":"/private/source/store"}"#.as_slice(),
            br#"{"version":1,"operation":0,"attempt":0,"source":{"id":"0123456789abcdef0123456789abcdef","display_name":"Notes","provider":"generic_https","visibility":"public","remote":"https://example.com/notes.git","branch":"main","notes_folder":""},"repository":"/private/source/repository.git","store":"/private/source/store"}"#.as_slice(),
            br#"{"version":1,"operation":7,"attempt":3,"source":{"id":"0123456789abcdef0123456789abcdef","display_name":"Notes","provider":"generic_https","visibility":"public","remote":"https://example.com/notes.git","branch":"main","notes_folder":""},"repository":"/private/source/repository.git","store":"/private/source/store"}"#.as_slice(),
        ] {
            assert!(decode_refresh_request(invalid).is_err());
        }
    }

    #[test]
    fn statuses_keep_fetched_ready_and_retry_state_distinct() {
        let source = decode_refresh_request(
            br#"{"version":1,"operation":7,"attempt":0,"source":{"id":"0123456789abcdef0123456789abcdef","display_name":"Notes","provider":"generic_https","visibility":"public","remote":"https://example.com/notes.git","branch":"main","notes_folder":""},"repository":"/private/source/repository.git","store":"/private/source/store"}"#,
        )
        .expect("request")
        .source;
        let status = RefreshStatus {
            source,
            operation: 9,
            state: RefreshState::Failed,
            fetched_revision: Some("fetched".to_owned()),
            ready_revision: Some("ready".to_owned()),
            ready_generation: Some(GenerationId::parse("g1").expect("generation")),
            progress: RefreshProgress {
                completed: 2,
                total: 3,
                unit: RefreshProgressUnit::Files,
            },
            failure: Some(RefreshFailure {
                reason: RefreshFailureReason::TransportFailed,
                retry: RefreshRetry::Backoff { after_seconds: 30 },
            }),
        };
        let encoded = encode_refresh_response(&RefreshResponse::answered(
            9,
            RefreshDisposition::Started,
            status,
        ));
        let decoded: RefreshResponse = serde_json::from_slice(&encoded).expect("response");
        let RefreshOutcome::Answered(decoded) = decoded.outcome else {
            panic!("answered response")
        };
        assert_ne!(
            decoded.status.fetched_revision,
            decoded.status.ready_revision
        );
        assert_eq!(decoded.status.state, RefreshState::Failed);
    }
}
