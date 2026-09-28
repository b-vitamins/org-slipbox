//! Startup recovery and bounded generation reclamation.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use slipbox_core::GenerationId;

use super::*;

const MAX_RECOVERY_DIRECTORY_ENTRIES: usize = 4_096;
const MAX_RECOVERY_TREE_ENTRIES: usize = 1_000_000;

/// The caller's explicit free-space floor. Recovery always reclaims abandoned
/// artifacts first, then reports pressure against this value without making a
/// readable generation unavailable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecoveryPolicy {
    pub minimum_free_bytes: u64,
}

impl RecoveryPolicy {
    #[must_use]
    pub const fn new(minimum_free_bytes: u64) -> Self {
        Self { minimum_free_bytes }
    }
}

impl Default for RecoveryPolicy {
    fn default() -> Self {
        Self::new(0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RecoveryDisposition {
    NoReadyGeneration,
    ReadyUnchanged,
    PublicationCompleted,
    RetainedRestored,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RecoveryNoticeKind {
    ActiveGenerationCorrupt,
    RetainedGenerationCorrupt,
    InterruptedPublication,
    CandidateGenerationCorrupt,
    IndexRebuildRequired,
    ConflictingPublications,
    StoragePressure,
    CleanupDeferred,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RecoveryAction {
    RetryImport,
    RebuildIndex,
    RefetchSource,
    FreeStorage,
    ReleaseReaders,
}

/// One closed, content-free diagnostic. Optional byte counts are present only
/// for storage pressure; no filesystem path enters this contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecoveryNotice {
    pub kind: RecoveryNoticeKind,
    pub action: RecoveryAction,
    pub generation: Option<GenerationId>,
    pub available_bytes: Option<u64>,
    pub required_bytes: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct RecoveryOutcome {
    pub disposition: RecoveryDisposition,
    pub ready: Option<GenerationLease>,
    pub reclaimed_generations: u64,
    pub reclaimed_candidates: u64,
    pub deferred_leased_generations: u64,
    pub reclaimed_bytes: u64,
    pub available_bytes: Option<u64>,
    pub notices: Vec<RecoveryNotice>,
}

#[derive(Debug, Clone)]
struct InspectedGeneration {
    path: PathBuf,
    manifest: GenerationManifest,
    record: GenerationRecord,
    staged: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InvalidReason {
    Unavailable,
    RebuildRequired,
}

#[derive(Debug)]
struct InvalidGeneration {
    generation: Option<GenerationId>,
    reason: InvalidReason,
    staged: bool,
    has_manifest: bool,
}

#[derive(Debug, Default)]
struct GenerationScan {
    regular: BTreeMap<GenerationId, InspectedGeneration>,
    staged: Vec<InspectedGeneration>,
    invalid: Vec<InvalidGeneration>,
}

#[derive(Debug)]
enum PointerState {
    Missing,
    Corrupt,
    Manifest(Box<GenerationManifest>),
}

impl GenerationStore {
    /// Recover a source store before refresh work starts. The operation takes
    /// the same crash-released filesystem lock as publication, validates every
    /// generation before activation, and serializes reclamation with lease
    /// acquisition across independently opened store handles and processes.
    pub fn recover(&self, policy: RecoveryPolicy) -> Result<RecoveryOutcome, PublicationError> {
        let _publication = PublicationLock::acquire(&self.root)?;
        let _recovery = RecoveryLock::acquire_exclusive(&self.root)?;
        let mut notices = Vec::new();
        let mut scan = self.scan_generations()?;
        let active = self.read_pointer(ACTIVE_GENERATION_FILE);
        let mut retained = self.read_pointer(RETAINED_GENERATION_FILE);
        let started_without_trusted_active = !matches!(active, PointerState::Manifest(_));
        let original_active = pointer_generation(&active).cloned();

        let active_record = pointer_record(&active, &scan.regular)
            .map(|(manifest, record)| (manifest.clone(), record.clone()));
        let mut retained_record = pointer_record(&retained, &scan.regular)
            .map(|(manifest, record)| (manifest.clone(), record.clone()));
        if retained_record.is_none()
            && let Some(predecessor) = active_record
                .as_ref()
                .and_then(|(manifest, _)| manifest.expected_active.as_ref())
                .and_then(|generation| scan.regular.get(generation))
        {
            self.replace_manifest(RETAINED_GENERATION_FILE, &predecessor.manifest)?;
            retained = PointerState::Manifest(Box::new(predecessor.manifest.clone()));
            retained_record = Some((predecessor.manifest.clone(), predecessor.record.clone()));
        }
        let retained_problem = retained_problem(&active_record, &retained, &retained_record);
        let mut disposition = RecoveryDisposition::ReadyUnchanged;
        let mut ready = active_record
            .as_ref()
            .map(|(_, record)| lease_record(record.clone()))
            .transpose()?;

        let active_needs_recovery =
            !matches!(active, PointerState::Manifest(_)) || active_record.is_none();
        let may_complete_publication = matches!(active, PointerState::Manifest(_))
            || (matches!(active, PointerState::Missing) && retained_record.is_none());

        if may_complete_publication {
            let expected_active = pointer_generation(&active);
            let pending = select_pending(&scan, expected_active, &retained, &mut notices);
            match pending.as_slice() {
                [candidate] => {
                    let candidate = self.promote_if_staged(candidate)?;
                    let candidate_lease = lease_record(candidate.record.clone())?;
                    if active_record.is_some() {
                        self.activate(&candidate.manifest)?;
                    } else {
                        self.activate_recovered(&candidate.manifest)?;
                    }
                    scan.regular.insert(
                        candidate.manifest.binding.generation.clone(),
                        candidate.clone(),
                    );
                    ready = Some(candidate_lease);
                    disposition = RecoveryDisposition::PublicationCompleted;
                }
                [] => {}
                _ => {
                    notices.push(notice(
                        RecoveryNoticeKind::ConflictingPublications,
                        RecoveryAction::RetryImport,
                        None,
                    ));
                }
            }
        }

        if ready.is_none() && disposition != RecoveryDisposition::PublicationCompleted {
            report_active_failure(&active, &scan, &mut notices);
            if let Some((manifest, record)) = retained_record {
                let retained_lease = lease_record(record)?;
                self.activate_recovered(&manifest)?;
                ready = Some(retained_lease);
                disposition = RecoveryDisposition::RetainedRestored;
            } else {
                report_retained_failure(&retained, &scan, &mut notices);
                disposition = RecoveryDisposition::NoReadyGeneration;
            }
        } else if active_needs_recovery && disposition == RecoveryDisposition::PublicationCompleted
        {
            report_active_failure(&active, &scan, &mut notices);
        }
        if ready.is_some()
            && let Some(generation) = retained_problem.as_ref()
        {
            notices.push(notice(
                RecoveryNoticeKind::RetainedGenerationCorrupt,
                RecoveryAction::RetryImport,
                generation.clone(),
            ));
        }

        report_invalid_candidates(&scan, &active, &retained, &mut notices);

        let active_after = self.read_pointer(ACTIVE_GENERATION_FILE);
        let retained_after = self.read_pointer(RETAINED_GENERATION_FILE);
        let mut preserve = BTreeSet::new();
        if let Some(generation) = pointer_generation(&active_after) {
            preserve.insert(generation.clone());
        }
        if let Some(generation) = pointer_generation(&retained_after) {
            preserve.insert(generation.clone());
        }
        if let Some(generation) = original_active {
            preserve.insert(generation);
        }
        if started_without_trusted_active || ready.is_none() || retained_problem.is_some() {
            preserve.extend(scan.regular.keys().cloned());
        }

        let cleanup = self.reclaim(&preserve, &mut notices);
        let available_bytes = fs2::available_space(&self.root).ok();
        if available_bytes.is_none() {
            notices.push(notice(
                RecoveryNoticeKind::CleanupDeferred,
                RecoveryAction::FreeStorage,
                ready
                    .as_ref()
                    .map(|lease| lease.binding().generation.clone()),
            ));
        } else if available_bytes.is_some_and(|available| available < policy.minimum_free_bytes) {
            notices.push(RecoveryNotice {
                kind: RecoveryNoticeKind::StoragePressure,
                action: RecoveryAction::FreeStorage,
                generation: ready
                    .as_ref()
                    .map(|lease| lease.binding().generation.clone()),
                available_bytes,
                required_bytes: Some(policy.minimum_free_bytes),
            });
        }

        Ok(RecoveryOutcome {
            disposition,
            ready,
            reclaimed_generations: cleanup.generations,
            reclaimed_candidates: cleanup.candidates,
            deferred_leased_generations: cleanup.deferred,
            reclaimed_bytes: cleanup.bytes,
            available_bytes,
            notices,
        })
    }

    fn read_pointer(&self, name: &str) -> PointerState {
        let path = self.root.join(name);
        match path_is_present(&path) {
            Ok(false) => PointerState::Missing,
            Ok(true) => match read_json::<GenerationManifest>(&path).and_then(|manifest| {
                validate_generation_manifest(&manifest, &self.source)?;
                Ok(manifest)
            }) {
                Ok(manifest) => PointerState::Manifest(Box::new(manifest)),
                Err(_) => PointerState::Corrupt,
            },
            Err(_) => PointerState::Corrupt,
        }
    }

    fn scan_generations(&self) -> Result<GenerationScan, PublicationError> {
        let generations = self.root.join(GENERATIONS_DIRECTORY);
        let mut scan = GenerationScan::default();
        for (index, entry) in fs::read_dir(&generations)
            .map_err(|_| PublicationError::StoreUnavailable)?
            .enumerate()
        {
            if index >= MAX_RECOVERY_DIRECTORY_ENTRIES {
                return Err(PublicationError::StoreUnavailable);
            }
            let entry = entry.map_err(|_| PublicationError::StoreUnavailable)?;
            let path = entry.path();
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                scan.invalid.push(InvalidGeneration {
                    generation: None,
                    reason: InvalidReason::Unavailable,
                    staged: false,
                    has_manifest: false,
                });
                continue;
            };
            let staged = name.starts_with(GENERATION_STAGE_PREFIX);
            let generation = if staged {
                None
            } else {
                GenerationId::parse(&name)
                    .ok()
                    .filter(generation_id_is_admitted)
            };
            if !staged && generation.is_none() {
                scan.invalid.push(InvalidGeneration {
                    generation: None,
                    reason: InvalidReason::Unavailable,
                    staged,
                    has_manifest: false,
                });
                continue;
            }
            let expected = generation.as_ref();
            match self.inspect_generation_directory(&path, expected) {
                Ok((manifest, record)) => {
                    let inspected = InspectedGeneration {
                        path,
                        manifest,
                        record,
                        staged,
                    };
                    if staged {
                        scan.staged.push(inspected);
                    } else if let Some(generation) = generation {
                        scan.regular.insert(generation, inspected);
                    }
                }
                Err(error) => {
                    let has_manifest =
                        path_is_present(&path.join(GENERATION_MANIFEST_FILE)).unwrap_or(false);
                    scan.invalid.push(InvalidGeneration {
                        generation,
                        reason: match error {
                            GenerationInspectionError::Unavailable => InvalidReason::Unavailable,
                            GenerationInspectionError::RebuildRequired => {
                                InvalidReason::RebuildRequired
                            }
                        },
                        staged,
                        has_manifest,
                    });
                }
            }
        }
        Ok(scan)
    }

    fn promote_if_staged(
        &self,
        candidate: &InspectedGeneration,
    ) -> Result<InspectedGeneration, PublicationError> {
        if !candidate.staged {
            return Ok(candidate.clone());
        }
        let destination = self.generation_directory(&candidate.manifest.binding.generation);
        if path_is_present(&destination)? {
            return Err(PublicationError::GenerationOccupied);
        }
        fs::rename(&candidate.path, &destination).map_err(|_| PublicationError::StorageFailed)?;
        sync_directory(&self.root.join(GENERATIONS_DIRECTORY))?;
        let (manifest, record) = self
            .inspect_generation_directory(
                &destination,
                Some(&candidate.manifest.binding.generation),
            )
            .map_err(|_| PublicationError::StoreUnavailable)?;
        if manifest != candidate.manifest {
            return Err(PublicationError::StoreUnavailable);
        }
        Ok(InspectedGeneration {
            path: destination,
            manifest,
            record,
            staged: false,
        })
    }

    fn reclaim(
        &self,
        preserve: &BTreeSet<GenerationId>,
        notices: &mut Vec<RecoveryNotice>,
    ) -> CleanupCounts {
        let mut counts = CleanupCounts::default();
        let generations = self.root.join(GENERATIONS_DIRECTORY);
        let generation_entries = match bounded_entries(&generations) {
            Ok(entries) => entries,
            Err(_) => {
                notices.push(notice(
                    RecoveryNoticeKind::CleanupDeferred,
                    RecoveryAction::FreeStorage,
                    None,
                ));
                Vec::new()
            }
        };
        for entry in generation_entries {
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_str();
            if name.is_some_and(|name| name.starts_with(GENERATION_STAGE_PREFIX)) {
                reclaim_entry(&path, false, &mut counts, notices);
                continue;
            }
            let generation = name
                .and_then(|name| GenerationId::parse(name).ok())
                .filter(generation_id_is_admitted);
            if let Some(generation) = generation.as_ref() {
                if preserve.contains(generation) {
                    continue;
                }
                match generation_is_leased(&path) {
                    Ok(true) => {
                        counts.deferred += 1;
                        notices.push(notice(
                            RecoveryNoticeKind::CleanupDeferred,
                            RecoveryAction::ReleaseReaders,
                            Some(generation.clone()),
                        ));
                        continue;
                    }
                    Ok(false) => {}
                    Err(()) => {
                        notices.push(notice(
                            RecoveryNoticeKind::CleanupDeferred,
                            RecoveryAction::FreeStorage,
                            Some(generation.clone()),
                        ));
                        continue;
                    }
                }
            }
            reclaim_entry(&path, true, &mut counts, notices);
        }

        let candidates = self.candidates_root();
        let candidate_entries = match bounded_entries(&candidates) {
            Ok(entries) => entries,
            Err(_) => {
                notices.push(notice(
                    RecoveryNoticeKind::CleanupDeferred,
                    RecoveryAction::FreeStorage,
                    None,
                ));
                Vec::new()
            }
        };
        if !candidate_entries.is_empty()
            && !notices
                .iter()
                .any(|notice| notice.kind == RecoveryNoticeKind::InterruptedPublication)
        {
            notices.push(notice(
                RecoveryNoticeKind::InterruptedPublication,
                RecoveryAction::RetryImport,
                None,
            ));
        }
        for entry in candidate_entries {
            reclaim_entry(&entry.path(), false, &mut counts, notices);
        }
        if sync_directory(&generations).is_err() || sync_directory(&candidates).is_err() {
            notices.push(notice(
                RecoveryNoticeKind::CleanupDeferred,
                RecoveryAction::FreeStorage,
                None,
            ));
        }
        counts
    }
}

fn pointer_generation(pointer: &PointerState) -> Option<&GenerationId> {
    match pointer {
        PointerState::Manifest(manifest) => Some(&manifest.binding.generation),
        PointerState::Missing | PointerState::Corrupt => None,
    }
}

fn pointer_record<'a>(
    pointer: &'a PointerState,
    regular: &'a BTreeMap<GenerationId, InspectedGeneration>,
) -> Option<(&'a GenerationManifest, &'a GenerationRecord)> {
    let PointerState::Manifest(pointer) = pointer else {
        return None;
    };
    let inspected = regular.get(&pointer.binding.generation)?;
    (inspected.manifest == **pointer).then_some((&inspected.manifest, &inspected.record))
}

fn retained_problem(
    active: &Option<(GenerationManifest, GenerationRecord)>,
    retained: &PointerState,
    retained_record: &Option<(GenerationManifest, GenerationRecord)>,
) -> Option<Option<GenerationId>> {
    if retained_record.is_some() {
        return None;
    }
    match retained {
        PointerState::Corrupt => Some(None),
        PointerState::Manifest(manifest) => Some(Some(manifest.binding.generation.clone())),
        PointerState::Missing => active
            .as_ref()
            .and_then(|(manifest, _)| manifest.expected_active.clone())
            .map(Some),
    }
}

fn select_pending(
    scan: &GenerationScan,
    expected_active: Option<&GenerationId>,
    retained: &PointerState,
    notices: &mut Vec<RecoveryNotice>,
) -> Vec<InspectedGeneration> {
    let retained = pointer_generation(retained);
    let mut selected = BTreeMap::<GenerationId, InspectedGeneration>::new();
    let mut conflicting = false;
    for candidate in scan.regular.values().chain(scan.staged.iter()) {
        let generation = &candidate.manifest.binding.generation;
        if Some(generation) == expected_active
            || Some(generation) == retained
            || candidate.manifest.expected_active.as_ref() != expected_active
        {
            continue;
        }
        match selected.get(generation) {
            Some(existing) if existing.manifest != candidate.manifest => {
                conflicting = true;
                notices.push(notice(
                    RecoveryNoticeKind::ConflictingPublications,
                    RecoveryAction::RetryImport,
                    Some(generation.clone()),
                ));
            }
            Some(existing) if !existing.staged => {}
            _ => {
                selected.insert(generation.clone(), candidate.clone());
            }
        }
    }
    if conflicting {
        Vec::new()
    } else {
        selected.into_values().collect()
    }
}

fn report_active_failure(
    active: &PointerState,
    scan: &GenerationScan,
    notices: &mut Vec<RecoveryNotice>,
) {
    match active {
        PointerState::Missing => {}
        PointerState::Corrupt => notices.push(notice(
            RecoveryNoticeKind::ActiveGenerationCorrupt,
            RecoveryAction::RefetchSource,
            None,
        )),
        PointerState::Manifest(manifest) => {
            let generation = manifest.binding.generation.clone();
            let kind = invalid_reason(scan, &generation)
                .map(notice_kind_for_invalid)
                .unwrap_or(RecoveryNoticeKind::ActiveGenerationCorrupt);
            notices.push(notice(
                kind,
                if kind == RecoveryNoticeKind::IndexRebuildRequired {
                    RecoveryAction::RebuildIndex
                } else {
                    RecoveryAction::RefetchSource
                },
                Some(generation),
            ));
        }
    }
}

fn report_retained_failure(
    retained: &PointerState,
    scan: &GenerationScan,
    notices: &mut Vec<RecoveryNotice>,
) {
    match retained {
        PointerState::Missing => {}
        PointerState::Corrupt => notices.push(notice(
            RecoveryNoticeKind::RetainedGenerationCorrupt,
            RecoveryAction::RefetchSource,
            None,
        )),
        PointerState::Manifest(manifest) if pointer_record(retained, &scan.regular).is_none() => {
            notices.push(notice(
                RecoveryNoticeKind::RetainedGenerationCorrupt,
                RecoveryAction::RefetchSource,
                Some(manifest.binding.generation.clone()),
            ));
        }
        PointerState::Manifest(_) => {}
    }
}

fn report_invalid_candidates(
    scan: &GenerationScan,
    active: &PointerState,
    retained: &PointerState,
    notices: &mut Vec<RecoveryNotice>,
) {
    let active = pointer_generation(active);
    let retained = pointer_generation(retained);
    for invalid in &scan.invalid {
        if invalid
            .generation
            .as_ref()
            .is_some_and(|generation| Some(generation) == active || Some(generation) == retained)
        {
            continue;
        }
        let kind = if invalid.reason == InvalidReason::RebuildRequired {
            RecoveryNoticeKind::IndexRebuildRequired
        } else if invalid.staged && !invalid.has_manifest {
            RecoveryNoticeKind::InterruptedPublication
        } else {
            RecoveryNoticeKind::CandidateGenerationCorrupt
        };
        notices.push(notice(
            kind,
            if kind == RecoveryNoticeKind::IndexRebuildRequired {
                RecoveryAction::RebuildIndex
            } else {
                RecoveryAction::RetryImport
            },
            invalid.generation.clone(),
        ));
    }
}

fn invalid_reason(scan: &GenerationScan, generation: &GenerationId) -> Option<InvalidReason> {
    scan.invalid
        .iter()
        .find(|invalid| invalid.generation.as_ref() == Some(generation))
        .map(|invalid| invalid.reason)
}

fn notice_kind_for_invalid(reason: InvalidReason) -> RecoveryNoticeKind {
    match reason {
        InvalidReason::Unavailable => RecoveryNoticeKind::ActiveGenerationCorrupt,
        InvalidReason::RebuildRequired => RecoveryNoticeKind::IndexRebuildRequired,
    }
}

fn notice(
    kind: RecoveryNoticeKind,
    action: RecoveryAction,
    generation: Option<GenerationId>,
) -> RecoveryNotice {
    RecoveryNotice {
        kind,
        action,
        generation,
        available_bytes: None,
        required_bytes: None,
    }
}

fn generation_is_leased(directory: &Path) -> Result<bool, ()> {
    let path = directory.join(GENERATION_READER_LOCK_FILE);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(_) => return Err(()),
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Ok(false);
    }
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|_| ())?;
    match fs2::FileExt::try_lock_exclusive(&file) {
        Ok(()) => {
            fs2::FileExt::unlock(&file).map_err(|_| ())?;
            Ok(false)
        }
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(true),
        Err(_) => Err(()),
    }
}

#[derive(Debug, Default)]
struct CleanupCounts {
    generations: u64,
    candidates: u64,
    deferred: u64,
    bytes: u64,
}

fn bounded_entries(path: &Path) -> Result<Vec<fs::DirEntry>, PublicationError> {
    let mut entries = Vec::new();
    for (index, entry) in fs::read_dir(path)
        .map_err(|_| PublicationError::StoreUnavailable)?
        .enumerate()
    {
        if index >= MAX_RECOVERY_DIRECTORY_ENTRIES {
            return Err(PublicationError::StoreUnavailable);
        }
        entries.push(entry.map_err(|_| PublicationError::StoreUnavailable)?);
    }
    Ok(entries)
}

fn reclaim_entry(
    path: &Path,
    generation: bool,
    counts: &mut CleanupCounts,
    notices: &mut Vec<RecoveryNotice>,
) {
    let mut visited = 0_usize;
    match remove_tree(path, &mut visited) {
        Ok(bytes) => {
            counts.bytes = counts.bytes.saturating_add(bytes);
            if generation {
                counts.generations += 1;
            } else {
                counts.candidates += 1;
            }
        }
        Err(()) => {
            notices.push(notice(
                RecoveryNoticeKind::CleanupDeferred,
                RecoveryAction::FreeStorage,
                None,
            ));
        }
    }
}

fn remove_tree(path: &Path, visited: &mut usize) -> Result<u64, ()> {
    *visited = visited.checked_add(1).ok_or(())?;
    if *visited > MAX_RECOVERY_TREE_ENTRIES {
        return Err(());
    }
    let metadata = fs::symlink_metadata(path).map_err(|_| ())?;
    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        let mut bytes = 0_u64;
        for entry in fs::read_dir(path).map_err(|_| ())? {
            bytes = bytes.saturating_add(remove_tree(&entry.map_err(|_| ())?.path(), visited)?);
        }
        fs::remove_dir(path).map_err(|_| ())?;
        Ok(bytes)
    } else {
        let bytes = metadata.len();
        fs::remove_file(path).map_err(|_| ())?;
        Ok(bytes)
    }
}
