//! Candidate index construction over immutable repository snapshots.

mod publication;
mod refresh;

pub use publication::*;
pub use refresh::*;

use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};
use slipbox_core::{IndexStats, NotesFolder, SourceId};
use slipbox_git::{
    DeltaDisposition, DeltaFileKind, DeltaOutcome, MAX_REPOSITORY_PATH_BYTES,
    MAX_SNAPSHOT_PATH_BYTES, SnapshotOutcome,
};
use slipbox_index::{DiscoveryPolicy, PlatformPolicy, scan_path_with_platform};
use slipbox_store::{Database, INDEX_SCHEMA_VERSION};
use thiserror::Error;

pub const STAGED_INDEX_FORMAT_VERSION: u32 = 1;
pub const STAGED_INDEX_DATABASE_FILE: &str = "index.sqlite3";
pub const STAGED_INDEX_MANIFEST_FILE: &str = "index.json";

const MAX_STAGED_INDEX_MANIFEST_BYTES: u64 = 64 * 1024;

/// Exact materialized input, delta and optional previous index for one candidate.
#[derive(Debug, Clone)]
pub struct StageIndexRequest {
    snapshot: SnapshotOutcome,
    delta: DeltaOutcome,
    reading_renames: Vec<(String, String)>,
    base: Option<PathBuf>,
    destination: PathBuf,
}

impl StageIndexRequest {
    pub fn new(
        snapshot: SnapshotOutcome,
        delta: DeltaOutcome,
        base: Option<PathBuf>,
        destination: PathBuf,
    ) -> Result<Self, StageIndexError> {
        if snapshot.source != delta.source
            || snapshot.revision != delta.revision
            || snapshot.notes_folder != delta.notes_folder
            || delta.previous_revision.is_some() != base.is_some()
            || !revision_is_admitted(&snapshot.revision)
            || delta
                .previous_revision
                .as_deref()
                .is_some_and(|revision| !revision_is_admitted(revision))
            || !storage_path_is_admitted(&snapshot.content_root)
            || !storage_path_is_admitted(&destination)
            || base
                .as_deref()
                .is_some_and(|path| !storage_path_is_admitted(path))
            || destination.starts_with(&snapshot.content_root)
            || snapshot.content_root.starts_with(&destination)
            || base.as_ref().is_some_and(|path| {
                path == &destination
                    || path.starts_with(&destination)
                    || destination.starts_with(path)
            })
        {
            return Err(StageIndexError::RequestRefused);
        }
        let reading_renames = reading_renames(&delta);
        Ok(Self {
            snapshot,
            delta,
            reading_renames,
            base,
            destination,
        })
    }

    /// Preserve proven moves from the incremental comparison when a schema
    /// rebuild replaces the indexing delta with an initial inventory.
    #[must_use]
    pub(crate) fn with_reading_renames(mut self, delta: &DeltaOutcome) -> Self {
        self.reading_renames = reading_renames(delta);
        self
    }

    #[must_use]
    pub fn source(&self) -> &SourceId {
        &self.snapshot.source
    }

    #[must_use]
    pub fn revision(&self) -> &str {
        &self.snapshot.revision
    }

    #[must_use]
    pub fn destination(&self) -> &Path {
        &self.destination
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StagedIndexDisposition {
    Created,
    Existing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedIndexOutcome {
    pub disposition: StagedIndexDisposition,
    pub source: SourceId,
    pub revision: String,
    pub previous_revision: Option<String>,
    pub notes_folder: NotesFolder,
    pub index_schema: i32,
    pub database: PathBuf,
    pub stats: IndexStats,
    pub parsed_org_files: u64,
    pub removed_file_paths: u64,
    pub reading_file_renames: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageIndexProgressStage {
    Preparing,
    Parsing,
    Updating,
    Verifying,
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageIndexProgress {
    pub stage: StageIndexProgressStage,
    pub completed_files: u64,
    pub total_files: u64,
}

/// Closed staging failures. Paths and note contents never become diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum StageIndexError {
    #[error("the staged index request is not admitted")]
    RequestRefused,
    #[error("the staged index destination is already occupied")]
    DestinationOccupied,
    #[error("the materialized snapshot is unavailable")]
    SnapshotUnavailable,
    #[error("the incremental index base is unavailable")]
    BaseUnavailable,
    #[error("the incremental index base does not match the requested delta")]
    BaseMismatch,
    #[error("the incremental index requires an explicit rebuild")]
    RebuildRequired,
    #[error("the index input is invalid")]
    InputInvalid,
    #[error("the index operation was cancelled")]
    Cancelled,
    #[error("the staged index could not be built")]
    IndexFailed,
    #[error("the staged index could not be stored")]
    StorageFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StagedIndexManifest {
    version: u32,
    source: SourceId,
    revision: String,
    previous_revision: Option<String>,
    notes_folder: NotesFolder,
    index_schema: i32,
    stats: IndexStats,
    parsed_org_files: u64,
    removed_file_paths: u64,
    #[serde(default)]
    reading_file_renames: u64,
}

#[derive(Debug)]
struct IndexPlan {
    parse: Vec<String>,
    remove: Vec<String>,
}

/// Build an immutable candidate database and expose it only after verification
/// and completion metadata have both reached its destination.
pub fn stage_index(
    request: &StageIndexRequest,
    cancelled: &AtomicBool,
    mut progress: impl FnMut(StageIndexProgress),
) -> Result<StagedIndexOutcome, StageIndexError> {
    check_cancelled(cancelled)?;
    let plan = IndexPlan::from_delta(&request.delta)?;
    let total_files = (plan.parse.len() + plan.remove.len()) as u64;
    progress(StageIndexProgress {
        stage: StageIndexProgressStage::Preparing,
        completed_files: 0,
        total_files,
    });

    if path_is_present(&request.destination)? {
        let existing = inspect_staged_index(&request.destination)?;
        if outcome_matches_request(&existing, request, &plan) {
            check_cancelled(cancelled)?;
            progress(StageIndexProgress {
                stage: StageIndexProgressStage::Complete,
                completed_files: total_files,
                total_files,
            });
            return Ok(StagedIndexOutcome {
                disposition: StagedIndexDisposition::Existing,
                ..existing
            });
        }
        return Err(StageIndexError::DestinationOccupied);
    }

    validate_content_root(&request.snapshot.content_root)?;
    let parent = request
        .destination
        .parent()
        .ok_or(StageIndexError::RequestRefused)?;
    validate_parent(parent)?;
    let staging = tempfile::Builder::new()
        .prefix(".slipbox-index-stage-")
        .tempdir_in(parent)
        .map_err(|_| StageIndexError::StorageFailed)?;
    let database_path = staging.path().join(STAGED_INDEX_DATABASE_FILE);

    if let Some(base) = request.base.as_deref() {
        validate_base(base, request)?;
        Database::clone_current_index(&base.join(STAGED_INDEX_DATABASE_FILE), &database_path)
            .map_err(|_| StageIndexError::IndexFailed)?;
    }
    check_cancelled(cancelled)?;

    let mut database = Database::open(&database_path).map_err(|_| StageIndexError::IndexFailed)?;
    let mut completed_files = 0_u64;
    if !plan.remove.is_empty() {
        progress(StageIndexProgress {
            stage: StageIndexProgressStage::Updating,
            completed_files,
            total_files,
        });
        database
            .remove_file_indexes(&plan.remove)
            .map_err(|_| StageIndexError::IndexFailed)?;
        completed_files += plan.remove.len() as u64;
    }

    let policy = DiscoveryPolicy::default();
    let platform = PlatformPolicy::headless();
    let mut parsed = Vec::with_capacity(plan.parse.len());
    for path in &plan.parse {
        check_cancelled(cancelled)?;
        progress(StageIndexProgress {
            stage: StageIndexProgressStage::Parsing,
            completed_files,
            total_files,
        });
        let absolute = request.snapshot.content_root.join(path);
        validate_snapshot_file(&request.snapshot.content_root, &absolute)?;
        let mut file = scan_path_with_platform(
            &request.snapshot.content_root,
            &absolute,
            &policy,
            &platform,
        )
        .map_err(|_| StageIndexError::InputInvalid)?;
        // Git commits carry no filesystem mtime. A stable zero keeps clean and
        // incremental repository indexes semantically identical.
        file.mtime_ns = 0;
        parsed.push(file);
        completed_files += 1;
    }

    check_cancelled(cancelled)?;
    progress(StageIndexProgress {
        stage: StageIndexProgressStage::Updating,
        completed_files,
        total_files,
    });
    database
        .sync_file_indexes(&parsed)
        .map_err(|_| StageIndexError::IndexFailed)?;
    database
        .record_reading_file_renames(&request.reading_renames)
        .map_err(|_| StageIndexError::IndexFailed)?;
    check_cancelled(cancelled)?;
    progress(StageIndexProgress {
        stage: StageIndexProgressStage::Verifying,
        completed_files,
        total_files,
    });
    database
        .finish_index_writes()
        .and_then(|()| database.verify_current_index())
        .map_err(|_| StageIndexError::IndexFailed)?;
    let stats = database.stats().map_err(|_| StageIndexError::IndexFailed)?;
    drop(database);
    make_private_file(&database_path)?;

    let manifest = StagedIndexManifest {
        version: STAGED_INDEX_FORMAT_VERSION,
        source: request.snapshot.source.clone(),
        revision: request.snapshot.revision.clone(),
        previous_revision: request.delta.previous_revision.clone(),
        notes_folder: request.snapshot.notes_folder.clone(),
        index_schema: INDEX_SCHEMA_VERSION,
        stats,
        parsed_org_files: plan.parse.len() as u64,
        removed_file_paths: plan.remove.len() as u64,
        reading_file_renames: request.reading_renames.len() as u64,
    };
    write_manifest(staging.path(), &manifest)?;
    check_cancelled(cancelled)?;
    sync_directory(staging.path())?;
    if path_is_present(&request.destination)? {
        return Err(StageIndexError::DestinationOccupied);
    }
    fs::rename(staging.path(), &request.destination).map_err(|_| StageIndexError::StorageFailed)?;
    let _ = staging.keep();
    sync_directory(parent)?;
    progress(StageIndexProgress {
        stage: StageIndexProgressStage::Complete,
        completed_files: total_files,
        total_files,
    });
    Ok(outcome_from_manifest(
        StagedIndexDisposition::Created,
        &request.destination,
        manifest,
    ))
}

/// Inspect a completed candidate without enabling schema migrations.
pub fn inspect_staged_index(directory: &Path) -> Result<StagedIndexOutcome, StageIndexError> {
    validate_stage_directory(directory).map_err(|_| StageIndexError::DestinationOccupied)?;
    let manifest = read_manifest(directory).map_err(|error| match error {
        StageIndexError::RebuildRequired => StageIndexError::RebuildRequired,
        _ => StageIndexError::DestinationOccupied,
    })?;
    let database = directory.join(STAGED_INDEX_DATABASE_FILE);
    validate_index_file(&database).map_err(|_| StageIndexError::DestinationOccupied)?;
    if Database::index_schema_version(&database)
        .map_err(|_| StageIndexError::DestinationOccupied)?
        != INDEX_SCHEMA_VERSION
    {
        return Err(StageIndexError::RebuildRequired);
    }
    let stats = Database::current_index_stats(&database)
        .map_err(|_| StageIndexError::DestinationOccupied)?;
    if stats != manifest.stats {
        return Err(StageIndexError::DestinationOccupied);
    }
    Ok(outcome_from_manifest(
        StagedIndexDisposition::Existing,
        directory,
        manifest,
    ))
}

impl IndexPlan {
    fn from_delta(delta: &DeltaOutcome) -> Result<Self, StageIndexError> {
        let mut parse = BTreeSet::new();
        let mut remove = BTreeSet::new();
        for file in &delta.added {
            validate_relative_path(&file.path)?;
            if file.kind == DeltaFileKind::Org {
                ensure_under_notes(&file.path, &delta.notes_folder)?;
                parse.insert(file.path.clone());
            }
        }
        for file in &delta.modified {
            validate_relative_path(&file.path)?;
            if file.kind == DeltaFileKind::Org {
                ensure_under_notes(&file.path, &delta.notes_folder)?;
                parse.insert(file.path.clone());
            }
        }
        for file in &delta.deleted {
            validate_relative_path(&file.path)?;
            if file.kind == DeltaFileKind::Org {
                if let Some(notes) = delta.previous_notes_folder.as_ref() {
                    ensure_under_notes(&file.path, notes)?;
                }
                remove.insert(file.path.clone());
            }
        }
        for rename in &delta.renamed {
            validate_relative_path(&rename.from)?;
            validate_relative_path(&rename.to)?;
            if rename.kind == DeltaFileKind::Org {
                if let Some(notes) = delta.previous_notes_folder.as_ref() {
                    ensure_under_notes(&rename.from, notes)?;
                }
                ensure_under_notes(&rename.to, &delta.notes_folder)?;
                remove.insert(rename.from.clone());
                parse.insert(rename.to.clone());
            }
        }
        if delta.disposition == DeltaDisposition::Unchanged
            && (!parse.is_empty() || !remove.is_empty())
        {
            return Err(StageIndexError::InputInvalid);
        }
        Ok(Self {
            parse: parse.into_iter().collect(),
            remove: remove.into_iter().collect(),
        })
    }
}

fn validate_base(base: &Path, request: &StageIndexRequest) -> Result<(), StageIndexError> {
    validate_stage_directory(base).map_err(|_| StageIndexError::BaseUnavailable)?;
    let manifest = read_manifest(base).map_err(|error| match error {
        StageIndexError::RebuildRequired => StageIndexError::RebuildRequired,
        _ => StageIndexError::BaseUnavailable,
    })?;
    if manifest.source != request.delta.source
        || Some(manifest.revision.as_str()) != request.delta.previous_revision.as_deref()
        || Some(&manifest.notes_folder) != request.delta.previous_notes_folder.as_ref()
    {
        return Err(StageIndexError::BaseMismatch);
    }
    let database = base.join(STAGED_INDEX_DATABASE_FILE);
    validate_index_file(&database).map_err(|_| StageIndexError::BaseUnavailable)?;
    if Database::index_schema_version(&database).map_err(|_| StageIndexError::BaseUnavailable)?
        != INDEX_SCHEMA_VERSION
    {
        return Err(StageIndexError::RebuildRequired);
    }
    let stats =
        Database::current_index_stats(&database).map_err(|_| StageIndexError::BaseUnavailable)?;
    if stats != manifest.stats {
        return Err(StageIndexError::BaseUnavailable);
    }
    Ok(())
}

fn outcome_matches_request(
    outcome: &StagedIndexOutcome,
    request: &StageIndexRequest,
    plan: &IndexPlan,
) -> bool {
    outcome.source == request.snapshot.source
        && outcome.revision == request.snapshot.revision
        && outcome.previous_revision == request.delta.previous_revision
        && outcome.notes_folder == request.snapshot.notes_folder
        && outcome.index_schema == INDEX_SCHEMA_VERSION
        && outcome.parsed_org_files == plan.parse.len() as u64
        && outcome.removed_file_paths == plan.remove.len() as u64
        && outcome.reading_file_renames == request.reading_renames.len() as u64
}

fn outcome_from_manifest(
    disposition: StagedIndexDisposition,
    directory: &Path,
    manifest: StagedIndexManifest,
) -> StagedIndexOutcome {
    StagedIndexOutcome {
        disposition,
        source: manifest.source,
        revision: manifest.revision,
        previous_revision: manifest.previous_revision,
        notes_folder: manifest.notes_folder,
        index_schema: manifest.index_schema,
        database: directory.join(STAGED_INDEX_DATABASE_FILE),
        stats: manifest.stats,
        parsed_org_files: manifest.parsed_org_files,
        removed_file_paths: manifest.removed_file_paths,
        reading_file_renames: manifest.reading_file_renames,
    }
}

fn reading_renames(delta: &DeltaOutcome) -> Vec<(String, String)> {
    delta
        .renamed
        .iter()
        .filter(|rename| rename.kind == DeltaFileKind::Org)
        .map(|rename| (rename.from.clone(), rename.to.clone()))
        .collect()
}

fn read_manifest(directory: &Path) -> Result<StagedIndexManifest, StageIndexError> {
    let path = directory.join(STAGED_INDEX_MANIFEST_FILE);
    let metadata = fs::symlink_metadata(&path).map_err(|_| StageIndexError::StorageFailed)?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > MAX_STAGED_INDEX_MANIFEST_BYTES
    {
        return Err(StageIndexError::StorageFailed);
    }
    let bytes = fs::read(path).map_err(|_| StageIndexError::StorageFailed)?;
    let manifest: StagedIndexManifest =
        serde_json::from_slice(&bytes).map_err(|_| StageIndexError::StorageFailed)?;
    if manifest.version != STAGED_INDEX_FORMAT_VERSION {
        return Err(StageIndexError::StorageFailed);
    }
    if !revision_is_admitted(&manifest.revision)
        || manifest
            .previous_revision
            .as_deref()
            .is_some_and(|revision| !revision_is_admitted(revision))
    {
        return Err(StageIndexError::StorageFailed);
    }
    if manifest.index_schema != INDEX_SCHEMA_VERSION {
        return Err(StageIndexError::RebuildRequired);
    }
    Ok(manifest)
}

fn write_manifest(directory: &Path, manifest: &StagedIndexManifest) -> Result<(), StageIndexError> {
    let bytes = serde_json::to_vec(manifest).map_err(|_| StageIndexError::StorageFailed)?;
    if bytes.len() as u64 > MAX_STAGED_INDEX_MANIFEST_BYTES {
        return Err(StageIndexError::StorageFailed);
    }
    let path = directory.join(STAGED_INDEX_MANIFEST_FILE);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|_| StageIndexError::StorageFailed)?;
    make_private_file(&path)?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| StageIndexError::StorageFailed)
}

fn validate_content_root(root: &Path) -> Result<(), StageIndexError> {
    let metadata = fs::symlink_metadata(root).map_err(|_| StageIndexError::SnapshotUnavailable)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(StageIndexError::SnapshotUnavailable);
    }
    if fs::canonicalize(root).map_err(|_| StageIndexError::SnapshotUnavailable)? != root {
        return Err(StageIndexError::SnapshotUnavailable);
    }
    Ok(())
}

fn validate_snapshot_file(root: &Path, path: &Path) -> Result<(), StageIndexError> {
    if !path.starts_with(root) {
        return Err(StageIndexError::InputInvalid);
    }
    let metadata = fs::symlink_metadata(path).map_err(|_| StageIndexError::InputInvalid)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(StageIndexError::InputInvalid);
    }
    if fs::canonicalize(path).map_err(|_| StageIndexError::InputInvalid)? != path {
        return Err(StageIndexError::InputInvalid);
    }
    Ok(())
}

fn validate_parent(parent: &Path) -> Result<(), StageIndexError> {
    let metadata = fs::symlink_metadata(parent).map_err(|_| StageIndexError::StorageFailed)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(StageIndexError::StorageFailed);
    }
    if fs::canonicalize(parent).map_err(|_| StageIndexError::StorageFailed)? != parent {
        return Err(StageIndexError::StorageFailed);
    }
    Ok(())
}

fn validate_stage_directory(directory: &Path) -> Result<(), StageIndexError> {
    if !storage_path_is_admitted(directory) {
        return Err(StageIndexError::StorageFailed);
    }
    let metadata = fs::symlink_metadata(directory).map_err(|_| StageIndexError::StorageFailed)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(StageIndexError::StorageFailed);
    }
    if fs::canonicalize(directory).map_err(|_| StageIndexError::StorageFailed)? != directory {
        return Err(StageIndexError::StorageFailed);
    }
    Ok(())
}

fn validate_index_file(path: &Path) -> Result<(), StageIndexError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| StageIndexError::StorageFailed)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(StageIndexError::StorageFailed);
    }
    Ok(())
}

fn validate_relative_path(path: &str) -> Result<(), StageIndexError> {
    let candidate = Path::new(path);
    if path.is_empty()
        || path.len() > MAX_SNAPSHOT_PATH_BYTES
        || path.contains('\\')
        || path.contains(':')
        || path.chars().any(char::is_control)
        || candidate.is_absolute()
        || candidate.components().any(|component| {
            let Component::Normal(segment) = component else {
                return true;
            };
            let Some(segment) = segment.to_str() else {
                return true;
            };
            segment.is_empty() || segment.len() > 255 || segment.eq_ignore_ascii_case(".git")
        })
    {
        return Err(StageIndexError::InputInvalid);
    }
    Ok(())
}

fn ensure_under_notes(path: &str, notes: &NotesFolder) -> Result<(), StageIndexError> {
    if notes.is_root()
        || path
            .strip_prefix(notes.as_str())
            .is_some_and(|suffix| suffix.starts_with('/'))
    {
        Ok(())
    } else {
        Err(StageIndexError::InputInvalid)
    }
}

fn check_cancelled(cancelled: &AtomicBool) -> Result<(), StageIndexError> {
    if cancelled.load(Ordering::Relaxed) {
        Err(StageIndexError::Cancelled)
    } else {
        Ok(())
    }
}

fn revision_is_admitted(revision: &str) -> bool {
    revision.len() == 40
        && revision
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn path_is_present(path: &Path) -> Result<bool, StageIndexError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(StageIndexError::StorageFailed),
    }
}

fn storage_path_is_admitted(path: &Path) -> bool {
    path.is_absolute()
        && path.as_os_str().as_encoded_bytes().len() <= MAX_REPOSITORY_PATH_BYTES
        && path.parent().is_some()
        && path
            .components()
            .all(|component| matches!(component, Component::RootDir | Component::Normal(_)))
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), StageIndexError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| StageIndexError::StorageFailed)
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<(), StageIndexError> {
    Ok(())
}

#[cfg(unix)]
fn make_private_file(path: &Path) -> Result<(), StageIndexError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|_| StageIndexError::StorageFailed)
}

#[cfg(not(unix))]
fn make_private_file(_path: &Path) -> Result<(), StageIndexError> {
    Ok(())
}
