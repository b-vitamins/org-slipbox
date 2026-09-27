//! Bounded changed-file inventories derived directly from repository objects.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use gix::object::tree::EntryKind;
use slipbox_core::{NotesFolder, SourceId};
use thiserror::Error;

use super::{
    BlobInspection, FilePolicy, MAX_COMMIT_OBJECT_BYTES, MAX_SNAPSHOT_DIAGNOSTICS,
    MAX_SYMLINK_EXPANSIONS, MAX_TREE_DEPTH, MAX_TREE_OBJECT_BYTES, SnapshotError, SnapshotLimits,
    appended, ensure_tree_header, inspect_blob, resolve_path, snapshot_cancelled,
    validate_component, validate_notes_folder, validate_revision, validate_storage_path,
};
use crate::isolated_options;

pub const MAX_DELTA_CHANGES: u64 = 100_000;
pub const MAX_DELTA_HISTORY_COMMITS: u64 = 4_096;
pub const MAX_DELTA_RENAME_COMPARISONS: u64 = 16_384;
pub const MAX_DELTA_FINGERPRINT_TOKENS: u64 = 65_536;
pub const MAX_DELTA_SIMILARITY_STEPS: u64 = 1_000_000;

const RENAME_SIMILARITY_PERCENT: u16 = 60;

/// Caller-adjustable work limits bounded by the product maxima above.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeltaLimits {
    snapshot: SnapshotLimits,
    max_changes: u64,
    max_history_commits: u64,
    max_rename_comparisons: u64,
}

impl DeltaLimits {
    pub fn new(
        max_changes: u64,
        max_history_commits: u64,
        max_rename_comparisons: u64,
    ) -> Result<Self, DeltaError> {
        if max_changes == 0
            || max_changes > MAX_DELTA_CHANGES
            || max_history_commits == 0
            || max_history_commits > MAX_DELTA_HISTORY_COMMITS
            || max_rename_comparisons == 0
            || max_rename_comparisons > MAX_DELTA_RENAME_COMPARISONS
        {
            return Err(DeltaError::LimitsRefused);
        }
        Ok(Self {
            snapshot: SnapshotLimits::default(),
            max_changes,
            max_history_commits,
            max_rename_comparisons,
        })
    }

    #[must_use]
    pub fn with_snapshot_limits(mut self, limits: SnapshotLimits) -> Self {
        self.snapshot = limits;
        self
    }
}

impl Default for DeltaLimits {
    fn default() -> Self {
        Self {
            snapshot: SnapshotLimits::default(),
            max_changes: MAX_DELTA_CHANGES,
            max_history_commits: MAX_DELTA_HISTORY_COMMITS,
            max_rename_comparisons: MAX_DELTA_RENAME_COMPARISONS,
        }
    }
}

/// Exact revisions and source policy used to derive one changed-file inventory.
#[derive(Debug, Clone)]
pub struct DeltaRequest {
    source: SourceId,
    repository: PathBuf,
    previous_revision: Option<String>,
    revision: String,
    previous_notes_folder: NotesFolder,
    notes_folder: NotesFolder,
    limits: DeltaLimits,
}

impl DeltaRequest {
    pub fn between(
        source: SourceId,
        repository: PathBuf,
        previous_revision: &str,
        revision: &str,
        notes_folder: NotesFolder,
    ) -> Result<Self, DeltaError> {
        validate_delta_repository(&repository)?;
        validate_delta_revision(previous_revision)?;
        validate_delta_revision(revision)?;
        Ok(Self {
            source,
            repository,
            previous_revision: Some(previous_revision.to_owned()),
            revision: revision.to_owned(),
            previous_notes_folder: notes_folder.clone(),
            notes_folder,
            limits: DeltaLimits::default(),
        })
    }

    pub fn initial(
        source: SourceId,
        repository: PathBuf,
        revision: &str,
        notes_folder: NotesFolder,
    ) -> Result<Self, DeltaError> {
        validate_delta_repository(&repository)?;
        validate_delta_revision(revision)?;
        Ok(Self {
            source,
            repository,
            previous_revision: None,
            revision: revision.to_owned(),
            previous_notes_folder: notes_folder.clone(),
            notes_folder,
            limits: DeltaLimits::default(),
        })
    }

    #[must_use]
    pub fn with_previous_notes_folder(mut self, notes_folder: NotesFolder) -> Self {
        self.previous_notes_folder = notes_folder;
        self
    }

    #[must_use]
    pub fn with_limits(mut self, limits: DeltaLimits) -> Self {
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
    pub fn previous_revision(&self) -> Option<&str> {
        self.previous_revision.as_deref()
    }

    #[must_use]
    pub fn previous_notes_folder(&self) -> Option<&NotesFolder> {
        self.previous_revision
            .as_ref()
            .map(|_| &self.previous_notes_folder)
    }

    #[must_use]
    pub fn revision(&self) -> &str {
        &self.revision
    }

    #[must_use]
    pub fn notes_folder(&self) -> &NotesFolder {
        &self.notes_folder
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DeltaFileKind {
    Org,
    Asset,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeltaFile {
    pub path: String,
    pub kind: DeltaFileKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenameHint {
    pub from: String,
    pub to: String,
    pub kind: DeltaFileKind,
    pub content_changed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeltaDisposition {
    Unchanged,
    Changed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeltaHistory {
    Initial,
    SameRevision,
    FastForward,
    Rewind,
    Diverged,
    Incomplete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenameDetection {
    Complete,
    Limited,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeltaOutcome {
    pub disposition: DeltaDisposition,
    pub source: SourceId,
    pub previous_revision: Option<String>,
    pub revision: String,
    pub previous_notes_folder: Option<NotesFolder>,
    pub notes_folder: NotesFolder,
    pub history: DeltaHistory,
    pub rename_detection: RenameDetection,
    pub added: Vec<DeltaFile>,
    pub modified: Vec<DeltaFile>,
    pub deleted: Vec<DeltaFile>,
    pub renamed: Vec<RenameHint>,
}

impl DeltaOutcome {
    #[must_use]
    pub fn change_count(&self) -> usize {
        self.added.len() + self.modified.len() + self.deleted.len() + self.renamed.len()
    }
}

/// Closed failures. Repository paths, revisions and contents never become error text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum DeltaError {
    #[error("the delta limits are not admitted")]
    LimitsRefused,
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
    #[error("the delta input budget was exhausted")]
    InputExhausted,
    #[error("the snapshot diagnostic budget was exhausted")]
    DiagnosticsExhausted,
    #[error("the operation was cancelled")]
    Cancelled,
}

/// Compare exact commit trees without consulting a worktree, repository
/// configuration, merge base or provider API.
pub fn derive_delta(
    request: &DeltaRequest,
    cancelled: &AtomicBool,
) -> Result<DeltaOutcome, DeltaError> {
    delta_cancelled(cancelled)?;
    let metadata =
        fs::symlink_metadata(request.repository()).map_err(|_| DeltaError::RepositoryInvalid)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(DeltaError::RepositoryInvalid);
    }
    let repository = gix::open_opts(request.repository(), isolated_options(false))
        .map_err(|_| DeltaError::RepositoryInvalid)?;
    let revision = parse_revision(request.revision())?;
    let root = load_root(&repository, revision)?;
    validate_notes_folder(&root, request.notes_folder()).map_err(map_snapshot_error)?;

    if request.previous_revision() == Some(request.revision())
        && request.previous_notes_folder == request.notes_folder
    {
        return Ok(empty_outcome(request, DeltaHistory::SameRevision));
    }

    let current = Inventory::collect(
        &repository,
        &root,
        request.notes_folder(),
        request.limits.snapshot,
        cancelled,
    )?;
    let (previous, history) = match request.previous_revision() {
        None => (Inventory::default(), DeltaHistory::Initial),
        Some(previous_revision) => {
            let previous_id = parse_revision(previous_revision)?;
            let previous_root = load_root(&repository, previous_id)?;
            validate_notes_folder(&previous_root, &request.previous_notes_folder)
                .map_err(map_snapshot_error)?;
            let previous = Inventory::collect(
                &repository,
                &previous_root,
                &request.previous_notes_folder,
                request.limits.snapshot,
                cancelled,
            )?;
            let history = classify_history(
                &repository,
                previous_id,
                revision,
                request.limits.max_history_commits,
                cancelled,
            )?;
            (previous, history)
        }
    };

    assemble_delta(request, &repository, previous, current, history, cancelled)
}

fn empty_outcome(request: &DeltaRequest, history: DeltaHistory) -> DeltaOutcome {
    DeltaOutcome {
        disposition: DeltaDisposition::Unchanged,
        source: request.source().clone(),
        previous_revision: request.previous_revision.clone(),
        revision: request.revision.clone(),
        previous_notes_folder: request.previous_notes_folder().cloned(),
        notes_folder: request.notes_folder.clone(),
        history,
        rename_detection: RenameDetection::Complete,
        added: Vec::new(),
        modified: Vec::new(),
        deleted: Vec::new(),
        renamed: Vec::new(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct InventoryEntry {
    kind: DeltaFileKind,
    object_id: gix::ObjectId,
    text: bool,
}

#[derive(Default)]
struct Inventory {
    files: BTreeMap<String, InventoryEntry>,
}

struct InventoryState<'a> {
    repository: &'a gix::Repository,
    root: &'a gix::Tree<'a>,
    notes: &'a NotesFolder,
    limits: SnapshotLimits,
    cancelled: &'a AtomicBool,
    entries: u64,
    bytes: u64,
    diagnostics: usize,
    files: BTreeMap<String, InventoryEntry>,
}

impl Inventory {
    fn collect(
        repository: &gix::Repository,
        root: &gix::Tree<'_>,
        notes: &NotesFolder,
        limits: SnapshotLimits,
        cancelled: &AtomicBool,
    ) -> Result<Self, DeltaError> {
        let mut state = InventoryState {
            repository,
            root,
            notes,
            limits,
            cancelled,
            entries: 0,
            bytes: 0,
            diagnostics: 0,
            files: BTreeMap::new(),
        };
        state.walk_tree(root, &[], &[], 0, 0)?;
        Ok(Self { files: state.files })
    }
}

impl InventoryState<'_> {
    fn walk_tree(
        &mut self,
        tree: &gix::Tree<'_>,
        source_prefix: &[String],
        output_prefix: &[String],
        depth: usize,
        symlink_expansions: usize,
    ) -> Result<(), DeltaError> {
        if depth > MAX_TREE_DEPTH {
            return Err(DeltaError::UnsafePath);
        }
        delta_cancelled(self.cancelled)?;
        for entry in tree.iter() {
            delta_cancelled(self.cancelled)?;
            let entry = entry.map_err(|_| DeltaError::ObjectInvalid)?;
            self.entries = self
                .entries
                .checked_add(1)
                .filter(|entries| *entries <= self.limits.max_entries)
                .ok_or(DeltaError::InputExhausted)?;
            let component = validate_component(entry.filename()).map_err(map_snapshot_error)?;
            let source_path = appended(source_prefix, &component).map_err(map_snapshot_error)?;
            let output_path = appended(output_prefix, &component).map_err(map_snapshot_error)?;
            let object_id = entry.object_id();
            match entry.kind() {
                EntryKind::Tree => {
                    ensure_tree_header(self.repository, object_id).map_err(map_snapshot_error)?;
                    let child = self
                        .repository
                        .find_tree(object_id)
                        .map_err(|_| DeltaError::ObjectInvalid)?;
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
                EntryKind::Commit => self.diagnostic()?,
                EntryKind::Link => {
                    self.process_symlink(&source_path, &output_path, depth, symlink_expansions)?
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
    ) -> Result<(), DeltaError> {
        if symlink_expansions >= MAX_SYMLINK_EXPANSIONS {
            return self.diagnostic();
        }
        let resolved = match resolve_path(self.repository, self.root, source_path)
            .map_err(map_snapshot_error)?
        {
            Ok(resolved) => resolved,
            Err(_) => return self.diagnostic(),
        };
        match resolved.kind {
            EntryKind::Tree => {
                ensure_tree_header(self.repository, resolved.object_id)
                    .map_err(map_snapshot_error)?;
                let tree = self
                    .repository
                    .find_tree(resolved.object_id)
                    .map_err(|_| DeltaError::ObjectInvalid)?;
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
            EntryKind::Commit | EntryKind::Link => self.diagnostic(),
        }
    }

    fn process_blob(
        &mut self,
        object_id: gix::ObjectId,
        path: &[String],
    ) -> Result<(), DeltaError> {
        let (policy, data) =
            match inspect_blob(self.repository, object_id, path, self.notes, self.limits)
                .map_err(map_snapshot_error)?
            {
                BlobInspection::Skip => return Ok(()),
                BlobInspection::Diagnostic(_) => return self.diagnostic(),
                BlobInspection::Admitted { policy, data } => (policy, data),
            };
        let text = matches!(policy, FilePolicy::Org | FilePolicy::Asset { text: true });
        self.bytes = self
            .bytes
            .checked_add(data.len() as u64)
            .filter(|bytes| *bytes <= self.limits.max_total_bytes)
            .ok_or(DeltaError::InputExhausted)?;
        let kind = match policy {
            FilePolicy::Org => DeltaFileKind::Org,
            FilePolicy::Asset { .. } => DeltaFileKind::Asset,
            FilePolicy::Skip | FilePolicy::EncryptedOrg | FilePolicy::Attributes => {
                return Err(DeltaError::ObjectInvalid);
            }
        };
        let path = path.join("/");
        if self
            .files
            .insert(
                path,
                InventoryEntry {
                    kind,
                    object_id,
                    text,
                },
            )
            .is_some()
        {
            return Err(DeltaError::ObjectInvalid);
        }
        Ok(())
    }

    fn diagnostic(&mut self) -> Result<(), DeltaError> {
        if self.diagnostics == MAX_SNAPSHOT_DIAGNOSTICS {
            return Err(DeltaError::DiagnosticsExhausted);
        }
        self.diagnostics += 1;
        Ok(())
    }
}

fn assemble_delta(
    request: &DeltaRequest,
    repository: &gix::Repository,
    mut previous: Inventory,
    mut current: Inventory,
    history: DeltaHistory,
    cancelled: &AtomicBool,
) -> Result<DeltaOutcome, DeltaError> {
    let mut modified = Vec::new();
    let common = previous
        .files
        .keys()
        .filter(|path| current.files.contains_key(*path))
        .cloned()
        .collect::<Vec<_>>();
    for path in common {
        let old = previous
            .files
            .remove(&path)
            .expect("collected previous path");
        let new = current.files.remove(&path).expect("collected current path");
        if old != new {
            modified.push(DeltaFile {
                path,
                kind: new.kind,
            });
        }
    }

    let raw_changes =
        modified.len() as u64 + previous.files.len() as u64 + current.files.len() as u64;
    if raw_changes > request.limits.max_changes {
        return Err(DeltaError::InputExhausted);
    }

    let mut renamed = exact_renames(&mut previous.files, &mut current.files);
    let comparisons = (previous.files.len() as u64).saturating_mul(current.files.len() as u64);
    let rename_detection = if comparisons <= request.limits.max_rename_comparisons {
        let (similar, complete) = similar_renames(
            repository,
            &mut previous.files,
            &mut current.files,
            cancelled,
        )?;
        renamed.extend(similar);
        if complete {
            RenameDetection::Complete
        } else {
            RenameDetection::Limited
        }
    } else {
        RenameDetection::Limited
    };

    let added = current
        .files
        .into_iter()
        .map(|(path, entry)| DeltaFile {
            path,
            kind: entry.kind,
        })
        .collect::<Vec<_>>();
    let deleted = previous
        .files
        .into_iter()
        .map(|(path, entry)| DeltaFile {
            path,
            kind: entry.kind,
        })
        .collect::<Vec<_>>();
    renamed.sort_by(|left, right| (&left.from, &left.to).cmp(&(&right.from, &right.to)));
    modified.sort_by(|left, right| left.path.cmp(&right.path));
    let disposition =
        if added.is_empty() && modified.is_empty() && deleted.is_empty() && renamed.is_empty() {
            DeltaDisposition::Unchanged
        } else {
            DeltaDisposition::Changed
        };
    Ok(DeltaOutcome {
        disposition,
        source: request.source().clone(),
        previous_revision: request.previous_revision.clone(),
        revision: request.revision.clone(),
        previous_notes_folder: request.previous_notes_folder().cloned(),
        notes_folder: request.notes_folder.clone(),
        history,
        rename_detection,
        added,
        modified,
        deleted,
        renamed,
    })
}

fn exact_renames(
    previous: &mut BTreeMap<String, InventoryEntry>,
    current: &mut BTreeMap<String, InventoryEntry>,
) -> Vec<RenameHint> {
    let mut destinations = BTreeMap::<(DeltaFileKind, gix::ObjectId), VecDeque<String>>::new();
    for (path, entry) in current.iter() {
        destinations
            .entry((entry.kind, entry.object_id))
            .or_default()
            .push_back(path.clone());
    }
    let mut pairs = Vec::new();
    for (path, entry) in previous.iter() {
        if let Some(destination) = destinations
            .get_mut(&(entry.kind, entry.object_id))
            .and_then(VecDeque::pop_front)
        {
            pairs.push((path.clone(), destination, entry.kind));
        }
    }
    for (from, to, _) in &pairs {
        previous.remove(from);
        current.remove(to);
    }
    pairs
        .into_iter()
        .map(|(from, to, kind)| RenameHint {
            from,
            to,
            kind,
            content_changed: false,
        })
        .collect()
}

fn similar_renames(
    repository: &gix::Repository,
    previous: &mut BTreeMap<String, InventoryEntry>,
    current: &mut BTreeMap<String, InventoryEntry>,
    cancelled: &AtomicBool,
) -> Result<(Vec<RenameHint>, bool), DeltaError> {
    let mut token_budget = MAX_DELTA_FINGERPRINT_TOKENS;
    let Some(old_fingerprints) =
        text_fingerprints(repository, previous, cancelled, &mut token_budget)?
    else {
        return Ok((Vec::new(), false));
    };
    let Some(new_fingerprints) =
        text_fingerprints(repository, current, cancelled, &mut token_budget)?
    else {
        return Ok((Vec::new(), false));
    };
    let mut candidates = Vec::new();
    let mut similarity_steps = 0_u64;
    for (from, old) in previous.iter() {
        let Some(old_fingerprint) = old_fingerprints.get(from) else {
            continue;
        };
        for (to, new) in current.iter() {
            if old.kind != new.kind {
                continue;
            }
            let Some(new_fingerprint) = new_fingerprints.get(to) else {
                continue;
            };
            similarity_steps = similarity_steps.saturating_add(old_fingerprint.tokens.len() as u64);
            if similarity_steps > MAX_DELTA_SIMILARITY_STEPS {
                return Ok((Vec::new(), false));
            }
            let similarity = old_fingerprint.similarity(new_fingerprint);
            if similarity >= RENAME_SIMILARITY_PERCENT {
                candidates.push((Reverse(similarity), from.clone(), to.clone(), old.kind));
            }
        }
    }
    candidates.sort();
    let mut used_old = BTreeSet::new();
    let mut used_new = BTreeSet::new();
    let mut renames = Vec::new();
    for (_, from, to, kind) in candidates {
        if used_old.contains(&from) || used_new.contains(&to) {
            continue;
        }
        used_old.insert(from.clone());
        used_new.insert(to.clone());
        previous.remove(&from);
        current.remove(&to);
        renames.push(RenameHint {
            from,
            to,
            kind,
            content_changed: true,
        });
    }
    Ok((renames, true))
}

#[derive(Default)]
struct TextFingerprint {
    tokens: BTreeMap<(gix::ObjectId, u64), u64>,
    bytes: u64,
}

impl TextFingerprint {
    fn from_bytes(bytes: &[u8], token_budget: &mut u64) -> Result<Option<Self>, DeltaError> {
        let mut fingerprint = Self::default();
        for token in bytes
            .split(u8::is_ascii_whitespace)
            .filter(|token| !token.is_empty())
        {
            let Some(remaining) = token_budget.checked_sub(1) else {
                return Ok(None);
            };
            *token_budget = remaining;
            let mut hasher = gix::hash::hasher(gix::hash::Kind::Sha1);
            hasher.update(token);
            let digest = hasher
                .try_finalize()
                .map_err(|_| DeltaError::ObjectInvalid)?;
            *fingerprint
                .tokens
                .entry((digest, token.len() as u64))
                .or_default() += 1;
            fingerprint.bytes += token.len() as u64;
        }
        Ok(Some(fingerprint))
    }

    fn similarity(&self, other: &Self) -> u16 {
        let denominator = self.bytes + other.bytes;
        if denominator == 0 {
            return 0;
        }
        let common = self
            .tokens
            .iter()
            .map(|(token, count)| {
                let other_count = other.tokens.get(token).copied().unwrap_or(0);
                count.min(&other_count) * token.1
            })
            .sum::<u64>();
        ((common.saturating_mul(200) / denominator).min(100)) as u16
    }
}

fn text_fingerprints(
    repository: &gix::Repository,
    entries: &BTreeMap<String, InventoryEntry>,
    cancelled: &AtomicBool,
    token_budget: &mut u64,
) -> Result<Option<BTreeMap<String, TextFingerprint>>, DeltaError> {
    let mut fingerprints = BTreeMap::new();
    for (path, entry) in entries {
        if !entry.text {
            continue;
        }
        delta_cancelled(cancelled)?;
        let blob = repository
            .find_blob(entry.object_id)
            .map_err(|_| DeltaError::ObjectInvalid)?;
        let Some(fingerprint) = TextFingerprint::from_bytes(&blob.data, token_budget)? else {
            return Ok(None);
        };
        fingerprints.insert(path.clone(), fingerprint);
    }
    Ok(Some(fingerprints))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reachability {
    Found,
    Absent,
    Incomplete,
}

fn classify_history(
    repository: &gix::Repository,
    previous: gix::ObjectId,
    current: gix::ObjectId,
    limit: u64,
    cancelled: &AtomicBool,
) -> Result<DeltaHistory, DeltaError> {
    if previous == current {
        return Ok(DeltaHistory::SameRevision);
    }
    let forward = reaches(repository, current, previous, limit, cancelled)?;
    if forward == Reachability::Found {
        return Ok(DeltaHistory::FastForward);
    }
    let backward = reaches(repository, previous, current, limit, cancelled)?;
    if backward == Reachability::Found {
        return Ok(DeltaHistory::Rewind);
    }
    if forward == Reachability::Absent && backward == Reachability::Absent {
        Ok(DeltaHistory::Diverged)
    } else {
        Ok(DeltaHistory::Incomplete)
    }
}

fn reaches(
    repository: &gix::Repository,
    start: gix::ObjectId,
    target: gix::ObjectId,
    limit: u64,
    cancelled: &AtomicBool,
) -> Result<Reachability, DeltaError> {
    let mut pending = VecDeque::from([start]);
    let mut seen = BTreeSet::new();
    let mut incomplete = false;
    while let Some(object_id) = pending.pop_front() {
        delta_cancelled(cancelled)?;
        if object_id == target {
            return Ok(Reachability::Found);
        }
        if !seen.insert(object_id) {
            continue;
        }
        if seen.len() as u64 > limit {
            return Ok(Reachability::Incomplete);
        }
        let header = match repository.find_header(object_id) {
            Ok(header) => header,
            Err(_) => {
                incomplete = true;
                continue;
            }
        };
        if header.kind() != gix::objs::Kind::Commit || header.size() > MAX_COMMIT_OBJECT_BYTES {
            return Err(DeltaError::ObjectInvalid);
        }
        let commit = repository
            .find_commit(object_id)
            .map_err(|_| DeltaError::ObjectInvalid)?;
        commit.decode().map_err(|_| DeltaError::ObjectInvalid)?;
        pending.extend(commit.parent_ids().map(|parent| parent.detach()));
    }
    Ok(if incomplete {
        Reachability::Incomplete
    } else {
        Reachability::Absent
    })
}

fn load_root(
    repository: &gix::Repository,
    revision: gix::ObjectId,
) -> Result<gix::Tree<'_>, DeltaError> {
    let header = repository
        .find_header(revision)
        .map_err(|_| DeltaError::RevisionUnavailable)?;
    if header.kind() != gix::objs::Kind::Commit || header.size() > MAX_COMMIT_OBJECT_BYTES {
        return Err(DeltaError::RevisionUnavailable);
    }
    let commit = repository
        .find_commit(revision)
        .map_err(|_| DeltaError::RevisionUnavailable)?;
    commit.decode().map_err(|_| DeltaError::ObjectInvalid)?;
    let tree_id = commit
        .tree_id()
        .map_err(|_| DeltaError::ObjectInvalid)?
        .detach();
    let tree_header = repository
        .find_header(tree_id)
        .map_err(|_| DeltaError::ObjectInvalid)?;
    if tree_header.kind() != gix::objs::Kind::Tree || tree_header.size() > MAX_TREE_OBJECT_BYTES {
        return Err(DeltaError::ObjectInvalid);
    }
    repository
        .find_tree(tree_id)
        .map_err(|_| DeltaError::ObjectInvalid)
}

fn parse_revision(revision: &str) -> Result<gix::ObjectId, DeltaError> {
    gix::ObjectId::from_hex(revision.as_bytes()).map_err(|_| DeltaError::RevisionRefused)
}

fn validate_delta_revision(revision: &str) -> Result<(), DeltaError> {
    validate_revision(revision).map_err(map_snapshot_error)
}

fn validate_delta_repository(repository: &Path) -> Result<(), DeltaError> {
    validate_storage_path(repository).map_err(|_| DeltaError::RepositoryInvalid)
}

fn delta_cancelled(cancelled: &AtomicBool) -> Result<(), DeltaError> {
    snapshot_cancelled(cancelled).map_err(map_snapshot_error)
}

fn map_snapshot_error(error: SnapshotError) -> DeltaError {
    match error {
        SnapshotError::LimitsRefused => DeltaError::LimitsRefused,
        SnapshotError::RevisionRefused => DeltaError::RevisionRefused,
        SnapshotError::RevisionUnavailable => DeltaError::RevisionUnavailable,
        SnapshotError::NotesFolderUnavailable => DeltaError::NotesFolderUnavailable,
        SnapshotError::UnsafePath => DeltaError::UnsafePath,
        SnapshotError::ObjectInvalid => DeltaError::ObjectInvalid,
        SnapshotError::InputExhausted => DeltaError::InputExhausted,
        SnapshotError::DiagnosticsExhausted => DeltaError::DiagnosticsExhausted,
        SnapshotError::Cancelled => DeltaError::Cancelled,
        SnapshotError::DestinationRefused
        | SnapshotError::DestinationOccupied
        | SnapshotError::StorageBoundary
        | SnapshotError::OwnershipMismatch
        | SnapshotError::StorageFailed
        | SnapshotError::RepositoryInvalid => DeltaError::RepositoryInvalid,
    }
}

#[cfg(test)]
#[path = "delta_tests.rs"]
mod tests;
