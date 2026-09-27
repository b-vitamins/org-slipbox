//! Immutable, source-bound filesystem snapshots materialized from Git objects.
//!
//! This module reads commits, trees and blobs directly. It never asks Git for a
//! checkout, so repository configuration, hooks, attributes and executable
//! bits cannot run code or alter the bytes that enter a snapshot.

use std::collections::{BTreeSet, VecDeque};
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::AtomicBool;

use gix::object::tree::EntryKind;
use serde::{Deserialize, Serialize};
use slipbox_core::{NotesFolder, SourceId};
use thiserror::Error;

use crate::{MAX_REPOSITORY_PATH_BYTES, check_cancelled, isolated_options};

pub const SNAPSHOT_FORMAT_VERSION: u32 = 1;
pub const SNAPSHOT_CONTENT_DIRECTORY: &str = "content";
pub const SNAPSHOT_MANIFEST_FILE: &str = "snapshot.json";

pub const MAX_SNAPSHOT_ENTRIES: u64 = 100_000;
pub const MAX_ORG_FILE_BYTES: u64 = 8 * 1024 * 1024;
pub const MAX_ASSET_FILE_BYTES: u64 = 32 * 1024 * 1024;
pub const MAX_SNAPSHOT_BYTES: u64 = 512 * 1024 * 1024;
pub const MAX_SNAPSHOT_DIAGNOSTICS: usize = 64;
pub const MAX_SNAPSHOT_PATH_BYTES: usize = 1_024;
pub const MAX_SNAPSHOT_MANIFEST_BYTES: u64 = 96 * 1_024;

const MAX_TREE_DEPTH: usize = 64;
const MAX_TREE_OBJECT_BYTES: u64 = 8 * 1024 * 1024;
const MAX_COMMIT_OBJECT_BYTES: u64 = 1024 * 1024;
const MAX_ATTRIBUTES_BYTES: u64 = 1024 * 1024;
const MAX_SYMLINK_BYTES: u64 = 4_096;
const MAX_SYMLINK_EXPANSIONS: usize = 16;

/// Caller-adjustable limits bounded by the product maxima above.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotLimits {
    max_entries: u64,
    max_org_file_bytes: u64,
    max_asset_file_bytes: u64,
    max_total_bytes: u64,
}

impl SnapshotLimits {
    pub fn new(
        max_entries: u64,
        max_org_file_bytes: u64,
        max_asset_file_bytes: u64,
        max_total_bytes: u64,
    ) -> Result<Self, SnapshotError> {
        if max_entries == 0
            || max_entries > MAX_SNAPSHOT_ENTRIES
            || max_org_file_bytes == 0
            || max_org_file_bytes > MAX_ORG_FILE_BYTES
            || max_asset_file_bytes == 0
            || max_asset_file_bytes > MAX_ASSET_FILE_BYTES
            || max_total_bytes == 0
            || max_total_bytes > MAX_SNAPSHOT_BYTES
        {
            return Err(SnapshotError::LimitsRefused);
        }
        Ok(Self {
            max_entries,
            max_org_file_bytes,
            max_asset_file_bytes,
            max_total_bytes,
        })
    }
}

impl Default for SnapshotLimits {
    fn default() -> Self {
        Self {
            max_entries: MAX_SNAPSHOT_ENTRIES,
            max_org_file_bytes: MAX_ORG_FILE_BYTES,
            max_asset_file_bytes: MAX_ASSET_FILE_BYTES,
            max_total_bytes: MAX_SNAPSHOT_BYTES,
        }
    }
}

/// One immutable snapshot candidate and the bare object store it is derived
/// from. `destination` is never overwritten.
#[derive(Debug, Clone)]
pub struct SnapshotRequest {
    source: SourceId,
    repository: PathBuf,
    revision: String,
    notes_folder: NotesFolder,
    destination: PathBuf,
    limits: SnapshotLimits,
}

impl SnapshotRequest {
    pub fn new(
        source: SourceId,
        repository: PathBuf,
        revision: &str,
        notes_folder: NotesFolder,
        destination: PathBuf,
    ) -> Result<Self, SnapshotError> {
        validate_storage_path(&repository)?;
        validate_storage_path(&destination)?;
        let source_root = repository
            .parent()
            .ok_or(SnapshotError::DestinationRefused)?;
        if destination.starts_with(&repository) || repository.starts_with(&destination) {
            return Err(SnapshotError::DestinationRefused);
        }
        if !destination.starts_with(source_root) {
            return Err(SnapshotError::DestinationRefused);
        }
        validate_revision(revision)?;
        Ok(Self {
            source,
            repository,
            revision: revision.to_owned(),
            notes_folder,
            destination,
            limits: SnapshotLimits::default(),
        })
    }

    #[must_use]
    pub fn with_limits(mut self, limits: SnapshotLimits) -> Self {
        self.limits = limits;
        self
    }

    #[must_use]
    pub fn source(&self) -> &SourceId {
        &self.source
    }

    #[must_use]
    pub fn repository(&self) -> &Path {
        &self.repository
    }

    #[must_use]
    pub fn revision(&self) -> &str {
        &self.revision
    }

    #[must_use]
    pub fn notes_folder(&self) -> &NotesFolder {
        &self.notes_folder
    }

    #[must_use]
    pub fn destination(&self) -> &Path {
        &self.destination
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SnapshotDiagnosticReason {
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotDiagnostic {
    pub path: String,
    pub reason: SnapshotDiagnosticReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SnapshotDisposition {
    Created,
    Existing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotOutcome {
    pub disposition: SnapshotDisposition,
    pub source: SourceId,
    pub revision: String,
    pub notes_folder: NotesFolder,
    pub content_root: PathBuf,
    pub entries: u64,
    pub files: u64,
    pub org_files: u64,
    pub assets: u64,
    pub bytes: u64,
    pub diagnostics: Vec<SnapshotDiagnostic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotProgressStage {
    Preparing,
    Inspecting,
    Writing,
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotProgress {
    pub stage: SnapshotProgressStage,
    pub entries: u64,
    pub bytes: u64,
}

/// Closed failures. Repository paths and contents never become error text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum SnapshotError {
    #[error("the snapshot limits are not admitted")]
    LimitsRefused,
    #[error("the snapshot destination is not admitted")]
    DestinationRefused,
    #[error("the snapshot destination is already occupied")]
    DestinationOccupied,
    #[error("the snapshot storage boundary is not a real directory")]
    StorageBoundary,
    #[error("the snapshot belongs to another source")]
    OwnershipMismatch,
    #[error("the repository object store is not usable")]
    RepositoryInvalid,
    #[error("the requested revision is not admitted")]
    RevisionRefused,
    #[error("the requested revision is unavailable")]
    RevisionUnavailable,
    #[error("the selected notes folder is unavailable")]
    NotesFolderUnavailable,
    #[error("the repository contains an unsafe path")]
    UnsafePath,
    #[error("the repository object graph is malformed")]
    ObjectInvalid,
    #[error("the snapshot input budget was exhausted")]
    InputExhausted,
    #[error("the snapshot diagnostic budget was exhausted")]
    DiagnosticsExhausted,
    #[error("the operation was cancelled")]
    Cancelled,
    #[error("the snapshot could not be stored")]
    StorageFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SnapshotManifest {
    version: u32,
    source: SourceId,
    revision: String,
    notes_folder: NotesFolder,
    entries: u64,
    files: u64,
    org_files: u64,
    assets: u64,
    bytes: u64,
    diagnostics: Vec<SnapshotDiagnostic>,
}

/// Build a candidate snapshot from one exact commit. The candidate appears at
/// `destination` only after its complete content and ownership manifest exist.
pub fn materialize(
    request: &SnapshotRequest,
    cancelled: &AtomicBool,
    mut progress: impl FnMut(SnapshotProgress),
) -> Result<SnapshotOutcome, SnapshotError> {
    snapshot_cancelled(cancelled)?;
    let parent = request
        .destination()
        .parent()
        .ok_or(SnapshotError::DestinationRefused)?;
    let source_root = request
        .repository()
        .parent()
        .ok_or(SnapshotError::DestinationRefused)?;
    validate_real_directory_prefix(source_root, parent)?;
    progress(SnapshotProgress {
        stage: SnapshotProgressStage::Preparing,
        entries: 0,
        bytes: 0,
    });

    if let Some(existing) = existing_snapshot(request)? {
        snapshot_cancelled(cancelled)?;
        progress(SnapshotProgress {
            stage: SnapshotProgressStage::Complete,
            entries: existing.entries,
            bytes: existing.bytes,
        });
        return Ok(outcome_from_manifest(
            SnapshotDisposition::Existing,
            request.destination(),
            existing,
        ));
    }

    let repository_metadata =
        fs::symlink_metadata(request.repository()).map_err(|_| SnapshotError::RepositoryInvalid)?;
    if !repository_metadata.is_dir() || repository_metadata.file_type().is_symlink() {
        return Err(SnapshotError::RepositoryInvalid);
    }
    let repository = gix::open_opts(request.repository(), isolated_options(false))
        .map_err(|_| SnapshotError::RepositoryInvalid)?;
    let revision = gix::ObjectId::from_hex(request.revision().as_bytes())
        .map_err(|_| SnapshotError::RevisionRefused)?;
    let commit_header = repository
        .find_header(revision)
        .map_err(|_| SnapshotError::RevisionUnavailable)?;
    if commit_header.kind() != gix::objs::Kind::Commit
        || commit_header.size() > MAX_COMMIT_OBJECT_BYTES
    {
        return Err(SnapshotError::RevisionUnavailable);
    }
    let commit = repository
        .find_commit(revision)
        .map_err(|_| SnapshotError::RevisionUnavailable)?;
    let tree_id = commit
        .tree_id()
        .map_err(|_| SnapshotError::ObjectInvalid)?
        .detach();
    ensure_tree_header(&repository, tree_id)?;
    let root = repository
        .find_tree(tree_id)
        .map_err(|_| SnapshotError::ObjectInvalid)?;
    validate_notes_folder(&root, request.notes_folder())?;

    ensure_real_directory(source_root, parent)?;
    let staging = tempfile::Builder::new()
        .prefix(".slipbox-stage-")
        .tempdir_in(parent)
        .map_err(|_| SnapshotError::StorageFailed)?;
    let content_root = staging.path().join(SNAPSHOT_CONTENT_DIRECTORY);
    create_private_directory(&content_root)?;

    progress(SnapshotProgress {
        stage: SnapshotProgressStage::Inspecting,
        entries: 0,
        bytes: 0,
    });
    let mut state = MaterializeState {
        repository: &repository,
        root: &root,
        notes: request.notes_folder(),
        content_root: &content_root,
        limits: request.limits,
        cancelled,
        progress: &mut progress,
        entries: 0,
        files: 0,
        org_files: 0,
        assets: 0,
        bytes: 0,
        diagnostics: Vec::new(),
    };
    state.walk_tree(&root, &[], &[], 0, 0)?;
    snapshot_cancelled(cancelled)?;

    let manifest = SnapshotManifest {
        version: SNAPSHOT_FORMAT_VERSION,
        source: request.source().clone(),
        revision: request.revision().to_owned(),
        notes_folder: request.notes_folder().clone(),
        entries: state.entries,
        files: state.files,
        org_files: state.org_files,
        assets: state.assets,
        bytes: state.bytes,
        diagnostics: state.diagnostics,
    };
    let manifest_bytes = serde_json::to_vec(&manifest).map_err(|_| SnapshotError::StorageFailed)?;
    if manifest_bytes.len() as u64 > MAX_SNAPSHOT_MANIFEST_BYTES {
        return Err(SnapshotError::DiagnosticsExhausted);
    }
    write_private_file(
        &staging.path().join(SNAPSHOT_MANIFEST_FILE),
        &manifest_bytes,
    )?;
    snapshot_cancelled(cancelled)?;
    if request.destination().exists() {
        return Err(SnapshotError::DestinationOccupied);
    }
    fs::rename(staging.path(), request.destination()).map_err(|_| SnapshotError::StorageFailed)?;
    let _ = staging.keep();

    progress(SnapshotProgress {
        stage: SnapshotProgressStage::Complete,
        entries: manifest.entries,
        bytes: manifest.bytes,
    });
    Ok(outcome_from_manifest(
        SnapshotDisposition::Created,
        request.destination(),
        manifest,
    ))
}

fn outcome_from_manifest(
    disposition: SnapshotDisposition,
    destination: &Path,
    manifest: SnapshotManifest,
) -> SnapshotOutcome {
    SnapshotOutcome {
        disposition,
        source: manifest.source,
        revision: manifest.revision,
        notes_folder: manifest.notes_folder,
        content_root: destination.join(SNAPSHOT_CONTENT_DIRECTORY),
        entries: manifest.entries,
        files: manifest.files,
        org_files: manifest.org_files,
        assets: manifest.assets,
        bytes: manifest.bytes,
        diagnostics: manifest.diagnostics,
    }
}

fn existing_snapshot(request: &SnapshotRequest) -> Result<Option<SnapshotManifest>, SnapshotError> {
    let metadata = match fs::symlink_metadata(request.destination()) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(SnapshotError::StorageFailed),
    };
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(SnapshotError::DestinationOccupied);
    }
    let manifest_path = request.destination().join(SNAPSHOT_MANIFEST_FILE);
    let manifest_metadata =
        fs::symlink_metadata(&manifest_path).map_err(|_| SnapshotError::DestinationOccupied)?;
    if !manifest_metadata.is_file() || manifest_metadata.len() > MAX_SNAPSHOT_MANIFEST_BYTES {
        return Err(SnapshotError::DestinationOccupied);
    }
    let bytes = fs::read(manifest_path).map_err(|_| SnapshotError::StorageFailed)?;
    let manifest: SnapshotManifest =
        serde_json::from_slice(&bytes).map_err(|_| SnapshotError::DestinationOccupied)?;
    if manifest.source != *request.source() {
        return Err(SnapshotError::OwnershipMismatch);
    }
    if manifest.version != SNAPSHOT_FORMAT_VERSION
        || manifest.revision != request.revision()
        || manifest.notes_folder != *request.notes_folder()
        || !manifest_counts_are_valid(&manifest, request.limits)
        || manifest.diagnostics.len() > MAX_SNAPSHOT_DIAGNOSTICS
        || manifest.diagnostics.iter().any(|diagnostic| {
            diagnostic.path.is_empty() || diagnostic.path.len() > MAX_SNAPSHOT_PATH_BYTES
        })
    {
        return Err(SnapshotError::DestinationOccupied);
    }
    let content = fs::symlink_metadata(request.destination().join(SNAPSHOT_CONTENT_DIRECTORY))
        .map_err(|_| SnapshotError::DestinationOccupied)?;
    if !content.is_dir() || content.file_type().is_symlink() {
        return Err(SnapshotError::DestinationOccupied);
    }
    Ok(Some(manifest))
}

fn manifest_counts_are_valid(manifest: &SnapshotManifest, limits: SnapshotLimits) -> bool {
    manifest.entries <= limits.max_entries
        && manifest.files <= manifest.entries
        && manifest.org_files.checked_add(manifest.assets) == Some(manifest.files)
        && manifest.bytes <= limits.max_total_bytes
}

struct MaterializeState<'a, P> {
    repository: &'a gix::Repository,
    root: &'a gix::Tree<'a>,
    notes: &'a NotesFolder,
    content_root: &'a Path,
    limits: SnapshotLimits,
    cancelled: &'a AtomicBool,
    progress: &'a mut P,
    entries: u64,
    files: u64,
    org_files: u64,
    assets: u64,
    bytes: u64,
    diagnostics: Vec<SnapshotDiagnostic>,
}

impl<P> MaterializeState<'_, P>
where
    P: FnMut(SnapshotProgress),
{
    fn walk_tree(
        &mut self,
        tree: &gix::Tree<'_>,
        source_prefix: &[String],
        output_prefix: &[String],
        depth: usize,
        symlink_expansions: usize,
    ) -> Result<(), SnapshotError> {
        if depth > MAX_TREE_DEPTH {
            return Err(SnapshotError::UnsafePath);
        }
        snapshot_cancelled(self.cancelled)?;
        for entry in tree.iter() {
            snapshot_cancelled(self.cancelled)?;
            let entry = entry.map_err(|_| SnapshotError::ObjectInvalid)?;
            self.entries = self
                .entries
                .checked_add(1)
                .filter(|entries| *entries <= self.limits.max_entries)
                .ok_or(SnapshotError::InputExhausted)?;
            let component = validate_component(entry.filename())?;
            let source_path = appended(source_prefix, &component)?;
            let output_path = appended(output_prefix, &component)?;
            let kind = entry.kind();
            let object_id = entry.object_id();
            match kind {
                EntryKind::Tree => {
                    ensure_tree_header(self.repository, object_id)?;
                    let child = self
                        .repository
                        .find_tree(object_id)
                        .map_err(|_| SnapshotError::ObjectInvalid)?;
                    self.walk_tree(
                        &child,
                        &source_path,
                        &output_path,
                        depth + 1,
                        symlink_expansions,
                    )?;
                }
                EntryKind::Blob | EntryKind::BlobExecutable => {
                    self.process_blob(object_id, &output_path)?;
                }
                EntryKind::Commit => {
                    self.diagnostic(&output_path, SnapshotDiagnosticReason::Submodule)?;
                }
                EntryKind::Link => {
                    self.process_symlink(&source_path, &output_path, depth, symlink_expansions)?;
                }
            }
        }
        Ok(())
    }

    fn process_symlink(
        &mut self,
        source_path: &[String],
        output_path: &[String],
        depth: usize,
        symlink_expansions: usize,
    ) -> Result<(), SnapshotError> {
        if symlink_expansions >= MAX_SYMLINK_EXPANSIONS {
            return self.diagnostic(output_path, SnapshotDiagnosticReason::SymlinkCycle);
        }
        let resolved = match resolve_path(self.repository, self.root, source_path)? {
            Ok(resolved) => resolved,
            Err(reason) => return self.diagnostic(output_path, reason),
        };
        match resolved.kind {
            EntryKind::Tree => {
                ensure_tree_header(self.repository, resolved.object_id)?;
                let tree = self
                    .repository
                    .find_tree(resolved.object_id)
                    .map_err(|_| SnapshotError::ObjectInvalid)?;
                self.walk_tree(
                    &tree,
                    &resolved.path,
                    output_path,
                    depth + 1,
                    symlink_expansions + 1,
                )
            }
            EntryKind::Blob | EntryKind::BlobExecutable => {
                self.process_blob(resolved.object_id, output_path)
            }
            EntryKind::Commit => self.diagnostic(output_path, SnapshotDiagnosticReason::Submodule),
            EntryKind::Link => self.diagnostic(output_path, SnapshotDiagnosticReason::SymlinkCycle),
        }
    }

    fn process_blob(
        &mut self,
        object_id: gix::ObjectId,
        path: &[String],
    ) -> Result<(), SnapshotError> {
        let policy = file_policy(path, self.notes);
        let limit = match policy {
            FilePolicy::Skip => return Ok(()),
            FilePolicy::EncryptedOrg => {
                return self.diagnostic(path, SnapshotDiagnosticReason::EncryptedOrg);
            }
            FilePolicy::Attributes => MAX_ATTRIBUTES_BYTES,
            FilePolicy::Org => self.limits.max_org_file_bytes,
            FilePolicy::Asset { .. } => self.limits.max_asset_file_bytes,
        };
        let header = self
            .repository
            .find_header(object_id)
            .map_err(|_| SnapshotError::ObjectInvalid)?;
        if header.kind() != gix::objs::Kind::Blob {
            return Err(SnapshotError::ObjectInvalid);
        }
        if header.size() > limit {
            return self.diagnostic(path, SnapshotDiagnosticReason::OversizedInput);
        }
        let blob = self
            .repository
            .find_blob(object_id)
            .map_err(|_| SnapshotError::ObjectInvalid)?;
        if blob.data.len() as u64 != header.size() {
            return Err(SnapshotError::ObjectInvalid);
        }
        if matches!(policy, FilePolicy::Attributes) {
            if attributes_request_filter(&blob.data) {
                self.diagnostic(path, SnapshotDiagnosticReason::ExternalFilter)?;
            }
            return Ok(());
        }
        if is_lfs_pointer(&blob.data) {
            return self.diagnostic(path, SnapshotDiagnosticReason::LfsPointer);
        }
        if matches!(policy, FilePolicy::Org | FilePolicy::Asset { text: true }) {
            let text = match std::str::from_utf8(&blob.data) {
                Ok(text) => text,
                Err(_) => {
                    return self.diagnostic(path, SnapshotDiagnosticReason::UnsupportedEncoding);
                }
            };
            if text.contains('\0') {
                return self.diagnostic(path, SnapshotDiagnosticReason::UnsupportedFormat);
            }
        }
        let next_bytes = self
            .bytes
            .checked_add(blob.data.len() as u64)
            .filter(|bytes| *bytes <= self.limits.max_total_bytes)
            .ok_or(SnapshotError::InputExhausted)?;
        snapshot_cancelled(self.cancelled)?;
        (self.progress)(SnapshotProgress {
            stage: SnapshotProgressStage::Writing,
            entries: self.entries,
            bytes: self.bytes,
        });
        let relative = path_to_pathbuf(path);
        write_private_file(&self.content_root.join(relative), &blob.data)?;
        self.bytes = next_bytes;
        self.files += 1;
        match policy {
            FilePolicy::Org => self.org_files += 1,
            FilePolicy::Asset { .. } => self.assets += 1,
            FilePolicy::Skip | FilePolicy::EncryptedOrg | FilePolicy::Attributes => {}
        }
        Ok(())
    }

    fn diagnostic(
        &mut self,
        path: &[String],
        reason: SnapshotDiagnosticReason,
    ) -> Result<(), SnapshotError> {
        if self.diagnostics.len() == MAX_SNAPSHOT_DIAGNOSTICS {
            return Err(SnapshotError::DiagnosticsExhausted);
        }
        self.diagnostics.push(SnapshotDiagnostic {
            path: path.join("/"),
            reason,
        });
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FilePolicy {
    Skip,
    Org,
    Asset { text: bool },
    EncryptedOrg,
    Attributes,
}

fn file_policy(path: &[String], notes: &NotesFolder) -> FilePolicy {
    let Some(name) = path.last() else {
        return FilePolicy::Skip;
    };
    if name == ".gitattributes" {
        return FilePolicy::Attributes;
    }
    let lowercase = name.to_ascii_lowercase();
    if is_within_notes(path, notes) {
        if lowercase.ends_with(".org.gpg") || lowercase.ends_with(".org.age") {
            return FilePolicy::EncryptedOrg;
        }
        if lowercase.ends_with(".org") {
            return FilePolicy::Org;
        }
    }
    match lowercase.rsplit_once('.').map(|(_, extension)| extension) {
        Some("png" | "jpg" | "jpeg" | "gif" | "webp") => FilePolicy::Asset { text: false },
        Some("txt") => FilePolicy::Asset { text: true },
        _ => FilePolicy::Skip,
    }
}

fn is_within_notes(path: &[String], notes: &NotesFolder) -> bool {
    if notes.is_root() {
        return true;
    }
    let mut components = notes.as_str().split('/');
    path.iter()
        .map(String::as_str)
        .zip(&mut components)
        .all(|(actual, expected)| actual == expected)
        && path.len() >= notes.as_str().split('/').count()
}

#[derive(Debug)]
struct ResolvedEntry {
    kind: EntryKind,
    object_id: gix::ObjectId,
    path: Vec<String>,
}

fn resolve_path(
    repository: &gix::Repository,
    root: &gix::Tree<'_>,
    initial: &[String],
) -> Result<Result<ResolvedEntry, SnapshotDiagnosticReason>, SnapshotError> {
    let mut pending = VecDeque::from(initial.to_vec());
    let mut resolved = Vec::new();
    let mut seen = BTreeSet::new();
    let mut expansions = 0;
    loop {
        let Some(component) = pending.pop_front() else {
            return Ok(Err(SnapshotDiagnosticReason::SymlinkUnavailable));
        };
        let lookup = appended(&resolved, &component)?;
        let entry = root
            .lookup_entry(lookup.iter().map(String::as_bytes))
            .map_err(|_| SnapshotError::ObjectInvalid)?;
        let Some(entry) = entry else {
            return Ok(Err(SnapshotDiagnosticReason::SymlinkUnavailable));
        };
        if entry.mode().kind() == EntryKind::Link {
            expansions += 1;
            if expansions > MAX_SYMLINK_EXPANSIONS {
                return Ok(Err(SnapshotDiagnosticReason::SymlinkCycle));
            }
            let key = lookup.join("/");
            if !seen.insert(key) {
                return Ok(Err(SnapshotDiagnosticReason::SymlinkCycle));
            }
            let target = read_symlink(repository, entry.object_id())?;
            let target = match target {
                Ok(target) => target,
                Err(reason) => return Ok(Err(reason)),
            };
            let mut redirected = match normalize_symlink_target(&resolved, &target) {
                Ok(path) => VecDeque::from(path),
                Err(reason) => return Ok(Err(reason)),
            };
            redirected.append(&mut pending);
            pending = redirected;
            resolved.clear();
            continue;
        }
        if pending.is_empty() {
            return Ok(Ok(ResolvedEntry {
                kind: entry.mode().kind(),
                object_id: entry.object_id(),
                path: lookup,
            }));
        }
        if entry.mode().kind() != EntryKind::Tree {
            return Ok(Err(SnapshotDiagnosticReason::SymlinkUnavailable));
        }
        resolved.push(component);
    }
}

fn read_symlink(
    repository: &gix::Repository,
    object_id: gix::ObjectId,
) -> Result<Result<String, SnapshotDiagnosticReason>, SnapshotError> {
    let header = repository
        .find_header(object_id)
        .map_err(|_| SnapshotError::ObjectInvalid)?;
    if header.kind() != gix::objs::Kind::Blob {
        return Err(SnapshotError::ObjectInvalid);
    }
    if header.size() > MAX_SYMLINK_BYTES {
        return Ok(Err(SnapshotDiagnosticReason::SymlinkUnavailable));
    }
    let blob = repository
        .find_blob(object_id)
        .map_err(|_| SnapshotError::ObjectInvalid)?;
    let target = match std::str::from_utf8(&blob.data) {
        Ok(target) => target,
        Err(_) => return Ok(Err(SnapshotDiagnosticReason::UnsupportedEncoding)),
    };
    if target.is_empty()
        || target.starts_with('/')
        || target.contains('\0')
        || target.contains('\\')
        || target.chars().any(char::is_control)
    {
        return Ok(Err(SnapshotDiagnosticReason::SymlinkEscapes));
    }
    Ok(Ok(target.to_owned()))
}

fn normalize_symlink_target(
    parent: &[String],
    target: &str,
) -> Result<Vec<String>, SnapshotDiagnosticReason> {
    let mut normalized = parent.to_vec();
    for component in target.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                if normalized.pop().is_none() {
                    return Err(SnapshotDiagnosticReason::SymlinkEscapes);
                }
            }
            component => {
                if validate_component(component.as_bytes()).is_err() {
                    return Err(SnapshotDiagnosticReason::SymlinkEscapes);
                }
                normalized.push(component.to_owned());
            }
        }
    }
    if normalized.is_empty() {
        return Err(SnapshotDiagnosticReason::SymlinkUnavailable);
    }
    Ok(normalized)
}

fn validate_notes_folder(root: &gix::Tree<'_>, notes: &NotesFolder) -> Result<(), SnapshotError> {
    if notes.is_root() {
        return Ok(());
    }
    let entry = root
        .lookup_entry(notes.as_str().split('/').map(str::as_bytes))
        .map_err(|_| SnapshotError::ObjectInvalid)?
        .ok_or(SnapshotError::NotesFolderUnavailable)?;
    if entry.mode().kind() != EntryKind::Tree {
        return Err(SnapshotError::NotesFolderUnavailable);
    }
    Ok(())
}

fn ensure_tree_header(
    repository: &gix::Repository,
    object_id: gix::ObjectId,
) -> Result<(), SnapshotError> {
    let header = repository
        .find_header(object_id)
        .map_err(|_| SnapshotError::ObjectInvalid)?;
    if header.kind() != gix::objs::Kind::Tree || header.size() > MAX_TREE_OBJECT_BYTES {
        return Err(SnapshotError::ObjectInvalid);
    }
    Ok(())
}

fn validate_component(bytes: &[u8]) -> Result<String, SnapshotError> {
    let component = std::str::from_utf8(bytes).map_err(|_| SnapshotError::UnsafePath)?;
    if component.is_empty()
        || component.len() > 255
        || matches!(component, "." | "..")
        || component.eq_ignore_ascii_case(".git")
        || component.contains(['/', '\\', ':', '\0'])
        || component.chars().any(char::is_control)
    {
        return Err(SnapshotError::UnsafePath);
    }
    Ok(component.to_owned())
}

fn appended(prefix: &[String], component: &str) -> Result<Vec<String>, SnapshotError> {
    let mut path = prefix.to_vec();
    path.push(component.to_owned());
    if path.len() > MAX_TREE_DEPTH || path.join("/").len() > MAX_SNAPSHOT_PATH_BYTES {
        return Err(SnapshotError::UnsafePath);
    }
    Ok(path)
}

fn path_to_pathbuf(path: &[String]) -> PathBuf {
    path.iter().collect()
}

fn attributes_request_filter(bytes: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return true;
    };
    text.lines().any(|line| {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            return false;
        }
        line.split_ascii_whitespace().skip(1).any(|attribute| {
            let attribute = attribute.trim_start_matches(['-', '!']);
            attribute == "filter" || attribute.starts_with("filter=")
        })
    })
}

fn is_lfs_pointer(bytes: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    let mut lines = text.lines();
    lines.next() == Some("version https://git-lfs.github.com/spec/v1")
        && lines.any(|line| line.starts_with("oid sha256:"))
}

fn validate_revision(revision: &str) -> Result<(), SnapshotError> {
    if revision.len() != 40
        || !revision
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(SnapshotError::RevisionRefused);
    }
    Ok(())
}

fn validate_storage_path(path: &Path) -> Result<(), SnapshotError> {
    if !path.is_absolute()
        || path.as_os_str().as_encoded_bytes().is_empty()
        || path.as_os_str().as_encoded_bytes().len() > MAX_REPOSITORY_PATH_BYTES
        || path.parent().is_none()
        || path
            .components()
            .any(|component| !matches!(component, Component::RootDir | Component::Normal(_)))
    {
        return Err(SnapshotError::DestinationRefused);
    }
    Ok(())
}

fn validate_real_directory_prefix(source_root: &Path, path: &Path) -> Result<(), SnapshotError> {
    let root_metadata =
        fs::symlink_metadata(source_root).map_err(|_| SnapshotError::StorageBoundary)?;
    if !root_metadata.is_dir() || root_metadata.file_type().is_symlink() {
        return Err(SnapshotError::StorageBoundary);
    }
    let relative = path
        .strip_prefix(source_root)
        .map_err(|_| SnapshotError::DestinationRefused)?;
    let mut current = source_root.to_path_buf();
    for component in relative.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(SnapshotError::DestinationRefused);
        }
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if !metadata.is_dir() || metadata.file_type().is_symlink() {
                    return Err(SnapshotError::StorageBoundary);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(_) => return Err(SnapshotError::StorageFailed),
        }
    }
    Ok(())
}

fn ensure_real_directory(source_root: &Path, path: &Path) -> Result<(), SnapshotError> {
    let root_metadata =
        fs::symlink_metadata(source_root).map_err(|_| SnapshotError::StorageBoundary)?;
    if !root_metadata.is_dir() || root_metadata.file_type().is_symlink() {
        return Err(SnapshotError::StorageBoundary);
    }
    let relative = path
        .strip_prefix(source_root)
        .map_err(|_| SnapshotError::DestinationRefused)?;
    let mut current = source_root.to_path_buf();
    for component in relative.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(SnapshotError::DestinationRefused);
        }
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if !metadata.is_dir() || metadata.file_type().is_symlink() {
                    return Err(SnapshotError::StorageBoundary);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&current).map_err(|_| SnapshotError::StorageFailed)?;
                set_private_directory_permissions(&current)?;
            }
            Err(_) => return Err(SnapshotError::StorageFailed),
        }
    }
    Ok(())
}

fn create_private_directory(path: &Path) -> Result<(), SnapshotError> {
    fs::create_dir(path).map_err(|_| SnapshotError::StorageFailed)?;
    set_private_directory_permissions(path)
}

fn write_private_file(path: &Path, bytes: &[u8]) -> Result<(), SnapshotError> {
    let parent = path.parent().ok_or(SnapshotError::UnsafePath)?;
    create_private_descendants(parent)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| SnapshotError::StorageFailed)?;
    set_private_file_permissions(&file)?;
    file.write_all(bytes)
        .map_err(|_| SnapshotError::StorageFailed)?;
    file.sync_data().map_err(|_| SnapshotError::StorageFailed)
}

fn create_private_descendants(path: &Path) -> Result<(), SnapshotError> {
    if path.is_dir() {
        return Ok(());
    }
    let parent = path.parent().ok_or(SnapshotError::UnsafePath)?;
    create_private_descendants(parent)?;
    create_private_directory(path)
}

#[cfg(unix)]
fn set_private_directory_permissions(path: &Path) -> Result<(), SnapshotError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|_| SnapshotError::StorageFailed)
}

#[cfg(not(unix))]
fn set_private_directory_permissions(_path: &Path) -> Result<(), SnapshotError> {
    Ok(())
}

#[cfg(unix)]
fn set_private_file_permissions(file: &File) -> Result<(), SnapshotError> {
    use std::os::unix::fs::PermissionsExt;
    file.set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(|_| SnapshotError::StorageFailed)
}

#[cfg(not(unix))]
fn set_private_file_permissions(_file: &File) -> Result<(), SnapshotError> {
    Ok(())
}

fn snapshot_cancelled(cancelled: &AtomicBool) -> Result<(), SnapshotError> {
    check_cancelled(cancelled).map_err(|_| SnapshotError::Cancelled)
}

#[cfg(test)]
#[path = "snapshot_tests.rs"]
mod tests;
