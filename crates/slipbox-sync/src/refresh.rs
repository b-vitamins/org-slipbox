//! Source-scoped refresh coordination and repository-to-generation execution.

use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use slipbox_core::{GenerationBinding, GenerationId, SourceId, SourceRecord, SourceVisibility};
use slipbox_git::{
    AccessToken, DeltaError, DeltaRequest, FetchRequest, GitError, GitProgress, ProgressStage,
    SnapshotError, SnapshotProgress, SnapshotRequest, derive_delta, materialize, synchronize,
};
use thiserror::Error;

use crate::{
    GenerationStore, PublicationError, PublicationStage, PublishGenerationRequest, RecoveryPolicy,
    StageIndexError, StageIndexProgress, StageIndexRequest, stage_index,
};

pub const MAX_CONCURRENT_REFRESHES: usize = 8;
pub const MAX_REFRESH_ATTEMPTS: u8 = 3;
const MAX_REFRESH_PATH_BYTES: usize = 4_096;

#[derive(Debug, Clone)]
pub struct RefreshContext {
    operation: i64,
    attempt: u8,
    source: SourceRecord,
    repository: PathBuf,
    store: PathBuf,
}

impl RefreshContext {
    pub fn new(
        operation: i64,
        attempt: u8,
        source: SourceRecord,
        repository: PathBuf,
        store: PathBuf,
    ) -> Result<Self, RefreshFailure> {
        if operation <= 0
            || attempt >= MAX_REFRESH_ATTEMPTS
            || !admitted_path(&repository)
            || !admitted_path(&store)
            || repository == store
            || repository.parent() != store.parent()
        {
            return Err(RefreshFailure::never(RefreshFailureReason::RequestRefused));
        }
        Ok(Self {
            operation,
            attempt,
            source,
            repository,
            store,
        })
    }

    #[must_use]
    pub fn operation(&self) -> i64 {
        self.operation
    }

    #[must_use]
    pub fn attempt(&self) -> u8 {
        self.attempt
    }

    #[must_use]
    pub fn source(&self) -> &SourceRecord {
        &self.source
    }

    #[must_use]
    pub fn repository(&self) -> &Path {
        &self.repository
    }

    #[must_use]
    pub fn store(&self) -> &Path {
        &self.store
    }

    fn same_import(&self, other: &Self) -> bool {
        self.source.has_same_import_configuration(&other.source)
            && self.repository == other.repository
            && self.store == other.store
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshDisposition {
    Started,
    Coalesced,
    Refused,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoordinatedRefresh {
    pub disposition: RefreshDisposition,
    pub status: RefreshStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefreshProgress {
    pub completed: u64,
    pub total: u64,
    pub unit: RefreshProgressUnit,
}

impl RefreshProgress {
    #[must_use]
    pub const fn none() -> Self {
        Self {
            completed: 0,
            total: 0,
            unit: RefreshProgressUnit::None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshProgressUnit {
    None,
    Objects,
    Entries,
    Files,
    Steps,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("refresh failed: {reason}")]
pub struct RefreshFailure {
    pub reason: RefreshFailureReason,
    pub retry: RefreshRetry,
}

impl RefreshFailure {
    #[must_use]
    pub const fn never(reason: RefreshFailureReason) -> Self {
        Self {
            reason,
            retry: RefreshRetry::Never,
        }
    }

    #[must_use]
    pub const fn with_retry(reason: RefreshFailureReason, retry: RefreshRetry) -> Self {
        Self { reason, retry }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum RefreshFailureReason {
    #[error("the refresh request is not admitted")]
    RequestRefused,
    #[error("the operation identity is already active")]
    OperationConflict,
    #[error("the active refresh limit was reached")]
    OperationsExhausted,
    #[error("authorization is required")]
    AuthorizationRequired,
    #[error("authorization was refused")]
    AuthorizationFailed,
    #[error("the repository transport failed")]
    TransportFailed,
    #[error("the repository is invalid")]
    RepositoryInvalid,
    #[error("the tracked branch is unavailable")]
    BranchUnavailable,
    #[error("the notes folder is unavailable")]
    NotesFolderUnavailable,
    #[error("the source input is unsafe")]
    UnsafeSource,
    #[error("the source input budget was exhausted")]
    InputExhausted,
    #[error("the index requires a rebuild")]
    RebuildRequired,
    #[error("private storage is unavailable")]
    StorageFailed,
    #[error("publication lost its active-generation precondition")]
    PublicationConflict,
    #[error("a newer source configuration superseded this refresh")]
    Superseded,
    #[error("the refresh was cancelled")]
    Cancelled,
    #[error("the refresh worker panicked")]
    Panicked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshRetry {
    Never,
    Reauthorize,
    Backoff { after_seconds: u64 },
    FreeStorage,
    Rebuild,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshCompletion {
    pub fetched_revision: String,
    pub ready_revision: String,
    pub ready_generation: GenerationId,
}

#[derive(Default)]
pub struct RefreshCoordinator {
    state: Mutex<CoordinatorState>,
    changed: Condvar,
}

#[derive(Default)]
struct CoordinatorState {
    active: BTreeMap<SourceId, Arc<Flight>>,
    operations: BTreeMap<i64, Arc<Flight>>,
    last: BTreeMap<SourceId, RefreshStatus>,
}

struct Flight {
    context: RefreshContext,
    cancelled: AtomicBool,
    superseded: AtomicBool,
    committed: AtomicBool,
    activation: Mutex<()>,
    lifecycle: Mutex<FlightLifecycle>,
    completed: Condvar,
}

struct FlightLifecycle {
    status: RefreshStatus,
    complete: bool,
}

pub struct RefreshControl {
    flight: Arc<Flight>,
}

impl RefreshCoordinator {
    /// Run at most one import per source. An equivalent overlapping request
    /// waits for and returns the same result. A changed configuration first
    /// supersedes the old flight, then starts only after it has stopped.
    pub fn coordinate(
        &self,
        context: RefreshContext,
        work: impl FnOnce(&RefreshControl) -> Result<RefreshCompletion, RefreshFailure>,
    ) -> CoordinatedRefresh {
        enum Admission {
            Lead(Arc<Flight>),
            Follow(Arc<Flight>),
            Refused(RefreshFailure),
        }

        let admission = loop {
            let mut state = lock(&self.state);
            if state.operations.contains_key(&context.operation) {
                break Admission::Refused(RefreshFailure::never(
                    RefreshFailureReason::OperationConflict,
                ));
            }
            if let Some(active) = state.active.get(context.source.id()).cloned() {
                if active.context.same_import(&context) {
                    state
                        .operations
                        .insert(context.operation, Arc::clone(&active));
                    break Admission::Follow(active);
                }
                drop(state);
                active.supersede();
                let mut state = lock(&self.state);
                while state
                    .active
                    .get(context.source.id())
                    .is_some_and(|current| Arc::ptr_eq(current, &active))
                {
                    state = wait(&self.changed, state);
                }
                continue;
            }
            if state.active.len() >= MAX_CONCURRENT_REFRESHES {
                break Admission::Refused(RefreshFailure::never(
                    RefreshFailureReason::OperationsExhausted,
                ));
            }
            let flight = Arc::new(Flight::new(context.clone()));
            state
                .active
                .insert(context.source.id().clone(), Arc::clone(&flight));
            state
                .operations
                .insert(context.operation, Arc::clone(&flight));
            break Admission::Lead(flight);
        };

        match admission {
            Admission::Refused(failure) => CoordinatedRefresh {
                disposition: RefreshDisposition::Refused,
                status: failed_status(&context, failure),
            },
            Admission::Follow(flight) => CoordinatedRefresh {
                disposition: RefreshDisposition::Coalesced,
                status: flight.wait_status_for(&context),
            },
            Admission::Lead(flight) => {
                let control = RefreshControl {
                    flight: Arc::clone(&flight),
                };
                match catch_unwind(AssertUnwindSafe(|| work(&control))) {
                    Ok(result) => {
                        let status = flight.finish(result);
                        self.retire(&context, &flight, &status);
                        CoordinatedRefresh {
                            disposition: RefreshDisposition::Started,
                            status,
                        }
                    }
                    Err(payload) => {
                        let status = flight
                            .finish(Err(RefreshFailure::never(RefreshFailureReason::Panicked)));
                        self.retire(&context, &flight, &status);
                        resume_unwind(payload)
                    }
                }
            }
        }
    }

    /// Cancel the source flight carrying this trigger. Cancellation linearizes
    /// with activation: once active-manifest replacement has begun, it is a
    /// successful refresh rather than a reported cancellation.
    #[must_use]
    pub fn cancel(&self, operation: i64) -> bool {
        let flight = {
            let state = lock(&self.state);
            state.operations.get(&operation).cloned()
        };
        flight.is_some_and(|flight| flight.cancel())
    }

    /// Return current or last terminal state without performing network,
    /// credential or filesystem work.
    #[must_use]
    pub fn status(&self, source: &SourceId) -> Option<RefreshStatus> {
        let state = lock(&self.state);
        state
            .active
            .get(source)
            .map(|flight| flight.status())
            .or_else(|| state.last.get(source).cloned())
    }

    #[cfg(test)]
    fn has_operation(&self, operation: i64) -> bool {
        lock(&self.state).operations.contains_key(&operation)
    }

    fn retire(&self, context: &RefreshContext, flight: &Arc<Flight>, status: &RefreshStatus) {
        let mut state = lock(&self.state);
        if state
            .active
            .get(context.source.id())
            .is_some_and(|current| Arc::ptr_eq(current, flight))
        {
            state.active.remove(context.source.id());
        }
        state
            .operations
            .retain(|_, active| !Arc::ptr_eq(active, flight));
        state
            .last
            .insert(context.source.id().clone(), status.clone());
        self.changed.notify_all();
    }
}

impl Flight {
    fn new(context: RefreshContext) -> Self {
        let status = RefreshStatus {
            source: context.source.clone(),
            operation: context.operation,
            state: RefreshState::Recovering,
            fetched_revision: None,
            ready_revision: None,
            ready_generation: None,
            progress: RefreshProgress::none(),
            failure: None,
        };
        Self {
            context,
            cancelled: AtomicBool::new(false),
            superseded: AtomicBool::new(false),
            committed: AtomicBool::new(false),
            activation: Mutex::new(()),
            lifecycle: Mutex::new(FlightLifecycle {
                status,
                complete: false,
            }),
            completed: Condvar::new(),
        }
    }

    fn status(&self) -> RefreshStatus {
        lock(&self.lifecycle).status.clone()
    }

    fn wait_status_for(&self, context: &RefreshContext) -> RefreshStatus {
        let mut lifecycle = lock(&self.lifecycle);
        while !lifecycle.complete {
            lifecycle = wait(&self.completed, lifecycle);
        }
        let mut status = lifecycle.status.clone();
        status.source = context.source.clone();
        status.operation = context.operation;
        status
    }

    fn finish(&self, result: Result<RefreshCompletion, RefreshFailure>) -> RefreshStatus {
        let mut lifecycle = lock(&self.lifecycle);
        match result {
            Ok(completion) => {
                lifecycle.status.state = RefreshState::Ready;
                lifecycle.status.fetched_revision = Some(completion.fetched_revision);
                lifecycle.status.ready_revision = Some(completion.ready_revision);
                lifecycle.status.ready_generation = Some(completion.ready_generation);
                lifecycle.status.failure = None;
            }
            Err(failure) => {
                lifecycle.status.state = if failure.reason == RefreshFailureReason::Cancelled
                    || failure.reason == RefreshFailureReason::Superseded
                {
                    RefreshState::Cancelled
                } else {
                    RefreshState::Failed
                };
                lifecycle.status.failure = Some(failure);
            }
        }
        lifecycle.complete = true;
        self.completed.notify_all();
        lifecycle.status.clone()
    }

    fn cancel(&self) -> bool {
        let _activation = lock(&self.activation);
        if self.committed.load(Ordering::Acquire) || lock(&self.lifecycle).complete {
            return false;
        }
        self.cancelled.store(true, Ordering::Release);
        true
    }

    fn supersede(&self) {
        let _activation = lock(&self.activation);
        if !self.committed.load(Ordering::Acquire) && !lock(&self.lifecycle).complete {
            self.superseded.store(true, Ordering::Release);
            self.cancelled.store(true, Ordering::Release);
        }
    }
}

impl RefreshControl {
    #[must_use]
    pub fn cancelled(&self) -> &AtomicBool {
        &self.flight.cancelled
    }

    pub fn report(
        &self,
        state: RefreshState,
        progress: RefreshProgress,
        fetched_revision: Option<&str>,
        ready_revision: Option<&str>,
        ready_generation: Option<&GenerationId>,
    ) -> Result<(), RefreshFailure> {
        self.check()?;
        let mut lifecycle = lock(&self.flight.lifecycle);
        lifecycle.status.state = state;
        lifecycle.status.progress = progress;
        if let Some(revision) = fetched_revision {
            lifecycle.status.fetched_revision = Some(revision.to_owned());
        }
        if let Some(revision) = ready_revision {
            lifecycle.status.ready_revision = Some(revision.to_owned());
        }
        if let Some(generation) = ready_generation {
            lifecycle.status.ready_generation = Some(generation.clone());
        }
        Ok(())
    }

    pub fn activate<T>(
        &self,
        publish: impl FnOnce(&AtomicBool) -> Result<T, RefreshFailure>,
    ) -> Result<T, RefreshFailure> {
        let _activation = lock(&self.flight.activation);
        self.check()?;
        let outcome = publish(&self.flight.cancelled);
        if outcome.is_ok() {
            self.flight.committed.store(true, Ordering::Release);
        }
        outcome
    }

    fn check(&self) -> Result<(), RefreshFailure> {
        if self.flight.superseded.load(Ordering::Acquire) {
            Err(RefreshFailure::never(RefreshFailureReason::Superseded))
        } else if self.flight.cancelled.load(Ordering::Acquire) {
            Err(RefreshFailure::never(RefreshFailureReason::Cancelled))
        } else {
            Ok(())
        }
    }
}

/// Execute the complete Rust-owned refresh path for one admitted source flight.
pub fn refresh_repository(
    context: &RefreshContext,
    credential: Option<AccessToken>,
    control: &RefreshControl,
) -> Result<RefreshCompletion, RefreshFailure> {
    control.report(
        RefreshState::Recovering,
        RefreshProgress::none(),
        None,
        None,
        None,
    )?;
    let store = GenerationStore::initialize(context.source.id().clone(), context.store.clone())
        .map_err(|error| map_publication(error, context.attempt))?;
    let recovered = store
        .recover(RecoveryPolicy::default())
        .map_err(|error| map_publication(error, context.attempt))?;
    let ready = recovered.ready;
    let ready_revision = ready.as_ref().map(|generation| generation.revision());
    let ready_generation = ready
        .as_ref()
        .map(|generation| &generation.binding().generation);
    control.report(
        RefreshState::Fetching,
        RefreshProgress::none(),
        None,
        ready_revision,
        ready_generation,
    )?;

    match (context.source.visibility(), credential.is_some()) {
        (SourceVisibility::Private, false) => {
            return Err(RefreshFailure::with_retry(
                RefreshFailureReason::AuthorizationRequired,
                RefreshRetry::Reauthorize,
            ));
        }
        (SourceVisibility::Public, true) => {
            return Err(RefreshFailure::never(
                RefreshFailureReason::AuthorizationFailed,
            ));
        }
        _ => {}
    }
    let fetch = FetchRequest::new(
        context.source.remote().clone(),
        context.source.branch().clone(),
        context.repository.clone(),
    )
    .map_err(|error| map_git(error, context.attempt))?;
    let fetched = synchronize(&fetch, credential, control.cancelled(), |progress| {
        report_git(control, progress, ready_revision, ready_generation);
    })
    .map_err(|error| map_git(error, context.attempt))?;
    control.report(
        RefreshState::Comparing,
        RefreshProgress::none(),
        Some(&fetched.revision),
        ready_revision,
        ready_generation,
    )?;

    if let Some(active) = ready.as_ref()
        && active.revision() == fetched.revision
        && active
            .source_record()
            .has_same_import_configuration(&context.source)
    {
        return Ok(RefreshCompletion {
            fetched_revision: fetched.revision.clone(),
            ready_revision: fetched.revision,
            ready_generation: active.binding().generation.clone(),
        });
    }

    let mut delta = derive_refresh_delta(context, ready.as_ref(), &fetched.revision, control)
        .map_err(|error| map_delta(error, context.attempt))?;
    let stem = format!("refresh-{}", context.operation);
    let snapshot_path = store.candidates_root().join(format!("{stem}-source"));
    let index_path = store.candidates_root().join(format!("{stem}-index"));
    let snapshot_request = SnapshotRequest::new(
        context.source.id().clone(),
        context.repository.clone(),
        &fetched.revision,
        context.source.notes_folder().clone(),
        snapshot_path,
    )
    .map_err(|error| map_snapshot(error, context.attempt))?;
    let snapshot = materialize(&snapshot_request, control.cancelled(), |progress| {
        report_snapshot(control, progress, &fetched.revision, ready.as_ref())
    })
    .map_err(|error| map_snapshot(error, context.attempt))?;

    let base = ready
        .as_ref()
        .map(|generation| generation.index_directory().to_owned());
    let staged = match stage_index(
        &StageIndexRequest::new(snapshot.clone(), delta.clone(), base, index_path.clone())
            .map_err(|error| map_index(error, context.attempt))?,
        control.cancelled(),
        |progress| report_index(control, progress, &fetched.revision, ready.as_ref()),
    ) {
        Ok(staged) => staged,
        Err(StageIndexError::RebuildRequired) => {
            delta = derive_delta(
                &DeltaRequest::initial(
                    context.source.id().clone(),
                    context.repository.clone(),
                    &fetched.revision,
                    context.source.notes_folder().clone(),
                )
                .map_err(|error| map_delta(error, context.attempt))?,
                control.cancelled(),
            )
            .map_err(|error| map_delta(error, context.attempt))?;
            stage_index(
                &StageIndexRequest::new(snapshot.clone(), delta, None, index_path)
                    .map_err(|error| map_index(error, context.attempt))?,
                control.cancelled(),
                |progress| report_index(control, progress, &fetched.revision, ready.as_ref()),
            )
            .map_err(|error| map_index(error, context.attempt))?
        }
        Err(error) => return Err(map_index(error, context.attempt)),
    };

    let generation = GenerationId::parse(&format!("refresh-{}", context.operation))
        .map_err(|_| RefreshFailure::never(RefreshFailureReason::RequestRefused))?;
    let expected = ready
        .as_ref()
        .map(|active| active.binding().generation.clone());
    let publication = PublishGenerationRequest::new(
        context.source.clone(),
        GenerationBinding::new(context.source.id().clone(), generation),
        expected,
        snapshot,
        staged,
    )
    .map_err(|error| map_publication(error, context.attempt))?;
    let outcome = control.activate(|cancelled| {
        store
            .publish(&publication, cancelled, |stage| {
                report_publication(control, stage, &fetched.revision, ready.as_ref());
            })
            .map_err(|error| map_publication(error, context.attempt))
    })?;
    Ok(RefreshCompletion {
        fetched_revision: fetched.revision,
        ready_revision: outcome.generation.revision().to_owned(),
        ready_generation: outcome.generation.binding().generation.clone(),
    })
}

fn derive_refresh_delta(
    context: &RefreshContext,
    ready: Option<&crate::GenerationLease>,
    revision: &str,
    control: &RefreshControl,
) -> Result<slipbox_git::DeltaOutcome, DeltaError> {
    let request = match ready {
        Some(active) => DeltaRequest::between(
            context.source.id().clone(),
            context.repository.clone(),
            active.revision(),
            revision,
            context.source.notes_folder().clone(),
        )?
        .with_previous_notes_folder(active.notes_folder().clone()),
        None => DeltaRequest::initial(
            context.source.id().clone(),
            context.repository.clone(),
            revision,
            context.source.notes_folder().clone(),
        )?,
    };
    match derive_delta(&request, control.cancelled()) {
        Err(DeltaError::RevisionUnavailable) if ready.is_some() => derive_delta(
            &DeltaRequest::initial(
                context.source.id().clone(),
                context.repository.clone(),
                revision,
                context.source.notes_folder().clone(),
            )?,
            control.cancelled(),
        ),
        outcome => outcome,
    }
}

fn report_git(
    control: &RefreshControl,
    progress: GitProgress,
    ready_revision: Option<&str>,
    ready_generation: Option<&GenerationId>,
) {
    let _ = control.report(
        RefreshState::Fetching,
        RefreshProgress {
            completed: progress.objects,
            total: 0,
            unit: if progress.stage == ProgressStage::Preparing {
                RefreshProgressUnit::None
            } else {
                RefreshProgressUnit::Objects
            },
        },
        None,
        ready_revision,
        ready_generation,
    );
}

fn report_snapshot(
    control: &RefreshControl,
    progress: SnapshotProgress,
    fetched_revision: &str,
    ready: Option<&crate::GenerationLease>,
) {
    let _ = control.report(
        RefreshState::Materializing,
        RefreshProgress {
            completed: progress.entries,
            total: 0,
            unit: RefreshProgressUnit::Entries,
        },
        Some(fetched_revision),
        ready.map(|active| active.revision()),
        ready.map(|active| &active.binding().generation),
    );
}

fn report_index(
    control: &RefreshControl,
    progress: StageIndexProgress,
    fetched_revision: &str,
    ready: Option<&crate::GenerationLease>,
) {
    let _ = control.report(
        RefreshState::Indexing,
        RefreshProgress {
            completed: progress.completed_files,
            total: progress.total_files,
            unit: RefreshProgressUnit::Files,
        },
        Some(fetched_revision),
        ready.map(|active| active.revision()),
        ready.map(|active| &active.binding().generation),
    );
}

fn report_publication(
    control: &RefreshControl,
    stage: PublicationStage,
    fetched_revision: &str,
    ready: Option<&crate::GenerationLease>,
) {
    let completed = match stage {
        PublicationStage::Preparing => 0,
        PublicationStage::SourceSealed => 1,
        PublicationStage::IndexSealed => 2,
        PublicationStage::ManifestSealed => 3,
        PublicationStage::Activating => 4,
        PublicationStage::Retained => 5,
        PublicationStage::Complete => 6,
    };
    let _ = control.report(
        RefreshState::Publishing,
        RefreshProgress {
            completed,
            total: 6,
            unit: RefreshProgressUnit::Steps,
        },
        Some(fetched_revision),
        ready.map(|active| active.revision()),
        ready.map(|active| &active.binding().generation),
    );
}

fn map_git(error: GitError, attempt: u8) -> RefreshFailure {
    match error {
        GitError::DestinationRefused => never(RefreshFailureReason::UnsafeSource),
        GitError::CredentialRefused => reauthorize(RefreshFailureReason::AuthorizationFailed),
        GitError::Cancelled => never(RefreshFailureReason::Cancelled),
        GitError::StorageFailed => free_storage(),
        GitError::RepositoryInvalid => never(RefreshFailureReason::RepositoryInvalid),
        GitError::BranchUnavailable => never(RefreshFailureReason::BranchUnavailable),
        GitError::TransportFailed => backoff(RefreshFailureReason::TransportFailed, attempt),
    }
}

fn map_delta(error: DeltaError, attempt: u8) -> RefreshFailure {
    match error {
        DeltaError::LimitsRefused => never(RefreshFailureReason::RequestRefused),
        DeltaError::RepositoryInvalid => never(RefreshFailureReason::RepositoryInvalid),
        DeltaError::RevisionRefused | DeltaError::UnsafePath | DeltaError::ObjectInvalid => {
            never(RefreshFailureReason::UnsafeSource)
        }
        DeltaError::RevisionUnavailable => {
            backoff(RefreshFailureReason::RepositoryInvalid, attempt)
        }
        DeltaError::NotesFolderUnavailable => never(RefreshFailureReason::NotesFolderUnavailable),
        DeltaError::InputExhausted | DeltaError::DiagnosticsExhausted => {
            never(RefreshFailureReason::InputExhausted)
        }
        DeltaError::Cancelled => never(RefreshFailureReason::Cancelled),
    }
}

fn map_snapshot(error: SnapshotError, attempt: u8) -> RefreshFailure {
    match error {
        SnapshotError::LimitsRefused | SnapshotError::DestinationRefused => {
            never(RefreshFailureReason::RequestRefused)
        }
        SnapshotError::DestinationOccupied => {
            backoff(RefreshFailureReason::PublicationConflict, attempt)
        }
        SnapshotError::StorageBoundary | SnapshotError::StorageFailed => free_storage(),
        SnapshotError::OwnershipMismatch
        | SnapshotError::RevisionRefused
        | SnapshotError::UnsafePath
        | SnapshotError::ObjectInvalid => never(RefreshFailureReason::UnsafeSource),
        SnapshotError::RepositoryInvalid | SnapshotError::RevisionUnavailable => {
            backoff(RefreshFailureReason::RepositoryInvalid, attempt)
        }
        SnapshotError::NotesFolderUnavailable => {
            never(RefreshFailureReason::NotesFolderUnavailable)
        }
        SnapshotError::InputExhausted | SnapshotError::DiagnosticsExhausted => {
            never(RefreshFailureReason::InputExhausted)
        }
        SnapshotError::Cancelled => never(RefreshFailureReason::Cancelled),
    }
}

fn map_index(error: StageIndexError, attempt: u8) -> RefreshFailure {
    match error {
        StageIndexError::RequestRefused => never(RefreshFailureReason::RequestRefused),
        StageIndexError::DestinationOccupied => {
            backoff(RefreshFailureReason::PublicationConflict, attempt)
        }
        StageIndexError::SnapshotUnavailable
        | StageIndexError::BaseUnavailable
        | StageIndexError::BaseMismatch
        | StageIndexError::InputInvalid => never(RefreshFailureReason::UnsafeSource),
        StageIndexError::RebuildRequired => {
            RefreshFailure::with_retry(RefreshFailureReason::RebuildRequired, RefreshRetry::Rebuild)
        }
        StageIndexError::Cancelled => never(RefreshFailureReason::Cancelled),
        StageIndexError::IndexFailed => {
            RefreshFailure::with_retry(RefreshFailureReason::RebuildRequired, RefreshRetry::Rebuild)
        }
        StageIndexError::StorageFailed => free_storage(),
    }
}

fn map_publication(error: PublicationError, attempt: u8) -> RefreshFailure {
    match error {
        PublicationError::RequestRefused
        | PublicationError::OwnershipMismatch
        | PublicationError::CandidateMismatch => never(RefreshFailureReason::UnsafeSource),
        PublicationError::StoreUnavailable | PublicationError::StorageFailed => free_storage(),
        PublicationError::CandidateUnavailable | PublicationError::GenerationOccupied => {
            backoff(RefreshFailureReason::PublicationConflict, attempt)
        }
        PublicationError::PublicationInProgress
        | PublicationError::RecoveryInProgress
        | PublicationError::ActiveGenerationChanged => {
            backoff(RefreshFailureReason::PublicationConflict, attempt)
        }
        PublicationError::RebuildRequired => {
            RefreshFailure::with_retry(RefreshFailureReason::RebuildRequired, RefreshRetry::Rebuild)
        }
        PublicationError::Cancelled => never(RefreshFailureReason::Cancelled),
    }
}

fn never(reason: RefreshFailureReason) -> RefreshFailure {
    RefreshFailure::never(reason)
}

fn reauthorize(reason: RefreshFailureReason) -> RefreshFailure {
    RefreshFailure::with_retry(reason, RefreshRetry::Reauthorize)
}

fn free_storage() -> RefreshFailure {
    RefreshFailure::with_retry(
        RefreshFailureReason::StorageFailed,
        RefreshRetry::FreeStorage,
    )
}

fn backoff(reason: RefreshFailureReason, attempt: u8) -> RefreshFailure {
    let retry = if attempt + 1 >= MAX_REFRESH_ATTEMPTS {
        RefreshRetry::Never
    } else {
        RefreshRetry::Backoff {
            after_seconds: 30_u64 << (u32::from(attempt) * 2),
        }
    };
    RefreshFailure::with_retry(reason, retry)
}

fn failed_status(context: &RefreshContext, failure: RefreshFailure) -> RefreshStatus {
    RefreshStatus {
        source: context.source.clone(),
        operation: context.operation,
        state: if failure.reason == RefreshFailureReason::Cancelled
            || failure.reason == RefreshFailureReason::Superseded
        {
            RefreshState::Cancelled
        } else {
            RefreshState::Failed
        },
        fetched_revision: None,
        ready_revision: None,
        ready_generation: None,
        progress: RefreshProgress::none(),
        failure: Some(failure),
    }
}

fn admitted_path(path: &Path) -> bool {
    let bytes = path.as_os_str().as_encoded_bytes();
    path.is_absolute()
        && !bytes.is_empty()
        && bytes.len() <= MAX_REFRESH_PATH_BYTES
        && path.parent().is_some()
        && path.components().all(|component| {
            matches!(
                component,
                Component::Prefix(_) | Component::RootDir | Component::Normal(_)
            )
        })
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poison| poison.into_inner())
}

fn wait<'a, T>(
    condvar: &Condvar,
    guard: std::sync::MutexGuard<'a, T>,
) -> std::sync::MutexGuard<'a, T> {
    condvar
        .wait(guard)
        .unwrap_or_else(|poison| poison.into_inner())
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Barrier};
    use std::thread;

    use slipbox_core::{
        GitBranch, NotesFolder, RemoteUrl, SourceConfiguration, SourceDisplayName, SourceProvider,
        SourceVisibility,
    };

    use super::*;

    const SOURCE: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn equivalent_overlaps_execute_once_and_return_one_completion() {
        let coordinator = Arc::new(RefreshCoordinator::default());
        let started = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        let executions = Arc::new(AtomicUsize::new(0));
        let leader = {
            let coordinator = Arc::clone(&coordinator);
            let started = Arc::clone(&started);
            let release = Arc::clone(&release);
            let executions = Arc::clone(&executions);
            thread::spawn(move || {
                coordinator.coordinate(context(1, "main"), |_| {
                    executions.fetch_add(1, Ordering::Relaxed);
                    started.wait();
                    release.wait();
                    Ok(completion("g1"))
                })
            })
        };
        started.wait();
        let follower = {
            let coordinator = Arc::clone(&coordinator);
            thread::spawn(move || {
                coordinator.coordinate(context_with_name(2, "main", "Renamed notes"), |_| {
                    panic!("a coalesced request must not execute work")
                })
            })
        };
        while !coordinator.has_operation(2) {
            thread::yield_now();
        }
        release.wait();
        let leader = leader.join().expect("leader");
        let follower = follower.join().expect("follower");
        assert_eq!(executions.load(Ordering::Relaxed), 1);
        assert_eq!(leader.disposition, RefreshDisposition::Started);
        assert_eq!(follower.disposition, RefreshDisposition::Coalesced);
        assert_eq!(
            leader.status.ready_generation,
            follower.status.ready_generation
        );
        assert_eq!(follower.status.operation, 2);
        assert_eq!(
            follower.status.source.display_name().as_str(),
            "Renamed notes"
        );
    }

    #[test]
    fn changed_configuration_cancels_old_work_before_its_activation() {
        let coordinator = Arc::new(RefreshCoordinator::default());
        let entered = Arc::new(Barrier::new(2));
        let old = {
            let coordinator = Arc::clone(&coordinator);
            let entered = Arc::clone(&entered);
            thread::spawn(move || {
                coordinator.coordinate(context(11, "main"), |control| {
                    entered.wait();
                    while !control.cancelled().load(Ordering::Acquire) {
                        thread::yield_now();
                    }
                    control.activate(|_| Ok(()))?;
                    Ok(completion("old"))
                })
            })
        };
        entered.wait();
        let new = coordinator.coordinate(context(12, "other"), |_| Ok(completion("new")));
        let old = old.join().expect("old");
        assert_eq!(old.status.state, RefreshState::Cancelled);
        assert_eq!(
            old.status.failure.expect("failure").reason,
            RefreshFailureReason::Superseded
        );
        assert_eq!(new.status.ready_generation, Some(generation("new")));
        assert_eq!(new.status.source.branch().as_str(), "other");
    }

    #[test]
    fn cancellation_is_source_scoped_and_preserves_observed_readiness() {
        let coordinator = Arc::new(RefreshCoordinator::default());
        let entered = Arc::new(Barrier::new(2));
        let running = {
            let coordinator = Arc::clone(&coordinator);
            let entered = Arc::clone(&entered);
            thread::spawn(move || {
                coordinator.coordinate(context(21, "main"), |control| {
                    control.report(
                        RefreshState::Indexing,
                        RefreshProgress {
                            completed: 2,
                            total: 9,
                            unit: RefreshProgressUnit::Files,
                        },
                        Some("fetched"),
                        Some("ready"),
                        Some(&generation("base")),
                    )?;
                    entered.wait();
                    while !control.cancelled().load(Ordering::Acquire) {
                        thread::yield_now();
                    }
                    Err(RefreshFailure::never(RefreshFailureReason::Cancelled))
                })
            })
        };
        entered.wait();
        assert!(coordinator.cancel(21));
        let outcome = running.join().expect("running");
        assert_eq!(outcome.status.state, RefreshState::Cancelled);
        assert_eq!(outcome.status.fetched_revision.as_deref(), Some("fetched"));
        assert_eq!(outcome.status.ready_revision.as_deref(), Some("ready"));
        assert!(!coordinator.cancel(21));
    }

    #[test]
    fn retry_backoff_is_bounded_by_the_declared_attempt_limit() {
        assert_eq!(
            backoff(RefreshFailureReason::TransportFailed, 0).retry,
            RefreshRetry::Backoff { after_seconds: 30 }
        );
        assert_eq!(
            backoff(RefreshFailureReason::TransportFailed, 1).retry,
            RefreshRetry::Backoff { after_seconds: 120 }
        );
        assert_eq!(
            backoff(RefreshFailureReason::TransportFailed, 2).retry,
            RefreshRetry::Never
        );
        assert_eq!(
            map_index(StageIndexError::IndexFailed, 0),
            RefreshFailure::with_retry(
                RefreshFailureReason::RebuildRequired,
                RefreshRetry::Rebuild
            )
        );
        assert_eq!(
            map_publication(PublicationError::GenerationOccupied, 1),
            RefreshFailure::with_retry(
                RefreshFailureReason::PublicationConflict,
                RefreshRetry::Backoff { after_seconds: 120 }
            )
        );
    }

    #[test]
    fn a_panicking_leader_releases_followers_and_the_source_slot() {
        let coordinator = Arc::new(RefreshCoordinator::default());
        let started = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        let leader = {
            let coordinator = Arc::clone(&coordinator);
            let started = Arc::clone(&started);
            let release = Arc::clone(&release);
            thread::spawn(move || {
                coordinator.coordinate(context(31, "main"), |_| {
                    started.wait();
                    release.wait();
                    panic!("synthetic refresh panic")
                })
            })
        };
        started.wait();
        let follower = {
            let coordinator = Arc::clone(&coordinator);
            thread::spawn(move || {
                coordinator.coordinate(context(32, "main"), |_| {
                    panic!("a coalesced request must not execute work")
                })
            })
        };
        while !coordinator.has_operation(32) {
            thread::yield_now();
        }
        release.wait();
        assert!(leader.join().is_err());
        let follower = follower.join().expect("follower");
        assert_eq!(follower.status.state, RefreshState::Failed);
        assert_eq!(
            follower.status.failure.expect("failure").reason,
            RefreshFailureReason::Panicked
        );
        assert!(!coordinator.has_operation(31));
        assert!(!coordinator.has_operation(32));
        assert_eq!(
            coordinator
                .coordinate(context(33, "main"), |_| Ok(completion("recovered")))
                .status
                .ready_generation,
            Some(generation("recovered"))
        );
    }

    #[test]
    fn cancellation_after_activation_begins_cannot_claim_a_committed_refresh() {
        let coordinator = Arc::new(RefreshCoordinator::default());
        let entered = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        let leader = {
            let coordinator = Arc::clone(&coordinator);
            let entered = Arc::clone(&entered);
            let release = Arc::clone(&release);
            thread::spawn(move || {
                coordinator.coordinate(context(41, "main"), |control| {
                    control.activate(|_| {
                        entered.wait();
                        release.wait();
                        Ok(())
                    })?;
                    Ok(completion("committed"))
                })
            })
        };
        entered.wait();
        let cancellation = {
            let coordinator = Arc::clone(&coordinator);
            thread::spawn(move || coordinator.cancel(41))
        };
        release.wait();
        let outcome = leader.join().expect("leader");
        assert!(!cancellation.join().expect("cancellation"));
        assert_eq!(outcome.status.state, RefreshState::Ready);
        assert_eq!(
            outcome.status.ready_generation,
            Some(generation("committed"))
        );
    }

    fn context(operation: i64, branch: &str) -> RefreshContext {
        context_with_name(operation, branch, "Notes")
    }

    fn context_with_name(operation: i64, branch: &str, name: &str) -> RefreshContext {
        RefreshContext::new(
            operation,
            0,
            record(branch, name),
            PathBuf::from(format!("/private/{SOURCE}/repository.git")),
            PathBuf::from(format!("/private/{SOURCE}/store")),
        )
        .expect("context")
    }

    fn record(branch: &str, name: &str) -> SourceRecord {
        SourceRecord::new(SourceConfiguration {
            id: SourceId::parse(SOURCE).expect("source"),
            display_name: SourceDisplayName::parse(name).expect("name"),
            provider: SourceProvider::GenericHttps,
            visibility: SourceVisibility::Public,
            provider_repository_id: None,
            account: None,
            remote: RemoteUrl::parse("https://example.com/notes.git").expect("remote"),
            branch: GitBranch::parse(branch).expect("branch"),
            notes_folder: NotesFolder::parse("notes").expect("folder"),
            credential: None,
        })
        .expect("record")
    }

    fn completion(generation: &str) -> RefreshCompletion {
        RefreshCompletion {
            fetched_revision: "fetched".to_owned(),
            ready_revision: "ready".to_owned(),
            ready_generation: self::generation(generation),
        }
    }

    fn generation(value: &str) -> GenerationId {
        GenerationId::parse(value).expect("generation")
    }
}
