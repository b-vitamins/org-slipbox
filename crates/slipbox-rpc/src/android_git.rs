//! Versioned Android Git transport and snapshot DTOs.
//!
//! Credentials are deliberately absent. JNI carries a credential, when needed,
//! in a separate byte array whose lifetime is one transport call.

use serde::{Deserialize, Serialize};

use crate::android::{ResponseEncoding, write_bounded};

pub const GIT_PROTOCOL_VERSION: u32 = 1;
pub const MAX_GIT_REQUEST_BYTES: usize = 16 * 1024;
pub const MAX_GIT_RESPONSE_BYTES: usize = 96 * 1024;
pub const MAX_GIT_TEXT_BYTES: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitRequest {
    pub version: u32,
    pub operation: i64,
    pub remote: String,
    pub branch: String,
    pub repository: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitMaterializeRequest {
    pub version: u32,
    pub operation: i64,
    pub source: String,
    pub repository: String,
    pub revision: String,
    pub notes_folder: String,
    pub snapshot: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitResponse {
    pub version: u32,
    #[serde(flatten)]
    pub outcome: GitOutcome,
}

impl GitResponse {
    #[must_use]
    pub fn new(outcome: GitOutcome) -> Self {
        Self {
            version: GIT_PROTOCOL_VERSION,
            outcome,
        }
    }

    #[must_use]
    pub fn refused(reason: GitRefusalReason) -> Self {
        Self::new(GitOutcome::Refused(GitRefusal { reason }))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum GitOutcome {
    Fetched(GitFetched),
    Materialized(GitMaterialized),
    Refused(GitRefusal),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitFetched {
    pub operation: i64,
    pub disposition: GitDisposition,
    pub revision: String,
    pub received_objects: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitMaterialized {
    pub operation: i64,
    pub disposition: GitSnapshotDisposition,
    pub revision: String,
    pub entries: u64,
    pub files: u64,
    pub org_files: u64,
    pub assets: u64,
    pub bytes: u64,
    pub diagnostics: Vec<GitSnapshotDiagnostic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GitSnapshotDisposition {
    Created,
    Existing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitSnapshotDiagnostic {
    pub path: String,
    pub reason: GitSnapshotDiagnosticReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GitSnapshotDiagnosticReason {
    EncryptedOrg,
    UnsupportedEncoding,
    UnsupportedFormat,
    OversizedInput,
    Submodule,
    LfsPointer,
    ExternalFilter,
    SymlinkEscapes,
    SymlinkCycle,
    SymlinkUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GitDisposition {
    Cloned,
    Updated,
    Unchanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitRefusal {
    pub reason: GitRefusalReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GitRefusalReason {
    UnsupportedVersion,
    MalformedRequest,
    OutOfBounds,
    OperationConflict,
    OperationsExhausted,
    DestinationRefused,
    DestinationOccupied,
    StorageBoundary,
    CredentialRefused,
    Cancelled,
    StorageFailed,
    RepositoryInvalid,
    BranchUnavailable,
    TransportFailed,
    OwnershipMismatch,
    RevisionRefused,
    RevisionUnavailable,
    NotesFolderUnavailable,
    UnsafePath,
    ObjectInvalid,
    InputExhausted,
    DiagnosticsExhausted,
    EncodingFailed,
    Panicked,
}

/// Decode only a bounded, current request. Struct deserialization rejects
/// duplicate and unknown fields before native Git sees anything.
pub fn decode_git_request(bytes: &[u8]) -> Result<GitRequest, GitRefusalReason> {
    if bytes.len() > MAX_GIT_REQUEST_BYTES {
        return Err(GitRefusalReason::OutOfBounds);
    }
    let request: GitRequest =
        serde_json::from_slice(bytes).map_err(|_| GitRefusalReason::MalformedRequest)?;
    if request.version != GIT_PROTOCOL_VERSION {
        return Err(GitRefusalReason::UnsupportedVersion);
    }
    if request.operation <= 0
        || request.remote.is_empty()
        || request.branch.is_empty()
        || request.repository.is_empty()
    {
        return Err(GitRefusalReason::MalformedRequest);
    }
    if [
        request.remote.len(),
        request.branch.len(),
        request.repository.len(),
    ]
    .into_iter()
    .any(|length| length > MAX_GIT_TEXT_BYTES)
    {
        return Err(GitRefusalReason::OutOfBounds);
    }
    Ok(request)
}

/// Decode one bounded materialization request. An empty notes folder names the
/// repository root; every other text field must be present.
pub fn decode_materialize_request(bytes: &[u8]) -> Result<GitMaterializeRequest, GitRefusalReason> {
    if bytes.len() > MAX_GIT_REQUEST_BYTES {
        return Err(GitRefusalReason::OutOfBounds);
    }
    let request: GitMaterializeRequest =
        serde_json::from_slice(bytes).map_err(|_| GitRefusalReason::MalformedRequest)?;
    if request.version != GIT_PROTOCOL_VERSION {
        return Err(GitRefusalReason::UnsupportedVersion);
    }
    if request.operation <= 0
        || request.source.is_empty()
        || request.repository.is_empty()
        || request.revision.is_empty()
        || request.snapshot.is_empty()
    {
        return Err(GitRefusalReason::MalformedRequest);
    }
    if [
        request.source.len(),
        request.repository.len(),
        request.revision.len(),
        request.notes_folder.len(),
        request.snapshot.len(),
    ]
    .into_iter()
    .any(|length| length > MAX_GIT_TEXT_BYTES)
    {
        return Err(GitRefusalReason::OutOfBounds);
    }
    Ok(request)
}

#[must_use]
pub fn encode_git_response(response: &GitResponse) -> Vec<u8> {
    match write_bounded(response, MAX_GIT_RESPONSE_BYTES) {
        ResponseEncoding::Written(bytes) => bytes,
        ResponseEncoding::Oversized { .. } | ResponseEncoding::Failed { .. } => {
            serde_json::to_vec(&GitResponse::refused(GitRefusalReason::EncodingFailed))
                .expect("the fixed Git encoding refusal is serializable")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_contract_is_versioned_bounded_and_closed() {
        let valid = br#"{"version":1,"operation":7,"remote":"https://github.com/o/r.git","branch":"main","repository":"/private/r.git"}"#;
        assert_eq!(decode_git_request(valid).expect("valid").operation, 7);

        for invalid in [
            br#"{"version":2,"operation":7,"remote":"https://github.com/o/r.git","branch":"main","repository":"/private/r.git"}"#.as_slice(),
            br#"{"version":1,"operation":0,"remote":"https://github.com/o/r.git","branch":"main","repository":"/private/r.git"}"#.as_slice(),
            br#"{"version":1,"operation":7,"remote":"https://github.com/o/r.git","branch":"main","repository":"/private/r.git","credential":"secret"}"#.as_slice(),
        ] {
            assert!(decode_git_request(invalid).is_err());
        }
    }

    #[test]
    fn response_never_contains_request_or_credential_material() {
        let encoded = encode_git_response(&GitResponse::refused(GitRefusalReason::TransportFailed));
        let text = String::from_utf8(encoded).expect("UTF-8 JSON");
        assert_eq!(
            text,
            r#"{"version":1,"outcome":"refused","reason":"transport-failed"}"#
        );
    }

    #[test]
    fn materialization_contract_is_versioned_bounded_and_closed() {
        let valid = br#"{"version":1,"operation":8,"source":"0123456789abcdef0123456789abcdef","repository":"/private/r.git","revision":"0123456789abcdef0123456789abcdef01234567","notes_folder":"notes","snapshot":"/private/snapshots/one"}"#;
        let decoded = decode_materialize_request(valid).expect("valid materialization");
        assert_eq!(decoded.operation, 8);
        assert_eq!(decoded.notes_folder, "notes");

        for invalid in [
            br#"{"version":2,"operation":8,"source":"0123456789abcdef0123456789abcdef","repository":"/private/r.git","revision":"0123456789abcdef0123456789abcdef01234567","notes_folder":"","snapshot":"/private/snapshots/one"}"#.as_slice(),
            br#"{"version":1,"operation":0,"source":"0123456789abcdef0123456789abcdef","repository":"/private/r.git","revision":"0123456789abcdef0123456789abcdef01234567","notes_folder":"","snapshot":"/private/snapshots/one"}"#.as_slice(),
            br#"{"version":1,"operation":8,"source":"0123456789abcdef0123456789abcdef","repository":"/private/r.git","revision":"0123456789abcdef0123456789abcdef01234567","notes_folder":"","snapshot":"/private/snapshots/one","extra":true}"#.as_slice(),
        ] {
            assert!(decode_materialize_request(invalid).is_err());
        }
    }
}
