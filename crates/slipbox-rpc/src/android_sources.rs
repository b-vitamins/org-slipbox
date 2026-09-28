//! Versioned Android source-catalog transport.
//!
//! Catalog mutation remains in Rust. Android supplies application-private paths
//! and a complete source record, then receives only a source whose published
//! generation was reopened and verified.

use serde::{Deserialize, Serialize};
use slipbox_core::{GenerationBinding, IndexStats, SourceRecord};

use crate::android::{ResponseEncoding, write_bounded};

pub const SOURCE_CATALOG_PROTOCOL_VERSION: u32 = 1;
pub const MAX_SOURCE_CATALOG_REQUEST_BYTES: usize = 32 * 1024;
pub const MAX_SOURCE_CATALOG_RESPONSE_BYTES: usize = 64 * 1024;
pub const MAX_SOURCE_CATALOG_PATH_BYTES: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "kebab-case", deny_unknown_fields)]
pub enum SourceCatalogRequest {
    Load {
        version: u32,
        catalog: String,
    },
    Verify {
        version: u32,
        catalog: String,
        expected_revision: u64,
        source: SourceRecord,
        store: String,
    },
    Commit {
        version: u32,
        catalog: String,
        expected_revision: u64,
        source: SourceRecord,
        store: String,
        generation: String,
    },
}

impl SourceCatalogRequest {
    #[must_use]
    pub const fn version(&self) -> u32 {
        match self {
            Self::Load { version, .. }
            | Self::Verify { version, .. }
            | Self::Commit { version, .. } => *version,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceCatalogResponse {
    pub version: u32,
    #[serde(flatten)]
    pub outcome: SourceCatalogOutcome,
}

impl SourceCatalogResponse {
    #[must_use]
    pub fn refused(reason: SourceCatalogFailureReason) -> Self {
        Self {
            version: SOURCE_CATALOG_PROTOCOL_VERSION,
            outcome: SourceCatalogOutcome::Refused { reason },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum SourceCatalogOutcome {
    Loaded {
        revision: u64,
        active_source: Option<SourceRecord>,
    },
    Ready {
        revision: u64,
        ready: Box<ReadySource>,
    },
    Refused {
        reason: SourceCatalogFailureReason,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadySource {
    pub source: SourceRecord,
    pub binding: GenerationBinding,
    pub revision: String,
    pub content_root: String,
    pub database: String,
    pub stats: IndexStats,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceCatalogFailureReason {
    UnsupportedVersion,
    MalformedRequest,
    OutOfBounds,
    CatalogUnavailable,
    CatalogConflict,
    SourceConflict,
    GenerationUnavailable,
    GenerationMismatch,
    StorageFailed,
    EncodingFailed,
    Panicked,
}

pub fn decode_source_catalog_request(
    bytes: &[u8],
) -> Result<SourceCatalogRequest, SourceCatalogFailureReason> {
    if bytes.len() > MAX_SOURCE_CATALOG_REQUEST_BYTES {
        return Err(SourceCatalogFailureReason::OutOfBounds);
    }
    let request: SourceCatalogRequest =
        serde_json::from_slice(bytes).map_err(|_| SourceCatalogFailureReason::MalformedRequest)?;
    if request.version() != SOURCE_CATALOG_PROTOCOL_VERSION {
        return Err(SourceCatalogFailureReason::UnsupportedVersion);
    }
    let paths: &[&str] = match &request {
        SourceCatalogRequest::Load { catalog, .. } => &[catalog],
        SourceCatalogRequest::Verify { catalog, store, .. } => &[catalog, store],
        SourceCatalogRequest::Commit {
            catalog,
            store,
            generation,
            ..
        } => {
            if generation.is_empty() || generation.len() > 64 {
                return Err(SourceCatalogFailureReason::MalformedRequest);
            }
            &[catalog, store]
        }
    };
    if paths.iter().any(|path| path.is_empty()) {
        return Err(SourceCatalogFailureReason::MalformedRequest);
    }
    if paths
        .iter()
        .any(|path| path.len() > MAX_SOURCE_CATALOG_PATH_BYTES)
    {
        return Err(SourceCatalogFailureReason::OutOfBounds);
    }
    Ok(request)
}

#[must_use]
pub fn encode_source_catalog_response(response: &SourceCatalogResponse) -> Vec<u8> {
    match write_bounded(response, MAX_SOURCE_CATALOG_RESPONSE_BYTES) {
        ResponseEncoding::Written(bytes) => bytes,
        ResponseEncoding::Oversized { .. } | ResponseEncoding::Failed { .. } => serde_json::to_vec(
            &SourceCatalogResponse::refused(SourceCatalogFailureReason::EncodingFailed),
        )
        .expect("the fixed source-catalog refusal encodes"),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn load_is_strict_versioned_and_bounded() {
        let request = serde_json::to_vec(&json!({
            "operation": "load",
            "version": SOURCE_CATALOG_PROTOCOL_VERSION,
            "catalog": "/private/source-catalog.json"
        }))
        .expect("request");
        assert!(matches!(
            decode_source_catalog_request(&request),
            Ok(SourceCatalogRequest::Load { .. })
        ));

        let unknown = serde_json::to_vec(&json!({
            "operation": "load",
            "version": SOURCE_CATALOG_PROTOCOL_VERSION,
            "catalog": "/private/source-catalog.json",
            "credential": "not admitted"
        }))
        .expect("request");
        assert_eq!(
            decode_source_catalog_request(&unknown),
            Err(SourceCatalogFailureReason::MalformedRequest)
        );

        assert_eq!(
            decode_source_catalog_request(&vec![b'x'; MAX_SOURCE_CATALOG_REQUEST_BYTES + 1]),
            Err(SourceCatalogFailureReason::OutOfBounds)
        );
    }

    #[test]
    fn responses_expose_only_closed_failure_categories() {
        let encoded = encode_source_catalog_response(&SourceCatalogResponse::refused(
            SourceCatalogFailureReason::GenerationMismatch,
        ));
        assert!(encoded.len() <= MAX_SOURCE_CATALOG_RESPONSE_BYTES);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&encoded).expect("response"),
            json!({
                "version": SOURCE_CATALOG_PROTOCOL_VERSION,
                "outcome": "refused",
                "reason": "generation-mismatch"
            })
        );
    }
}
