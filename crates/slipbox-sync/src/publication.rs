mod recovery;

pub use recovery::*;

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use slipbox_core::{
    GenerationBinding, GenerationId, IndexStats, NotesFolder, SourceId, SourceRecord,
};
use slipbox_git::{
    MAX_REPOSITORY_PATH_BYTES, MAX_SNAPSHOT_PATH_BYTES, SNAPSHOT_CONTENT_DIRECTORY,
    SnapshotOutcome, inspect_snapshot,
};
use tempfile::NamedTempFile;
use thiserror::Error;

use crate::{
    STAGED_INDEX_DATABASE_FILE, StageIndexError, StagedIndexOutcome, inspect_staged_index,
};

pub const GENERATION_FORMAT_VERSION: u32 = 3;
pub const GENERATION_STORE_FORMAT_VERSION: u32 = 1;
pub const GENERATION_STORE_MANIFEST_FILE: &str = "generation-store.json";
pub const GENERATIONS_DIRECTORY: &str = "generations";
pub const GENERATION_CANDIDATES_DIRECTORY: &str = "candidates";
pub const ACTIVE_GENERATION_FILE: &str = "active-generation.json";
pub const RETAINED_GENERATION_FILE: &str = "retained-generation.json";
pub const GENERATION_MANIFEST_FILE: &str = "generation.json";
pub const GENERATION_SOURCE_DIRECTORY: &str = "source";
pub const GENERATION_INDEX_DIRECTORY: &str = "index";
pub const GENERATION_READER_LOCK_FILE: &str = ".generation-readers.lock";

const PUBLICATION_LOCK_FILE: &str = ".generation-publication.lock";
const RECOVERY_LOCK_FILE: &str = ".generation-recovery.lock";
pub(crate) const GENERATION_STAGE_PREFIX: &str = ".generation-stage-";
pub(crate) const MAX_GENERATION_MANIFEST_BYTES: u64 = 64 * 1024;

/// One source-owned generation store. It carries no ambient or global root.
#[derive(Debug, Clone)]
pub struct GenerationStore {
    pub(crate) source: SourceId,
    pub(crate) root: PathBuf,
}

/// A sealed generation retained for the complete lifetime of a reader.
#[derive(Debug, Clone)]
pub struct GenerationLease {
    record: Arc<GenerationRecord>,
    _reader_lock: Arc<File>,
}

/// One open content descriptor that keeps its generation leased until the
/// descriptor itself is released.
#[derive(Debug)]
pub struct GenerationContent {
    file: File,
    _reader_lock: Arc<File>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GenerationRecord {
    pub(crate) binding: GenerationBinding,
    pub(crate) source_record: SourceRecord,
    pub(crate) revision: String,
    pub(crate) previous_revision: Option<String>,
    pub(crate) notes_folder: NotesFolder,
    pub(crate) content_root: PathBuf,
    pub(crate) index_directory: PathBuf,
    pub(crate) database: PathBuf,
    pub(crate) reader_lock: PathBuf,
    pub(crate) index_schema: i32,
    pub(crate) stats: IndexStats,
}

/// Exact candidates and the active generation the caller observed before work
/// began. Activation is compare-and-swap against that observation.
#[derive(Debug, Clone)]
pub struct PublishGenerationRequest {
    source_record: SourceRecord,
    binding: GenerationBinding,
    expected_active: Option<GenerationId>,
    snapshot: SnapshotOutcome,
    index: StagedIndexOutcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicationDisposition {
    Published,
    ActivatedExisting,
    AlreadyActive,
}

#[derive(Debug, Clone)]
pub struct PublicationOutcome {
    pub disposition: PublicationDisposition,
    pub generation: GenerationLease,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublicationStage {
    Preparing,
    SourceSealed,
    IndexSealed,
    ManifestSealed,
    Activating,
    Retained,
    Complete,
}

/// Work that may be newer than the ready generation. Fetching has no revision
/// until the transport answers; every later stage names the exact revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "stage", rename_all = "kebab-case")]
pub enum InFlightGeneration {
    Fetching,
    Materializing { revision: String },
    Indexing { revision: String },
    Publishing { revision: String },
}

/// Honest source freshness. `fetched_revision` never implies readability;
/// `ready_revision` and `ready_generation` come only from the active manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceFreshness {
    pub source: SourceId,
    pub fetched_revision: Option<String>,
    pub ready_revision: Option<String>,
    pub ready_generation: Option<GenerationId>,
    pub in_flight: Option<InFlightGeneration>,
}

/// Closed publication failures. Paths and repository contents never enter a
/// diagnostic value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PublicationError {
    #[error("the generation store request is not admitted")]
    RequestRefused,
    #[error("the generation store is unavailable")]
    StoreUnavailable,
    #[error("the generation store belongs to another source")]
    OwnershipMismatch,
    #[error("another publication is already in progress")]
    PublicationInProgress,
    #[error("generation recovery is in progress")]
    RecoveryInProgress,
    #[error("the publication candidates are unavailable")]
    CandidateUnavailable,
    #[error("the publication candidates do not describe one generation")]
    CandidateMismatch,
    #[error("the candidate index requires an explicit rebuild")]
    RebuildRequired,
    #[error("the active generation changed before publication")]
    ActiveGenerationChanged,
    #[error("the generation destination is occupied")]
    GenerationOccupied,
    #[error("the publication was cancelled")]
    Cancelled,
    #[error("the generation could not be stored")]
    StorageFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GenerationStoreManifest {
    version: u32,
    source: SourceId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GenerationManifest {
    pub(crate) version: u32,
    pub(crate) source_record: SourceRecord,
    pub(crate) binding: GenerationBinding,
    pub(crate) expected_active: Option<GenerationId>,
    pub(crate) revision: String,
    pub(crate) previous_revision: Option<String>,
    pub(crate) notes_folder: NotesFolder,
    pub(crate) snapshot_entries: u64,
    pub(crate) snapshot_files: u64,
    pub(crate) snapshot_org_files: u64,
    pub(crate) snapshot_assets: u64,
    pub(crate) snapshot_bytes: u64,
    pub(crate) index_schema: i32,
    pub(crate) stats: IndexStats,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GenerationInspectionError {
    Unavailable,
    RebuildRequired,
}

impl PublishGenerationRequest {
    pub fn new(
        source_record: SourceRecord,
        binding: GenerationBinding,
        expected_active: Option<GenerationId>,
        snapshot: SnapshotOutcome,
        index: StagedIndexOutcome,
    ) -> Result<Self, PublicationError> {
        if source_record.id() != &binding.source
            || !generation_id_is_admitted(&binding.generation)
            || expected_active
                .as_ref()
                .is_some_and(|generation| !generation_id_is_admitted(generation))
        {
            return Err(PublicationError::RequestRefused);
        }
        if binding.source != snapshot.source
            || binding.source != index.source
            || source_record.notes_folder() != &snapshot.notes_folder
            || snapshot.revision != index.revision
            || snapshot.notes_folder != index.notes_folder
            || !revision_is_admitted(&snapshot.revision)
            || index
                .previous_revision
                .as_deref()
                .is_some_and(|revision| !revision_is_admitted(revision))
            || snapshot
                .content_root
                .file_name()
                .and_then(|name| name.to_str())
                != Some(SNAPSHOT_CONTENT_DIRECTORY)
            || index.database.file_name().and_then(|name| name.to_str())
                != Some(STAGED_INDEX_DATABASE_FILE)
        {
            return Err(PublicationError::CandidateMismatch);
        }
        Ok(Self {
            source_record,
            binding,
            expected_active,
            snapshot,
            index,
        })
    }

    #[must_use]
    pub fn binding(&self) -> &GenerationBinding {
        &self.binding
    }

    #[must_use]
    pub fn source_record(&self) -> &SourceRecord {
        &self.source_record
    }
}

impl InFlightGeneration {
    pub fn materializing(revision: &str) -> Result<Self, PublicationError> {
        admitted_revision(revision).map(|revision| Self::Materializing { revision })
    }

    pub fn indexing(revision: &str) -> Result<Self, PublicationError> {
        admitted_revision(revision).map(|revision| Self::Indexing { revision })
    }

    pub fn publishing(revision: &str) -> Result<Self, PublicationError> {
        admitted_revision(revision).map(|revision| Self::Publishing { revision })
    }

    fn is_admitted(&self) -> bool {
        match self {
            Self::Fetching => true,
            Self::Materializing { revision }
            | Self::Indexing { revision }
            | Self::Publishing { revision } => revision_is_admitted(revision),
        }
    }
}

impl GenerationStore {
    /// Create the source binding and generations directory, then open them.
    /// The caller supplies an existing private source root explicitly.
    pub fn initialize(source: SourceId, root: PathBuf) -> Result<Self, PublicationError> {
        validate_real_directory(&root).map_err(|_| PublicationError::StoreUnavailable)?;
        let owner_path = root.join(GENERATION_STORE_MANIFEST_FILE);
        if path_is_present(&owner_path)? {
            validate_store_manifest(&owner_path, &source)?;
        } else {
            let manifest = GenerationStoreManifest {
                version: GENERATION_STORE_FORMAT_VERSION,
                source: source.clone(),
            };
            write_new_json(&owner_path, &manifest)?;
        }
        ensure_private_directory(&root.join(GENERATIONS_DIRECTORY))?;
        ensure_private_directory(&root.join(GENERATION_CANDIDATES_DIRECTORY))?;
        ensure_private_lock_file(&root.join(PUBLICATION_LOCK_FILE))?;
        ensure_private_lock_file(&root.join(RECOVERY_LOCK_FILE))?;
        sync_directory(&root)?;
        Self::open(source, root)
    }

    /// Open an initialized store without creating or repairing any state.
    pub fn open(source: SourceId, root: PathBuf) -> Result<Self, PublicationError> {
        validate_real_directory(&root).map_err(|_| PublicationError::StoreUnavailable)?;
        validate_store_manifest(&root.join(GENERATION_STORE_MANIFEST_FILE), &source)?;
        validate_real_directory(&root.join(GENERATIONS_DIRECTORY))
            .map_err(|_| PublicationError::StoreUnavailable)?;
        validate_real_directory(&root.join(GENERATION_CANDIDATES_DIRECTORY))
            .map_err(|_| PublicationError::StoreUnavailable)?;
        validate_lock_file(&root.join(PUBLICATION_LOCK_FILE))?;
        validate_lock_file(&root.join(RECOVERY_LOCK_FILE))?;
        Ok(Self { source, root })
    }

    #[must_use]
    pub fn source(&self) -> &SourceId {
        &self.source
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The only store-owned location for snapshot and index candidates. A
    /// startup recovery may reclaim every child of this directory.
    #[must_use]
    pub fn candidates_root(&self) -> PathBuf {
        self.root.join(GENERATION_CANDIDATES_DIRECTORY)
    }

    /// Acquire the currently active immutable generation. A missing active
    /// manifest is a defined no-ready-generation state.
    pub fn lease(&self) -> Result<Option<GenerationLease>, PublicationError> {
        let _recovery = RecoveryLock::acquire_shared(&self.root)?;
        self.read_active_record()?.map(lease_record).transpose()
    }

    /// Report transport, ready and in-flight state without conflating them.
    pub fn freshness(
        &self,
        fetched_revision: Option<&str>,
        in_flight: Option<InFlightGeneration>,
    ) -> Result<SourceFreshness, PublicationError> {
        let fetched_revision = fetched_revision.map(admitted_revision).transpose()?;
        if in_flight.as_ref().is_some_and(|state| !state.is_admitted()) {
            return Err(PublicationError::RequestRefused);
        }
        let ready = self.lease()?;
        Ok(SourceFreshness {
            source: self.source.clone(),
            fetched_revision,
            ready_revision: ready.as_ref().map(|lease| lease.revision().to_owned()),
            ready_generation: ready
                .as_ref()
                .map(|lease| lease.binding().generation.clone()),
            in_flight,
        })
    }

    /// Seal both candidates below one immutable generation and atomically
    /// replace the active manifest. Cancellation is honored only before the
    /// activation rename; once that boundary is crossed, the new generation is
    /// the successful ready result.
    pub fn publish(
        &self,
        request: &PublishGenerationRequest,
        cancelled: &AtomicBool,
        mut progress: impl FnMut(PublicationStage),
    ) -> Result<PublicationOutcome, PublicationError> {
        if request.binding.source != self.source {
            return Err(PublicationError::OwnershipMismatch);
        }
        let _publication = PublicationLock::acquire(&self.root)?;
        advance(PublicationStage::Preparing, cancelled, &mut progress)?;

        let current = self.lease()?;
        if let Some(active) = current.as_ref()
            && active.binding() == &request.binding
        {
            let active_manifest: GenerationManifest =
                read_json(&self.root.join(ACTIVE_GENERATION_FILE))?;
            if active_manifest == GenerationManifest::from_request(request) {
                progress(PublicationStage::Complete);
                return Ok(PublicationOutcome {
                    disposition: PublicationDisposition::AlreadyActive,
                    generation: active.clone(),
                });
            }
            return Err(PublicationError::GenerationOccupied);
        }
        let current_id = current
            .as_ref()
            .map(|generation| &generation.binding().generation);
        if current_id != request.expected_active.as_ref() {
            return Err(PublicationError::ActiveGenerationChanged);
        }

        let destination = self.generation_directory(&request.binding.generation);
        let (generation, disposition) = if path_is_present(&destination)? {
            let (manifest, record) = self.inspect_generation(&request.binding.generation)?;
            if manifest != GenerationManifest::from_request(request) {
                return Err(PublicationError::GenerationOccupied);
            }
            let generation = lease_record(record)?;
            advance(PublicationStage::Activating, cancelled, &mut progress)?;
            self.activate_publication(&manifest, cancelled, &mut progress)?;
            (generation, PublicationDisposition::ActivatedExisting)
        } else {
            let (source, index) = self.validate_candidates(request)?;
            ensure_same_filesystem(&self.root, &source)?;
            ensure_same_filesystem(&self.root, &index)?;

            let generations = self.root.join(GENERATIONS_DIRECTORY);
            let staging = tempfile::Builder::new()
                .prefix(GENERATION_STAGE_PREFIX)
                .tempdir_in(&generations)
                .map_err(|_| PublicationError::StorageFailed)?;
            ensure_private_lock_file(&staging.path().join(GENERATION_READER_LOCK_FILE))?;
            let staged_source = staging.path().join(GENERATION_SOURCE_DIRECTORY);
            fs::rename(&source, &staged_source).map_err(|_| PublicationError::StorageFailed)?;
            advance(PublicationStage::SourceSealed, cancelled, &mut progress)?;

            let staged_index = staging.path().join(GENERATION_INDEX_DIRECTORY);
            fs::rename(&index, &staged_index).map_err(|_| PublicationError::StorageFailed)?;
            advance(PublicationStage::IndexSealed, cancelled, &mut progress)?;

            sync_tree_directories(&staged_source)?;
            sync_tree_directories(&staged_index)?;
            File::open(staged_index.join(STAGED_INDEX_DATABASE_FILE))
                .and_then(|file| file.sync_all())
                .map_err(|_| PublicationError::StorageFailed)?;

            let manifest = GenerationManifest::from_request(request);
            write_new_json(&staging.path().join(GENERATION_MANIFEST_FILE), &manifest)?;
            sync_directory(staging.path())?;
            advance(PublicationStage::ManifestSealed, cancelled, &mut progress)?;
            if path_is_present(&destination)? {
                return Err(PublicationError::GenerationOccupied);
            }
            fs::rename(staging.path(), &destination)
                .map_err(|_| PublicationError::StorageFailed)?;
            let _ = staging.keep();
            sync_directory(&generations)?;
            let (sealed, record) = self.inspect_generation(&request.binding.generation)?;
            if sealed != manifest {
                return Err(PublicationError::StoreUnavailable);
            }
            let generation = lease_record(record)?;
            advance(PublicationStage::Activating, cancelled, &mut progress)?;
            self.activate_publication(&manifest, cancelled, &mut progress)?;
            (generation, PublicationDisposition::Published)
        };

        progress(PublicationStage::Complete);
        Ok(PublicationOutcome {
            disposition,
            generation,
        })
    }

    fn validate_candidates(
        &self,
        request: &PublishGenerationRequest,
    ) -> Result<(PathBuf, PathBuf), PublicationError> {
        let source = request
            .snapshot
            .content_root
            .parent()
            .ok_or(PublicationError::CandidateUnavailable)?
            .to_owned();
        let index = request
            .index
            .database
            .parent()
            .ok_or(PublicationError::CandidateUnavailable)?
            .to_owned();
        let candidates = self.candidates_root();
        if !source.starts_with(&candidates)
            || !index.starts_with(&candidates)
            || source == index
            || source.starts_with(&index)
            || index.starts_with(&source)
        {
            return Err(PublicationError::CandidateUnavailable);
        }
        let snapshot =
            inspect_snapshot(&source).map_err(|_| PublicationError::CandidateUnavailable)?;
        let staged = inspect_staged_index(&index).map_err(|error| match error {
            StageIndexError::RebuildRequired => PublicationError::RebuildRequired,
            _ => PublicationError::CandidateUnavailable,
        })?;
        if !snapshot_matches(&snapshot, &request.snapshot)
            || !staged_index_matches(&staged, &request.index)
        {
            return Err(PublicationError::CandidateMismatch);
        }
        Ok((source, index))
    }

    pub(crate) fn read_active_record(&self) -> Result<Option<GenerationRecord>, PublicationError> {
        let active_path = self.root.join(ACTIVE_GENERATION_FILE);
        if !path_is_present(&active_path)? {
            return Ok(None);
        }
        let active: GenerationManifest = read_json(&active_path)?;
        validate_generation_manifest(&active, &self.source)?;
        let (sealed, record) = self.inspect_generation(&active.binding.generation)?;
        if sealed != active {
            return Err(PublicationError::StoreUnavailable);
        }
        Ok(Some(record))
    }

    pub(crate) fn inspect_generation(
        &self,
        generation: &GenerationId,
    ) -> Result<(GenerationManifest, GenerationRecord), PublicationError> {
        let directory = self.generation_directory(generation);
        self.inspect_generation_directory(&directory, Some(generation))
            .map_err(|_| PublicationError::StoreUnavailable)
    }

    pub(crate) fn inspect_generation_directory(
        &self,
        directory: &Path,
        expected_generation: Option<&GenerationId>,
    ) -> Result<(GenerationManifest, GenerationRecord), GenerationInspectionError> {
        validate_real_directory(directory).map_err(|_| GenerationInspectionError::Unavailable)?;
        validate_lock_file(&directory.join(GENERATION_READER_LOCK_FILE))
            .map_err(|_| GenerationInspectionError::Unavailable)?;
        let manifest: GenerationManifest = read_json(&directory.join(GENERATION_MANIFEST_FILE))
            .map_err(|_| GenerationInspectionError::Unavailable)?;
        validate_generation_manifest(&manifest, &self.source)
            .map_err(|_| GenerationInspectionError::Unavailable)?;
        if expected_generation.is_some_and(|generation| &manifest.binding.generation != generation)
        {
            return Err(GenerationInspectionError::Unavailable);
        }
        let source_directory = directory.join(GENERATION_SOURCE_DIRECTORY);
        let index_directory = directory.join(GENERATION_INDEX_DIRECTORY);
        let snapshot = inspect_snapshot(&source_directory)
            .map_err(|_| GenerationInspectionError::Unavailable)?;
        let index = inspect_staged_index(&index_directory).map_err(|error| match error {
            StageIndexError::RebuildRequired => GenerationInspectionError::RebuildRequired,
            _ => GenerationInspectionError::Unavailable,
        })?;
        if snapshot.source != manifest.binding.source
            || snapshot.revision != manifest.revision
            || snapshot.notes_folder != manifest.notes_folder
            || snapshot.entries != manifest.snapshot_entries
            || snapshot.files != manifest.snapshot_files
            || snapshot.org_files != manifest.snapshot_org_files
            || snapshot.assets != manifest.snapshot_assets
            || snapshot.bytes != manifest.snapshot_bytes
            || index.source != manifest.binding.source
            || index.revision != manifest.revision
            || index.previous_revision != manifest.previous_revision
            || index.notes_folder != manifest.notes_folder
            || index.index_schema != manifest.index_schema
            || index.stats != manifest.stats
        {
            return Err(GenerationInspectionError::Unavailable);
        }
        let record = GenerationRecord {
            binding: manifest.binding.clone(),
            source_record: manifest.source_record.clone(),
            revision: manifest.revision.clone(),
            previous_revision: manifest.previous_revision.clone(),
            notes_folder: manifest.notes_folder.clone(),
            content_root: snapshot.content_root,
            index_directory,
            database: index.database,
            reader_lock: directory.join(GENERATION_READER_LOCK_FILE),
            index_schema: manifest.index_schema,
            stats: manifest.stats.clone(),
        };
        Ok((manifest, record))
    }

    fn activate(&self, manifest: &GenerationManifest) -> Result<(), PublicationError> {
        let active_path = self.root.join(ACTIVE_GENERATION_FILE);
        if path_is_present(&active_path)? {
            let current: GenerationManifest = read_json(&active_path)?;
            self.replace_manifest(RETAINED_GENERATION_FILE, &current)?;
        }
        self.replace_manifest(ACTIVE_GENERATION_FILE, manifest)
    }

    fn activate_publication(
        &self,
        manifest: &GenerationManifest,
        cancelled: &AtomicBool,
        progress: &mut impl FnMut(PublicationStage),
    ) -> Result<(), PublicationError> {
        let active_path = self.root.join(ACTIVE_GENERATION_FILE);
        if path_is_present(&active_path)? {
            let current: GenerationManifest = read_json(&active_path)?;
            self.replace_manifest(RETAINED_GENERATION_FILE, &current)?;
            advance(PublicationStage::Retained, cancelled, progress)?;
        }
        self.replace_manifest(ACTIVE_GENERATION_FILE, manifest)
    }

    pub(crate) fn activate_recovered(
        &self,
        manifest: &GenerationManifest,
    ) -> Result<(), PublicationError> {
        self.replace_manifest(ACTIVE_GENERATION_FILE, manifest)
    }

    fn replace_manifest(
        &self,
        name: &str,
        manifest: &GenerationManifest,
    ) -> Result<(), PublicationError> {
        let path = self.root.join(name);
        if let Ok(metadata) = fs::symlink_metadata(&path)
            && (!metadata.is_file() || metadata.file_type().is_symlink())
        {
            return Err(PublicationError::StoreUnavailable);
        }
        let bytes = encoded_json(manifest)?;
        let root_directory = File::open(&self.root).map_err(|_| PublicationError::StorageFailed)?;
        let mut temporary =
            NamedTempFile::new_in(&self.root).map_err(|_| PublicationError::StorageFailed)?;
        set_private_file_permissions(temporary.as_file())?;
        temporary
            .write_all(&bytes)
            .and_then(|()| temporary.as_file().sync_all())
            .map_err(|_| PublicationError::StorageFailed)?;
        temporary
            .persist(&path)
            .map_err(|_| PublicationError::StorageFailed)?;
        // The atomic rename commits this pointer. A directory-sync error cannot
        // roll it back, so reporting failure after that point would lie to a
        // retrying caller. Active-pointer callers perform no later fallible
        // operation; recovery can observe only the previous or sealed value.
        let _ = root_directory.sync_all();
        Ok(())
    }

    pub(crate) fn generation_directory(&self, generation: &GenerationId) -> PathBuf {
        self.root
            .join(GENERATIONS_DIRECTORY)
            .join(generation.as_str())
    }
}

impl GenerationLease {
    #[must_use]
    pub fn binding(&self) -> &GenerationBinding {
        &self.record.binding
    }

    #[must_use]
    pub fn source_record(&self) -> &SourceRecord {
        &self.record.source_record
    }

    #[must_use]
    pub fn revision(&self) -> &str {
        &self.record.revision
    }

    #[must_use]
    pub fn previous_revision(&self) -> Option<&str> {
        self.record.previous_revision.as_deref()
    }

    #[must_use]
    pub fn notes_folder(&self) -> &NotesFolder {
        &self.record.notes_folder
    }

    #[must_use]
    pub fn content_root(&self) -> &Path {
        &self.record.content_root
    }

    #[must_use]
    pub fn index_directory(&self) -> &Path {
        &self.record.index_directory
    }

    #[must_use]
    pub fn database(&self) -> &Path {
        &self.record.database
    }

    #[must_use]
    pub fn index_schema(&self) -> i32 {
        self.record.index_schema
    }

    #[must_use]
    pub fn stats(&self) -> &IndexStats {
        &self.record.stats
    }

    /// Open one ordinary file from this generation's immutable source tree.
    /// The returned descriptor remains bound even after a newer generation is
    /// activated.
    pub fn open_content(&self, relative: &str) -> Result<GenerationContent, PublicationError> {
        validate_content_path(relative)?;
        let path = self.record.content_root.join(relative);
        let metadata =
            fs::symlink_metadata(&path).map_err(|_| PublicationError::StoreUnavailable)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(PublicationError::StoreUnavailable);
        }
        if fs::canonicalize(&path).map_err(|_| PublicationError::StoreUnavailable)? != path
            || !path.starts_with(&self.record.content_root)
        {
            return Err(PublicationError::StoreUnavailable);
        }
        let file = File::open(path).map_err(|_| PublicationError::StoreUnavailable)?;
        Ok(GenerationContent {
            file,
            _reader_lock: Arc::clone(&self._reader_lock),
        })
    }
}

impl Read for GenerationContent {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        self.file.read(buffer)
    }
}

impl Seek for GenerationContent {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.file.seek(position)
    }
}

impl GenerationManifest {
    pub(crate) fn from_request(request: &PublishGenerationRequest) -> Self {
        Self {
            version: GENERATION_FORMAT_VERSION,
            source_record: request.source_record.clone(),
            binding: request.binding.clone(),
            expected_active: request.expected_active.clone(),
            revision: request.snapshot.revision.clone(),
            previous_revision: request.index.previous_revision.clone(),
            notes_folder: request.snapshot.notes_folder.clone(),
            snapshot_entries: request.snapshot.entries,
            snapshot_files: request.snapshot.files,
            snapshot_org_files: request.snapshot.org_files,
            snapshot_assets: request.snapshot.assets,
            snapshot_bytes: request.snapshot.bytes,
            index_schema: request.index.index_schema,
            stats: request.index.stats.clone(),
        }
    }
}

fn advance(
    stage: PublicationStage,
    cancelled: &AtomicBool,
    progress: &mut impl FnMut(PublicationStage),
) -> Result<(), PublicationError> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(PublicationError::Cancelled);
    }
    progress(stage);
    if cancelled.load(Ordering::Relaxed) {
        Err(PublicationError::Cancelled)
    } else {
        Ok(())
    }
}

fn snapshot_matches(actual: &SnapshotOutcome, expected: &SnapshotOutcome) -> bool {
    actual.source == expected.source
        && actual.revision == expected.revision
        && actual.notes_folder == expected.notes_folder
        && actual.content_root == expected.content_root
        && actual.entries == expected.entries
        && actual.files == expected.files
        && actual.org_files == expected.org_files
        && actual.assets == expected.assets
        && actual.bytes == expected.bytes
        && actual.diagnostics == expected.diagnostics
}

fn staged_index_matches(actual: &StagedIndexOutcome, expected: &StagedIndexOutcome) -> bool {
    actual.source == expected.source
        && actual.revision == expected.revision
        && actual.previous_revision == expected.previous_revision
        && actual.notes_folder == expected.notes_folder
        && actual.index_schema == expected.index_schema
        && actual.database == expected.database
        && actual.stats == expected.stats
        && actual.parsed_org_files == expected.parsed_org_files
        && actual.removed_file_paths == expected.removed_file_paths
}

pub(crate) fn validate_generation_manifest(
    manifest: &GenerationManifest,
    source: &SourceId,
) -> Result<(), PublicationError> {
    if manifest.version != GENERATION_FORMAT_VERSION
        || manifest.source_record.id() != source
        || manifest.source_record.notes_folder() != &manifest.notes_folder
        || &manifest.binding.source != source
        || !generation_id_is_admitted(&manifest.binding.generation)
        || manifest
            .expected_active
            .as_ref()
            .is_some_and(|generation| !generation_id_is_admitted(generation))
        || !revision_is_admitted(&manifest.revision)
        || manifest
            .previous_revision
            .as_deref()
            .is_some_and(|revision| !revision_is_admitted(revision))
        || manifest
            .snapshot_org_files
            .checked_add(manifest.snapshot_assets)
            != Some(manifest.snapshot_files)
        || manifest.snapshot_files > manifest.snapshot_entries
    {
        return Err(PublicationError::StoreUnavailable);
    }
    Ok(())
}

fn generation_id_is_admitted(generation: &GenerationId) -> bool {
    !matches!(generation.as_str(), "." | "..")
        && Path::new(generation.as_str())
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn lease_record(record: GenerationRecord) -> Result<GenerationLease, PublicationError> {
    validate_lock_file(&record.reader_lock)?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&record.reader_lock)
        .map_err(|_| PublicationError::StoreUnavailable)?;
    match fs2::FileExt::try_lock_shared(&file) {
        Ok(()) => Ok(GenerationLease {
            record: Arc::new(record),
            _reader_lock: Arc::new(file),
        }),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
            Err(PublicationError::RecoveryInProgress)
        }
        Err(_) => Err(PublicationError::StoreUnavailable),
    }
}

fn validate_store_manifest(path: &Path, source: &SourceId) -> Result<(), PublicationError> {
    let manifest: GenerationStoreManifest = read_json(path)?;
    if manifest.version != GENERATION_STORE_FORMAT_VERSION {
        return Err(PublicationError::StoreUnavailable);
    }
    if &manifest.source != source {
        return Err(PublicationError::OwnershipMismatch);
    }
    Ok(())
}

fn admitted_revision(revision: &str) -> Result<String, PublicationError> {
    if revision_is_admitted(revision) {
        Ok(revision.to_owned())
    } else {
        Err(PublicationError::RequestRefused)
    }
}

fn revision_is_admitted(revision: &str) -> bool {
    revision.len() == 40
        && revision
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn validate_content_path(relative: &str) -> Result<(), PublicationError> {
    let path = Path::new(relative);
    if relative.is_empty()
        || relative.len() > MAX_SNAPSHOT_PATH_BYTES
        || relative.contains('\\')
        || relative.contains(':')
        || relative.chars().any(char::is_control)
        || path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(PublicationError::RequestRefused);
    }
    Ok(())
}

fn validate_real_directory(path: &Path) -> Result<(), PublicationError> {
    if !storage_path_is_admitted(path) {
        return Err(PublicationError::RequestRefused);
    }
    let metadata = fs::symlink_metadata(path).map_err(|_| PublicationError::StoreUnavailable)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || fs::canonicalize(path).map_err(|_| PublicationError::StoreUnavailable)? != path
    {
        return Err(PublicationError::StoreUnavailable);
    }
    Ok(())
}

fn storage_path_is_admitted(path: &Path) -> bool {
    path.is_absolute()
        && path.as_os_str().as_encoded_bytes().len() <= MAX_REPOSITORY_PATH_BYTES
        && path.parent().is_some()
        && path
            .components()
            .all(|component| matches!(component, Component::RootDir | Component::Normal(_)))
}

fn path_is_present(path: &Path) -> Result<bool, PublicationError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(PublicationError::StorageFailed),
    }
}

fn ensure_private_directory(path: &Path) -> Result<(), PublicationError> {
    match fs::create_dir(path) {
        Ok(()) => set_private_directory_permissions(path),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            validate_real_directory(path).map_err(|_| PublicationError::StoreUnavailable)
        }
        Err(_) => Err(PublicationError::StorageFailed),
    }
}

fn ensure_private_lock_file(path: &Path) -> Result<(), PublicationError> {
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(file) => {
            set_private_file_permissions(&file)?;
            file.sync_all().map_err(|_| PublicationError::StorageFailed)
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => validate_lock_file(path),
        Err(_) => Err(PublicationError::StorageFailed),
    }
}

fn validate_lock_file(path: &Path) -> Result<(), PublicationError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| PublicationError::StoreUnavailable)?;
    if metadata.is_file() && !metadata.file_type().is_symlink() {
        Ok(())
    } else {
        Err(PublicationError::StoreUnavailable)
    }
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T, PublicationError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| PublicationError::StoreUnavailable)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > MAX_GENERATION_MANIFEST_BYTES
    {
        return Err(PublicationError::StoreUnavailable);
    }
    let bytes = fs::read(path).map_err(|_| PublicationError::StoreUnavailable)?;
    serde_json::from_slice(&bytes).map_err(|_| PublicationError::StoreUnavailable)
}

fn encoded_json(value: &impl Serialize) -> Result<Vec<u8>, PublicationError> {
    let bytes = serde_json::to_vec(value).map_err(|_| PublicationError::StorageFailed)?;
    if bytes.len() as u64 > MAX_GENERATION_MANIFEST_BYTES {
        return Err(PublicationError::StorageFailed);
    }
    Ok(bytes)
}

fn write_new_json(path: &Path, value: &impl Serialize) -> Result<(), PublicationError> {
    let bytes = encoded_json(value)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| PublicationError::StorageFailed)?;
    set_private_file_permissions(&file)?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| PublicationError::StorageFailed)
}

fn sync_tree_directories(path: &Path) -> Result<(), PublicationError> {
    for entry in fs::read_dir(path).map_err(|_| PublicationError::StorageFailed)? {
        let entry = entry.map_err(|_| PublicationError::StorageFailed)?;
        let file_type = entry
            .file_type()
            .map_err(|_| PublicationError::StorageFailed)?;
        if file_type.is_symlink() {
            return Err(PublicationError::StorageFailed);
        }
        if file_type.is_dir() {
            sync_tree_directories(&entry.path())?;
        }
    }
    sync_directory(path)
}

#[cfg(unix)]
fn ensure_same_filesystem(left: &Path, right: &Path) -> Result<(), PublicationError> {
    use std::os::unix::fs::MetadataExt;

    let left = fs::metadata(left).map_err(|_| PublicationError::StorageFailed)?;
    let right = fs::metadata(right).map_err(|_| PublicationError::CandidateUnavailable)?;
    if left.dev() == right.dev() {
        Ok(())
    } else {
        Err(PublicationError::CandidateUnavailable)
    }
}

#[cfg(not(unix))]
fn ensure_same_filesystem(_left: &Path, _right: &Path) -> Result<(), PublicationError> {
    Ok(())
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), PublicationError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| PublicationError::StorageFailed)
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<(), PublicationError> {
    Ok(())
}

#[cfg(unix)]
fn set_private_directory_permissions(path: &Path) -> Result<(), PublicationError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|_| PublicationError::StorageFailed)
}

#[cfg(not(unix))]
fn set_private_directory_permissions(_path: &Path) -> Result<(), PublicationError> {
    Ok(())
}

#[cfg(unix)]
fn set_private_file_permissions(file: &File) -> Result<(), PublicationError> {
    use std::os::unix::fs::PermissionsExt;

    file.set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(|_| PublicationError::StorageFailed)
}

#[cfg(not(unix))]
fn set_private_file_permissions(_file: &File) -> Result<(), PublicationError> {
    Ok(())
}

struct PublicationLock {
    file: File,
}

impl PublicationLock {
    fn acquire(root: &Path) -> Result<Self, PublicationError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(root.join(PUBLICATION_LOCK_FILE))
            .map_err(|_| PublicationError::StoreUnavailable)?;
        match fs2::FileExt::try_lock_exclusive(&file) {
            Ok(()) => Ok(Self { file }),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                Err(PublicationError::PublicationInProgress)
            }
            Err(_) => Err(PublicationError::StorageFailed),
        }
    }
}

impl Drop for PublicationLock {
    fn drop(&mut self) {
        let _ = fs2::FileExt::unlock(&self.file);
    }
}

struct RecoveryLock {
    file: File,
}

impl RecoveryLock {
    fn open(root: &Path) -> Result<File, PublicationError> {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(root.join(RECOVERY_LOCK_FILE))
            .map_err(|_| PublicationError::StoreUnavailable)
    }

    fn acquire_shared(root: &Path) -> Result<Self, PublicationError> {
        let file = Self::open(root)?;
        match fs2::FileExt::try_lock_shared(&file) {
            Ok(()) => Ok(Self { file }),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                Err(PublicationError::RecoveryInProgress)
            }
            Err(_) => Err(PublicationError::StorageFailed),
        }
    }

    fn acquire_exclusive(root: &Path) -> Result<Self, PublicationError> {
        let file = Self::open(root)?;
        match fs2::FileExt::try_lock_exclusive(&file) {
            Ok(()) => Ok(Self { file }),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                Err(PublicationError::RecoveryInProgress)
            }
            Err(_) => Err(PublicationError::StorageFailed),
        }
    }
}

impl Drop for RecoveryLock {
    fn drop(&mut self) {
        let _ = fs2::FileExt::unlock(&self.file);
    }
}
