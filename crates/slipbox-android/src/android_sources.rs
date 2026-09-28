//! Android ownership of source-catalog activation.

use std::path::PathBuf;

use slipbox_core::GenerationId;
use slipbox_rpc::android_sources as wire;
use slipbox_sources::{
    CatalogRevision, SourceCatalog, SourceCatalogStore, SourceCatalogStoreError,
};
use slipbox_sync::GenerationStore;

#[must_use]
pub fn serve_source_catalog(request: &[u8]) -> Vec<u8> {
    let outcome = match wire::decode_source_catalog_request(request) {
        Ok(wire::SourceCatalogRequest::Load { catalog, .. }) => load(PathBuf::from(catalog)),
        Ok(wire::SourceCatalogRequest::Verify {
            catalog,
            expected_revision,
            source,
            store,
            ..
        }) => verify(
            PathBuf::from(catalog),
            expected_revision,
            source,
            PathBuf::from(store),
        ),
        Ok(wire::SourceCatalogRequest::Commit {
            catalog,
            expected_revision,
            source,
            store,
            generation,
            ..
        }) => commit(
            PathBuf::from(catalog),
            expected_revision,
            source,
            PathBuf::from(store),
            &generation,
        ),
        Err(reason) => wire::SourceCatalogOutcome::Refused { reason },
    };
    wire::encode_source_catalog_response(&wire::SourceCatalogResponse {
        version: wire::SOURCE_CATALOG_PROTOCOL_VERSION,
        outcome,
    })
}

fn verify(
    catalog_path: PathBuf,
    expected_revision: u64,
    source: slipbox_core::SourceRecord,
    generation_store: PathBuf,
) -> wire::SourceCatalogOutcome {
    let store = SourceCatalogStore::at(catalog_path);
    let stored = match store.load() {
        Ok(Some(stored)) => stored,
        Ok(None) => return refused(wire::SourceCatalogFailureReason::CatalogConflict),
        Err(failure) => return refused(map_catalog_failure(&failure)),
    };
    if stored.revision().get() != expected_revision {
        return refused(wire::SourceCatalogFailureReason::CatalogConflict);
    }
    if stored.catalog().active() != Some(&source) {
        return refused(wire::SourceCatalogFailureReason::SourceConflict);
    }
    match verified_ready(&source, generation_store, None) {
        Ok(ready) => wire::SourceCatalogOutcome::Ready {
            revision: stored.revision().get(),
            ready: Box::new(ready),
        },
        Err(reason) => refused(reason),
    }
}

fn load(catalog_path: PathBuf) -> wire::SourceCatalogOutcome {
    let store = SourceCatalogStore::at(catalog_path);
    match store.load() {
        Ok(None) => wire::SourceCatalogOutcome::Loaded {
            revision: CatalogRevision::ABSENT.get(),
            active_source: None,
        },
        Ok(Some(stored)) => wire::SourceCatalogOutcome::Loaded {
            revision: stored.revision().get(),
            active_source: stored.catalog().active().cloned(),
        },
        Err(failure) => refused(map_catalog_failure(&failure)),
    }
}

fn commit(
    catalog_path: PathBuf,
    expected_revision: u64,
    source: slipbox_core::SourceRecord,
    generation_store: PathBuf,
    generation: &str,
) -> wire::SourceCatalogOutcome {
    let ready = match verified_ready(&source, generation_store, Some(generation)) {
        Ok(ready) => ready,
        Err(reason) => return refused(reason),
    };

    let store = SourceCatalogStore::at(catalog_path);
    let loaded = match store.load() {
        Ok(loaded) => loaded,
        Err(failure) => return refused(map_catalog_failure(&failure)),
    };
    let found_revision = loaded
        .as_ref()
        .map_or(CatalogRevision::ABSENT, |stored| stored.revision());
    if found_revision.get() != expected_revision {
        return refused(wire::SourceCatalogFailureReason::CatalogConflict);
    }
    let mut catalog = loaded.map_or_else(SourceCatalog::new, |stored| stored.into_catalog());
    match catalog.get(source.id()) {
        Some(existing) if existing != &source => {
            return refused(wire::SourceCatalogFailureReason::SourceConflict);
        }
        Some(_) => {}
        None => {
            if catalog.add(source.clone()).is_err() {
                return refused(wire::SourceCatalogFailureReason::SourceConflict);
            }
        }
    }
    if catalog.set_active(source.id()).is_err() {
        return refused(wire::SourceCatalogFailureReason::SourceConflict);
    }
    match store.save(found_revision, &catalog) {
        Ok(revision) => wire::SourceCatalogOutcome::Ready {
            revision: revision.get(),
            ready: Box::new(ready),
        },
        Err(failure) => refused(map_catalog_failure(&failure)),
    }
}

fn verified_ready(
    source: &slipbox_core::SourceRecord,
    store: PathBuf,
    generation: Option<&str>,
) -> Result<wire::ReadySource, wire::SourceCatalogFailureReason> {
    let expected = generation
        .map(GenerationId::parse)
        .transpose()
        .map_err(|_| wire::SourceCatalogFailureReason::GenerationMismatch)?;
    let generation_store = GenerationStore::open(source.id().clone(), store)
        .map_err(|_| wire::SourceCatalogFailureReason::GenerationUnavailable)?;
    let lease = generation_store
        .lease()
        .map_err(|_| wire::SourceCatalogFailureReason::GenerationUnavailable)?
        .ok_or(wire::SourceCatalogFailureReason::GenerationUnavailable)?;
    if lease.source_record() != source
        || expected
            .as_ref()
            .is_some_and(|expected| &lease.binding().generation != expected)
    {
        return Err(wire::SourceCatalogFailureReason::GenerationMismatch);
    }
    let content_root = lease
        .content_root()
        .to_str()
        .ok_or(wire::SourceCatalogFailureReason::StorageFailed)?
        .to_owned();
    let database = lease
        .database()
        .to_str()
        .ok_or(wire::SourceCatalogFailureReason::StorageFailed)?
        .to_owned();
    Ok(wire::ReadySource {
        source: source.clone(),
        binding: lease.binding().clone(),
        revision: lease.revision().to_owned(),
        content_root,
        database,
        stats: lease.stats().clone(),
    })
}

fn refused(reason: wire::SourceCatalogFailureReason) -> wire::SourceCatalogOutcome {
    wire::SourceCatalogOutcome::Refused { reason }
}

fn map_catalog_failure(failure: &SourceCatalogStoreError) -> wire::SourceCatalogFailureReason {
    match failure {
        SourceCatalogStoreError::UnsupportedVersion { .. } => {
            wire::SourceCatalogFailureReason::UnsupportedVersion
        }
        SourceCatalogStoreError::StaleWrite { .. } => {
            wire::SourceCatalogFailureReason::CatalogConflict
        }
        SourceCatalogStoreError::Malformed { .. } | SourceCatalogStoreError::Invalid { .. } => {
            wire::SourceCatalogFailureReason::CatalogUnavailable
        }
        _ => wire::SourceCatalogFailureReason::StorageFailed,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn absent_catalog_loads_as_revision_zero_without_creating_it() {
        let root = tempdir().expect("root");
        let catalog = root.path().join("sources.json");
        let request = serde_json::to_vec(&json!({
            "operation": "load",
            "version": wire::SOURCE_CATALOG_PROTOCOL_VERSION,
            "catalog": catalog
        }))
        .expect("request");

        let response: wire::SourceCatalogResponse =
            serde_json::from_slice(&serve_source_catalog(&request)).expect("response");
        assert_eq!(
            response.outcome,
            wire::SourceCatalogOutcome::Loaded {
                revision: 0,
                active_source: None,
            }
        );
        assert!(!catalog.exists());
    }

    #[test]
    fn malformed_requests_are_closed_refusals() {
        let response: wire::SourceCatalogResponse =
            serde_json::from_slice(&serve_source_catalog(b"{}")).expect("response");
        assert_eq!(
            response.outcome,
            wire::SourceCatalogOutcome::Refused {
                reason: wire::SourceCatalogFailureReason::MalformedRequest,
            }
        );
    }

    #[test]
    fn verification_refuses_an_absent_catalog_before_opening_a_generation() {
        let root = tempdir().expect("root");
        let request = serde_json::to_vec(&json!({
            "operation": "verify",
            "version": wire::SOURCE_CATALOG_PROTOCOL_VERSION,
            "catalog": root.path().join("sources.json"),
            "expected_revision": 0,
            "source": {
                "id": "0123456789abcdef0123456789abcdef",
                "display_name": "Notes",
                "provider": "generic_https",
                "visibility": "public",
                "remote": "https://example.com/notes.git",
                "branch": "main",
                "notes_folder": ""
            },
            "store": root.path().join("store")
        }))
        .expect("request");

        let response: wire::SourceCatalogResponse =
            serde_json::from_slice(&serve_source_catalog(&request)).expect("response");
        assert_eq!(
            response.outcome,
            wire::SourceCatalogOutcome::Refused {
                reason: wire::SourceCatalogFailureReason::CatalogConflict,
            }
        );
    }
}
