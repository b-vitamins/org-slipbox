//! Source/generation-bound engine sessions and their owned handle registry.
//! Wire DTOs and limits live in [`slipbox_rpc::android`].

use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use serde_json::Value;
use slipbox_core::GenerationBinding;
use slipbox_engine::service::SlipboxService;
use slipbox_engine::{DiscoveryPolicy, PlatformPolicy};
use slipbox_rpc::android::{
    ADAPTER_LIMITS, AdapterBound, AdapterCapability, AdapterContract, AdapterOutcome,
    AdapterRefusal, AdapterResponse, AnsweredOperation, CloseRequest, ClosedSession, EngineRefusal,
    MaintenanceRequest, OpenRequest, OpenedSession, OperationAnswer, ReadAnswer, ReadRequest,
    RefusalReason, ResponseEncoding, SessionContext, SessionIdentity, decode_close_request,
    decode_maintenance_request, decode_open_request, decode_read_request, encode_refusal,
    encode_response, write_response,
};
use slipbox_rpc::{JsonRpcError, OperationMutation, operation_descriptor_by_method};

/// The sessions this library owns.
///
/// One table per loaded library: session identities are only meaningful against
/// the table that allocated them.
static SESSIONS: Registry = Registry::new();

pub(crate) fn sessions() -> &'static Registry {
    &SESSIONS
}

/// A reply this library can produce without running the call that failed.
pub trait Refusable {
    fn refused(refusal: AdapterRefusal) -> Self;
}

impl Refusable for Vec<u8> {
    fn refused(refusal: AdapterRefusal) -> Self {
        encode_refusal(refusal)
    }
}

impl Refusable for Served {
    fn refused(refusal: AdapterRefusal) -> Self {
        Served {
            response: encode_refusal(refusal),
            opened: None,
        }
    }
}

/// Contain Rust unwinding before it reaches JNI, returning a structured refusal.
pub fn contained<T: Refusable>(call: impl FnOnce() -> T) -> T {
    panic::catch_unwind(AssertUnwindSafe(call))
        .unwrap_or_else(|_| T::refused(AdapterRefusal::of(RefusalReason::Panicked)))
}

/// One answer, and the session it opened if it opened one.
///
/// A caller that cannot deliver the answer owns that session and must retire it:
/// nothing else can name a handle its caller never received.
pub struct Served {
    pub response: Vec<u8>,
    pub opened: Option<SessionIdentity>,
}

/// Answers with what this library admits.
#[must_use]
pub fn serve_contract() -> Vec<u8> {
    answer(Ok(AdapterOutcome::Contract(AdapterContract::default())))
}

#[must_use]
pub fn serve_open(sessions: &Registry, capability: AdapterCapability, request: &[u8]) -> Served {
    match decode_open_request(request).and_then(|request| sessions.open(capability, &request)) {
        Ok(session) => {
            let handle = session.handle;
            let opened = AdapterResponse::new(AdapterOutcome::Opened(session));
            match write_response(&opened) {
                ResponseEncoding::Written(response) => Served {
                    response,
                    opened: Some(handle),
                },
                // The answer that would have named this handle is not going out,
                // so nothing else could ever retire it.
                undelivered => {
                    sessions.abandon(handle);
                    Served::refused(undeliverable(&undelivered))
                }
            }
        }
        Err(refusal) => Served::refused(refusal),
    }
}

fn undeliverable(encoding: &ResponseEncoding) -> AdapterRefusal {
    match encoding {
        ResponseEncoding::Oversized { .. } => AdapterRefusal::bounded(AdapterBound::ResponseBytes),
        _ => AdapterRefusal::of(RefusalReason::EncodingFailed),
    }
}

#[must_use]
pub fn serve_read(sessions: &Registry, request: &[u8]) -> Vec<u8> {
    answer(
        decode_read_request(request)
            .and_then(|request| sessions.read(&request))
            .map(AdapterOutcome::Answered),
    )
}

#[must_use]
pub fn serve_maintenance(sessions: &Registry, request: &[u8]) -> Vec<u8> {
    answer(
        decode_maintenance_request(request)
            .and_then(|request| sessions.maintain(&request))
            .map(AdapterOutcome::Answered),
    )
}

#[must_use]
pub fn serve_close(sessions: &Registry, request: &[u8]) -> Vec<u8> {
    answer(
        decode_close_request(request)
            .and_then(|request| sessions.close(&request))
            .map(AdapterOutcome::Closed),
    )
}

fn answer(outcome: Result<AdapterOutcome, AdapterRefusal>) -> Vec<u8> {
    match outcome {
        Ok(outcome) => encode_response(&AdapterResponse::new(outcome)),
        Err(refusal) => encode_refusal(refusal),
    }
}

/// The owned table of open sessions.
pub struct Registry {
    table: Mutex<Table>,
}

struct Table {
    open: Vec<Arc<Session>>,
    /// The next identity to allocate. It only ever increases, so a retired
    /// identity is never handed out again and no answer can reach a session
    /// other than the one that was asked.
    next: SessionIdentity,
}

impl Registry {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            table: Mutex::new(Table {
                open: Vec::new(),
                next: 1,
            }),
        }
    }

    /// Opens one session against the root, database and binding the request
    /// names.
    ///
    /// The engine is constructed while the table is held, so a table that
    /// reports a session always has one.
    pub fn open(
        &self,
        capability: AdapterCapability,
        request: &OpenRequest,
    ) -> Result<OpenedSession, AdapterRefusal> {
        if request.capability != capability {
            return Err(AdapterRefusal::of(RefusalReason::CapabilityMismatch));
        }
        if let Some(bound) = request.context.violated_bound() {
            return Err(AdapterRefusal::bounded(bound));
        }
        let (root, database) = named_paths(&request.context)?;

        let mut table = self.table()?;
        let identity = table.allocate()?;
        let engine = SlipboxService::with_platform(
            root,
            database,
            Vec::new(),
            DiscoveryPolicy::default(),
            // Android runs no external program, whatever a desktop feature set
            // compiles in.
            PlatformPolicy::headless(),
        )
        .map_err(|_| AdapterRefusal::of(RefusalReason::InvalidContext))?;

        table.open.push(Arc::new(Session {
            identity,
            capability,
            binding: request.binding.clone(),
            retired: AtomicBool::new(false),
            engine: Mutex::new(Some(engine)),
        }));
        Ok(OpenedSession {
            handle: identity,
            capability,
            binding: request.binding.clone(),
        })
    }

    pub fn read(&self, request: &ReadRequest) -> Result<AnsweredOperation, AdapterRefusal> {
        if let Some(bound) = request.operation.violated_bound() {
            return Err(AdapterRefusal::bounded(bound));
        }
        let session = self.session(request.handle, AdapterCapability::Read, &request.binding)?;
        let method = request.operation.method();
        admits(method, AdapterCapability::Read)?;
        let result = session.answer(method, request.operation.params()?)?;
        let answer = request.operation.answered(result)?;
        complete_source(&answer)?;
        Ok(session.answered(answer.into()))
    }

    pub fn maintain(
        &self,
        request: &MaintenanceRequest,
    ) -> Result<AnsweredOperation, AdapterRefusal> {
        if let Some(bound) = request.operation.violated_bound() {
            return Err(AdapterRefusal::bounded(bound));
        }
        let session = self.session(
            request.handle,
            AdapterCapability::Maintenance,
            &request.binding,
        )?;
        let method = request.operation.method();
        admits(method, AdapterCapability::Maintenance)?;
        let result = session.answer(method, request.operation.params()?)?;
        Ok(session.answered(request.operation.answered(result)?.into()))
    }

    /// Retires one session. The call that removes it from the table reports the
    /// retirement and the binding it retired; a later repeat reports that there
    /// was nothing left to retire, without retaining a record of it.
    ///
    /// A request naming another binding retires nothing and leaves the session
    /// open, so a caller that has confused two sessions loses neither.
    pub fn close(&self, request: &CloseRequest) -> Result<ClosedSession, AdapterRefusal> {
        let identity = request.handle;
        let mut table = self.table()?;
        if !table.allocated(identity) {
            return Err(AdapterRefusal::of(RefusalReason::UnknownHandle));
        }
        let Some(position) = table
            .open
            .iter()
            .position(|session| session.identity == identity)
        else {
            return Ok(ClosedSession {
                handle: identity,
                retired: false,
                binding: None,
            });
        };
        if table.open[position].binding != request.binding {
            return Err(AdapterRefusal::of(RefusalReason::BindingMismatch));
        }
        let session = table.open.remove(position);
        drop(table);

        session.retire();
        Ok(ClosedSession {
            handle: identity,
            retired: true,
            binding: Some(session.binding.clone()),
        })
    }

    /// Retires a session whose answer never reached the caller that would have
    /// owned it, and reports whether there was one to retire. Only that session
    /// is released; a pre-existing session under another identity is untouched.
    pub fn abandon(&self, identity: SessionIdentity) -> bool {
        let Ok(mut table) = self.table.lock() else {
            return false;
        };
        let Some(position) = table
            .open
            .iter()
            .position(|session| session.identity == identity)
        else {
            return false;
        };
        let session = table.open.remove(position);
        drop(table);

        session.retire();
        true
    }

    fn session(
        &self,
        identity: SessionIdentity,
        capability: AdapterCapability,
        binding: &GenerationBinding,
    ) -> Result<Arc<Session>, AdapterRefusal> {
        let session = self.table()?.find(identity)?;
        if session.capability != capability {
            return Err(AdapterRefusal::of(RefusalReason::CapabilityMismatch));
        }
        if &session.binding != binding {
            return Err(AdapterRefusal::of(RefusalReason::BindingMismatch));
        }
        Ok(session)
    }

    fn table(&self) -> Result<MutexGuard<'_, Table>, AdapterRefusal> {
        self.table
            .lock()
            .map_err(|_| AdapterRefusal::of(RefusalReason::EngineFailed))
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

impl Table {
    /// Reserves the next identity, or refuses because the table is full or
    /// because no identity is left to reserve.
    fn allocate(&mut self) -> Result<SessionIdentity, AdapterRefusal> {
        if self.open.len() >= ADAPTER_LIMITS.max_open_sessions {
            return Err(AdapterRefusal {
                reason: RefusalReason::SessionsExhausted,
                bound: Some(AdapterBound::OpenSessions),
                engine: None,
            });
        }
        let next = self
            .next
            .checked_add(1)
            .ok_or(AdapterRefusal::of(RefusalReason::IdentitiesExhausted))?;
        let identity = self.next;
        self.next = next;
        Ok(identity)
    }

    fn allocated(&self, identity: SessionIdentity) -> bool {
        identity > 0 && identity < self.next
    }

    fn find(&self, identity: SessionIdentity) -> Result<Arc<Session>, AdapterRefusal> {
        if !self.allocated(identity) {
            return Err(AdapterRefusal::of(RefusalReason::UnknownHandle));
        }
        self.open
            .iter()
            .find(|session| session.identity == identity)
            .cloned()
            .ok_or(AdapterRefusal::of(RefusalReason::RetiredHandle))
    }
}

/// One open session: one engine, one binding, one root and database, from
/// construction to retirement.
///
/// The root and database are the engine's, held by the engine the session owns,
/// so no request can move the session to another pair and `slipbox/status`
/// reports the same pair on the last call before retirement as on the first.
struct Session {
    identity: SessionIdentity,
    capability: AdapterCapability,
    binding: GenerationBinding,
    retired: AtomicBool,
    engine: Mutex<Option<SlipboxService>>,
}

impl Session {
    fn answer(&self, method: &str, params: Value) -> Result<Value, AdapterRefusal> {
        if self.retired.load(Ordering::SeqCst) {
            return Err(AdapterRefusal::of(RefusalReason::RetiredHandle));
        }
        let mut engine = self
            .engine
            .lock()
            .map_err(|_| AdapterRefusal::of(RefusalReason::EngineFailed))?;
        // Read again under the lock: a retirement that raced the check above
        // must not be answered.
        if self.retired.load(Ordering::SeqCst) {
            return Err(AdapterRefusal::of(RefusalReason::RetiredHandle));
        }
        let engine = engine
            .as_mut()
            .ok_or(AdapterRefusal::of(RefusalReason::RetiredHandle))?;
        engine
            .invoke_value(method, params)
            .map_err(refused_by_engine)
    }

    /// The answer names the binding the session holds, never one a request
    /// supplied.
    fn answered(&self, answer: OperationAnswer) -> AnsweredOperation {
        AnsweredOperation {
            handle: self.identity,
            binding: self.binding.clone(),
            answer,
        }
    }

    /// Refuses further work and releases the engine as soon as no call holds it.
    /// Work already dispatched finishes; nothing new is admitted.
    fn retire(&self) {
        self.retired.store(true, Ordering::SeqCst);
        if let Ok(mut engine) = self.engine.try_lock() {
            *engine = None;
        }
    }
}

/// Refuses a method the canonical classification does not place in this
/// capability, so an operation added to the engine later is refused here until
/// it is admitted deliberately.
fn admits(method: &str, capability: AdapterCapability) -> Result<(), AdapterRefusal> {
    let descriptor = operation_descriptor_by_method(method)
        .ok_or(AdapterRefusal::of(RefusalReason::UnknownOperation))?;
    let admitted = match capability {
        AdapterCapability::Read => descriptor.mutation == OperationMutation::ReadOnly,
        AdapterCapability::Maintenance => descriptor.mutation == OperationMutation::DerivedIndex,
    };
    if admitted {
        Ok(())
    } else {
        Err(AdapterRefusal::of(RefusalReason::UnknownOperation))
    }
}

/// Keeps the engine's numbers and drops its prose, which quotes the node keys,
/// paths and queries a caller supplied.
fn refused_by_engine(error: JsonRpcError) -> AdapterRefusal {
    let error = error.into_inner();
    let reason = if error.code == -32603 {
        RefusalReason::EngineFailed
    } else {
        RefusalReason::EngineRefused
    };
    AdapterRefusal::engine(
        reason,
        EngineRefusal {
            code: error.code,
            kind: error.data.map(|data| data.kind),
        },
    )
}

/// Refuses a source answer the engine had to cut short, rather than passing a
/// partial note off as the note.
fn complete_source(answer: &ReadAnswer) -> Result<(), AdapterRefusal> {
    let ReadAnswer::ReadNodeSource(answer) = answer else {
        return Ok(());
    };
    if answer.source.truncated_before || answer.source.truncated_after {
        return Err(AdapterRefusal::bounded(AdapterBound::NoteSourceLines));
    }
    Ok(())
}

/// A session names both of its paths explicitly; there is no root to fall back
/// on and no relative path to resolve against one.
fn named_paths(context: &SessionContext) -> Result<(PathBuf, PathBuf), AdapterRefusal> {
    let root = PathBuf::from(&context.root);
    let database = PathBuf::from(&context.database);
    let named = |path: &Path| path.is_absolute() && path.file_name().is_some();
    if !named(&root) || !named(&database) || root == database {
        return Err(AdapterRefusal::of(RefusalReason::InvalidContext));
    }
    Ok((root, database))
}

#[cfg(test)]
mod tests {
    use slipbox_rpc::{METHOD_CAPTURE_NODE, METHOD_INDEX, METHOD_STATUS};

    use super::*;

    fn table(next: SessionIdentity) -> Table {
        Table {
            open: Vec::new(),
            next,
        }
    }

    fn reason(refusal: &AdapterRefusal) -> RefusalReason {
        refusal.reason
    }

    #[test]
    fn identities_run_out_rather_than_wrapping_round_to_a_live_session() {
        let refusal = table(SessionIdentity::MAX)
            .allocate()
            .expect_err("the last identity cannot be followed by another");
        assert_eq!(reason(&refusal), RefusalReason::IdentitiesExhausted);

        let mut fresh = table(1);
        assert_eq!(fresh.allocate().expect("an empty table allocates"), 1);
        assert_eq!(fresh.allocate().expect("and then allocates again"), 2);
    }

    #[test]
    fn an_identity_the_table_never_allocated_is_told_apart_from_a_retired_one() {
        let table = table(5);
        for unknown in [0, 5, 6, -1, SessionIdentity::MAX] {
            let refusal = table
                .find(unknown)
                .err()
                .unwrap_or_else(|| panic!("{unknown} was never allocated"));
            assert_eq!(reason(&refusal), RefusalReason::UnknownHandle, "{unknown}");
        }
        for retired in [1, 4] {
            let refusal = table
                .find(retired)
                .err()
                .unwrap_or_else(|| panic!("{retired} holds no session"));
            assert_eq!(reason(&refusal), RefusalReason::RetiredHandle, "{retired}");
        }
    }

    #[test]
    fn each_capability_admits_only_the_mutation_class_it_owns() {
        assert!(admits(METHOD_STATUS, AdapterCapability::Read).is_ok());
        assert!(admits(METHOD_INDEX, AdapterCapability::Maintenance).is_ok());

        for (method, capability) in [
            (METHOD_STATUS, AdapterCapability::Maintenance),
            (METHOD_INDEX, AdapterCapability::Read),
            (METHOD_CAPTURE_NODE, AdapterCapability::Read),
            (METHOD_CAPTURE_NODE, AdapterCapability::Maintenance),
            ("slipbox/invented", AdapterCapability::Read),
            ("slipbox/invented", AdapterCapability::Maintenance),
        ] {
            let refusal = admits(method, capability)
                .err()
                .unwrap_or_else(|| panic!("{method} is admitted as {capability:?}"));
            assert_eq!(
                reason(&refusal),
                RefusalReason::UnknownOperation,
                "{method}"
            );
        }
    }
}
