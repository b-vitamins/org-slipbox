//! Android ownership of source-catalog activation.

use std::fs;
use std::path::{Path, PathBuf};

use ring::digest::{SHA256, digest};
use slipbox_core::{GenerationId, SourceChange, SourceId, SourceRecord};
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
        Ok(wire::SourceCatalogRequest::Activate {
            catalog,
            expected_revision,
            source,
            store,
            ..
        }) => activate(
            PathBuf::from(catalog),
            expected_revision,
            source,
            PathBuf::from(store),
        ),
        Ok(wire::SourceCatalogRequest::Replace {
            catalog,
            expected_revision,
            previous,
            source,
            store,
            generation,
            ..
        }) => replace(
            PathBuf::from(catalog),
            expected_revision,
            previous,
            source,
            PathBuf::from(store),
            &generation,
        ),
        Ok(wire::SourceCatalogRequest::Remove {
            catalog,
            expected_revision,
            source,
            ..
        }) => remove(PathBuf::from(catalog), expected_revision, source),
        Ok(wire::SourceCatalogRequest::Purge {
            private_root,
            source,
            ..
        }) => purge(PathBuf::from(private_root), &source),
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
            sources: Vec::new(),
            active_source: None,
        },
        Ok(Some(stored)) => loaded(stored.revision(), stored.catalog()),
        Err(failure) => refused(map_catalog_failure(&failure)),
    }
}

fn activate(
    catalog_path: PathBuf,
    expected_revision: u64,
    source: SourceRecord,
    generation_store: PathBuf,
) -> wire::SourceCatalogOutcome {
    let ready = match verified_ready(&source, generation_store, None) {
        Ok(ready) => ready,
        Err(reason) => return refused(reason),
    };
    let store = SourceCatalogStore::at(catalog_path);
    let stored = match store.load() {
        Ok(Some(stored)) => stored,
        Ok(None) => return refused(wire::SourceCatalogFailureReason::CatalogConflict),
        Err(failure) => return refused(map_catalog_failure(&failure)),
    };
    if stored.revision().get() != expected_revision
        || stored.catalog().get(source.id()) != Some(&source)
    {
        return refused(wire::SourceCatalogFailureReason::CatalogConflict);
    }
    if stored.catalog().active_id() == Some(source.id()) {
        return wire::SourceCatalogOutcome::Ready {
            revision: stored.revision().get(),
            ready: Box::new(ready),
        };
    }
    let revision = stored.revision();
    let mut catalog = stored.into_catalog();
    if catalog.set_active(source.id()).is_err() {
        return refused(wire::SourceCatalogFailureReason::SourceConflict);
    }
    match store.save(revision, &catalog) {
        Ok(revision) => wire::SourceCatalogOutcome::Ready {
            revision: revision.get(),
            ready: Box::new(ready),
        },
        Err(failure) => refused(map_catalog_failure(&failure)),
    }
}

fn replace(
    catalog_path: PathBuf,
    expected_revision: u64,
    previous: SourceRecord,
    source: SourceRecord,
    generation_store: PathBuf,
    generation: &str,
) -> wire::SourceCatalogOutcome {
    let ready = match verified_ready(&source, generation_store, Some(generation)) {
        Ok(ready) => ready,
        Err(reason) => return refused(reason),
    };
    if previous.id() != source.id() {
        return refused(wire::SourceCatalogFailureReason::InvalidChange);
    }
    let store = SourceCatalogStore::at(catalog_path);
    let stored = match store.load() {
        Ok(Some(stored)) => stored,
        Ok(None) => return refused(wire::SourceCatalogFailureReason::CatalogConflict),
        Err(failure) => return refused(map_catalog_failure(&failure)),
    };
    if stored.revision().get() != expected_revision
        || stored.catalog().get(previous.id()) != Some(&previous)
    {
        return refused(wire::SourceCatalogFailureReason::CatalogConflict);
    }
    let revision = stored.revision();
    let mut catalog = stored.into_catalog();
    let changed = match changed_configuration(&mut catalog, &previous, &source) {
        Ok(changed) => changed,
        Err(reason) => return refused(reason),
    };
    if !changed {
        return wire::SourceCatalogOutcome::Ready {
            revision: revision.get(),
            ready: Box::new(ready),
        };
    }
    match store.save(revision, &catalog) {
        Ok(revision) => wire::SourceCatalogOutcome::Ready {
            revision: revision.get(),
            ready: Box::new(ready),
        },
        Err(failure) => refused(map_catalog_failure(&failure)),
    }
}

fn changed_configuration(
    catalog: &mut SourceCatalog,
    previous: &SourceRecord,
    source: &SourceRecord,
) -> Result<bool, wire::SourceCatalogFailureReason> {
    let mut expected = previous.clone();
    let mut changed = false;
    if expected.display_name() != source.display_name() {
        catalog
            .apply(
                previous.id(),
                SourceChange::Relabel(source.display_name().clone()),
            )
            .map_err(|_| wire::SourceCatalogFailureReason::InvalidChange)?;
        expected = catalog
            .get(previous.id())
            .cloned()
            .ok_or(wire::SourceCatalogFailureReason::SourceConflict)?;
        changed = true;
    }
    if expected.branch() != source.branch() {
        catalog
            .apply(
                previous.id(),
                SourceChange::TrackBranch(source.branch().clone()),
            )
            .map_err(|_| wire::SourceCatalogFailureReason::InvalidChange)?;
        expected = catalog
            .get(previous.id())
            .cloned()
            .ok_or(wire::SourceCatalogFailureReason::SourceConflict)?;
        changed = true;
    }
    if expected.notes_folder() != source.notes_folder() {
        catalog
            .apply(
                previous.id(),
                SourceChange::TrackNotesFolder(source.notes_folder().clone()),
            )
            .map_err(|_| wire::SourceCatalogFailureReason::InvalidChange)?;
        expected = catalog
            .get(previous.id())
            .cloned()
            .ok_or(wire::SourceCatalogFailureReason::SourceConflict)?;
        changed = true;
    }
    if expected != *source {
        return Err(wire::SourceCatalogFailureReason::InvalidChange);
    }
    Ok(changed)
}

fn remove(
    catalog_path: PathBuf,
    expected_revision: u64,
    source: SourceRecord,
) -> wire::SourceCatalogOutcome {
    let store = SourceCatalogStore::at(catalog_path);
    let stored = match store.load() {
        Ok(Some(stored)) => stored,
        Ok(None) => return refused(wire::SourceCatalogFailureReason::CatalogConflict),
        Err(failure) => return refused(map_catalog_failure(&failure)),
    };
    if stored.revision().get() != expected_revision
        || stored.catalog().get(source.id()) != Some(&source)
    {
        return refused(wire::SourceCatalogFailureReason::CatalogConflict);
    }
    let revision = stored.revision();
    let mut catalog = stored.into_catalog();
    if catalog.remove(source.id()).is_err() {
        return refused(wire::SourceCatalogFailureReason::SourceConflict);
    }
    match store.save(revision, &catalog) {
        Ok(revision) => wire::SourceCatalogOutcome::Removed {
            revision: revision.get(),
            sources: catalog.sources().to_vec(),
            active_source: catalog.active().cloned(),
        },
        Err(failure) => refused(map_catalog_failure(&failure)),
    }
}

fn purge(private_root: PathBuf, source: &SourceId) -> wire::SourceCatalogOutcome {
    match purge_source_cache(&private_root, source) {
        Ok((removed_files, removed_bytes)) => wire::SourceCatalogOutcome::Purged {
            removed_files,
            removed_bytes,
        },
        Err(()) => refused(wire::SourceCatalogFailureReason::CleanupFailed),
    }
}

fn purge_source_cache(private_root: &Path, source: &SourceId) -> Result<(u64, u64), ()> {
    let private_root = verified_directory(private_root)?;
    let refresh = private_root.join("refresh");
    if !refresh.exists() {
        return Ok((0, 0));
    }
    let refresh = verified_directory(&refresh)?;
    let source_root = refresh.join(source_cache_key(source));
    if !source_root.exists() {
        return Ok((0, 0));
    }
    let metadata = fs::symlink_metadata(&source_root).map_err(|_| ())?;
    if !metadata.is_dir() || fs::canonicalize(&source_root).map_err(|_| ())? != source_root {
        return Err(());
    }
    let removed = remove_tree(&source_root)?;
    sync_directory(&refresh)?;
    Ok(removed)
}

fn verified_directory(path: &Path) -> Result<PathBuf, ()> {
    let absolute = path.canonicalize().map_err(|_| ())?;
    let metadata = fs::symlink_metadata(path).map_err(|_| ())?;
    if !metadata.is_dir() || absolute != path {
        return Err(());
    }
    Ok(absolute)
}

fn remove_tree(path: &Path) -> Result<(u64, u64), ()> {
    let metadata = fs::symlink_metadata(path).map_err(|_| ())?;
    if metadata.is_dir() {
        let mut removed = (0_u64, 0_u64);
        for entry in fs::read_dir(path).map_err(|_| ())? {
            let child = entry.map_err(|_| ())?.path();
            let count = remove_tree(&child)?;
            removed.0 = removed.0.saturating_add(count.0);
            removed.1 = removed.1.saturating_add(count.1);
        }
        fs::remove_dir(path).map_err(|_| ())?;
        Ok(removed)
    } else {
        let bytes = metadata.len();
        fs::remove_file(path).map_err(|_| ())?;
        Ok((1, bytes))
    }
}

fn sync_directory(path: &Path) -> Result<(), ()> {
    fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| ())
}

fn source_cache_key(source: &SourceId) -> String {
    let mut input = b"slipbox.refresh.source.1\0".to_vec();
    let bytes = source.as_str().as_bytes();
    input.push(u8::try_from(bytes.len()).expect("a source ID is bounded"));
    input.extend_from_slice(bytes);
    let value = digest(&SHA256, &input);
    let mut encoded = String::with_capacity(64);
    for byte in value.as_ref() {
        use std::fmt::Write;
        write!(&mut encoded, "{byte:02x}").expect("writing to a string cannot fail");
    }
    encoded
}

fn loaded(revision: CatalogRevision, catalog: &SourceCatalog) -> wire::SourceCatalogOutcome {
    wire::SourceCatalogOutcome::Loaded {
        revision: revision.get(),
        sources: catalog.sources().to_vec(),
        active_source: catalog.active().cloned(),
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
    use slipbox_core::SourceRecord;
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
                sources: Vec::new(),
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

    #[test]
    fn changed_configuration_accepts_only_identity_preserving_fields() {
        let previous = source("main", "", "https://example.com/notes.git");
        let replacement = source("next", "org", "https://example.com/notes.git");
        let mut catalog = SourceCatalog::new();
        catalog.add(previous.clone()).expect("source");

        assert_eq!(
            changed_configuration(&mut catalog, &previous, &replacement),
            Ok(true)
        );
        assert_eq!(catalog.get(previous.id()), Some(&replacement));

        let relocated = source("next", "org", "https://example.com/other.git");
        assert_eq!(
            changed_configuration(&mut catalog, &replacement, &relocated),
            Err(wire::SourceCatalogFailureReason::InvalidChange)
        );
        assert_eq!(catalog.get(previous.id()), Some(&replacement));
    }

    #[test]
    fn removal_checks_the_observed_record_and_clears_an_active_selection() {
        let root = tempdir().expect("root");
        let catalog_path = root.path().join("sources.json");
        let record = source("main", "", "https://example.com/notes.git");
        let mut catalog = SourceCatalog::new();
        catalog.add(record.clone()).expect("source");
        let revision = SourceCatalogStore::at(&catalog_path)
            .save(CatalogRevision::ABSENT, &catalog)
            .expect("catalog");

        let outcome = remove(catalog_path.clone(), revision.get(), record);
        let wire::SourceCatalogOutcome::Removed {
            revision,
            sources,
            active_source,
        } = outcome
        else {
            panic!("the source was not removed: {outcome:?}");
        };
        assert_eq!(revision, 2);
        assert!(sources.is_empty());
        assert!(active_source.is_none());
        assert!(
            SourceCatalogStore::at(catalog_path)
                .load()
                .expect("catalog")
                .expect("stored")
                .catalog()
                .is_empty()
        );
    }

    #[test]
    fn replacement_without_a_matching_ready_generation_leaves_catalog_untouched() {
        let root = tempdir().expect("root");
        let catalog_path = root.path().join("sources.json");
        let previous = source("main", "", "https://example.com/notes.git");
        let replacement = source("next", "org", "https://example.com/notes.git");
        let mut catalog = SourceCatalog::new();
        catalog.add(previous.clone()).expect("source");
        let revision = SourceCatalogStore::at(&catalog_path)
            .save(CatalogRevision::ABSENT, &catalog)
            .expect("catalog");

        assert_eq!(
            replace(
                catalog_path.clone(),
                revision.get(),
                previous.clone(),
                replacement,
                root.path().join("missing-store"),
                "generation-next",
            ),
            refused(wire::SourceCatalogFailureReason::GenerationUnavailable),
        );
        let stored = SourceCatalogStore::at(catalog_path)
            .load()
            .expect("catalog")
            .expect("stored");
        assert_eq!(stored.revision(), revision);
        assert_eq!(stored.catalog().active(), Some(&previous));
    }

    #[test]
    fn cache_purge_is_source_scoped_and_reports_removed_bytes() {
        let root = tempdir().expect("root");
        let record = source("main", "", "https://example.com/notes.git");
        let source_root = root
            .path()
            .join("refresh")
            .join(source_cache_key(record.id()));
        let other = root.path().join("refresh").join("other-source");
        fs::create_dir_all(source_root.join("configuration/store")).expect("source cache");
        fs::create_dir_all(&other).expect("other cache");
        fs::write(source_root.join("configuration/store/index"), b"derived").expect("cache");
        fs::write(other.join("keep"), b"keep").expect("other cache");

        assert_eq!(purge_source_cache(root.path(), record.id()), Ok((1, 7)));
        assert!(!source_root.exists());
        assert!(other.join("keep").exists());
        assert_eq!(purge_source_cache(root.path(), record.id()), Ok((0, 0)));
    }

    #[test]
    fn cache_key_matches_the_android_storage_contract() {
        let record = source("main", "", "https://example.com/notes.git");

        assert_eq!(
            source_cache_key(record.id()),
            "da037c8e07ff443a2145a7b351974f36142a4dec6301bfb77674d09377729cf1",
        );
    }

    #[cfg(unix)]
    #[test]
    fn cache_purge_refuses_a_link_at_the_source_boundary() {
        use std::os::unix::fs::symlink;

        let root = tempdir().expect("root");
        let outside = tempdir().expect("outside");
        let record = source("main", "", "https://example.com/notes.git");
        let refresh = root.path().join("refresh");
        fs::create_dir(&refresh).expect("refresh");
        symlink(outside.path(), refresh.join(source_cache_key(record.id()))).expect("link");

        assert_eq!(purge_source_cache(root.path(), record.id()), Err(()));
        assert!(outside.path().exists());
    }

    fn source(branch: &str, notes_folder: &str, remote: &str) -> SourceRecord {
        serde_json::from_value(json!({
            "id": "0123456789abcdef0123456789abcdef",
            "display_name": "Notes",
            "provider": "generic_https",
            "visibility": "public",
            "remote": remote,
            "branch": branch,
            "notes_folder": notes_folder
        }))
        .expect("source")
    }
}
