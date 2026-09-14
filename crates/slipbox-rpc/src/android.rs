//! Versioned Android wire DTOs using canonical engine parameters and results.
//! JNI transports UTF-8 JSON; operations and refusal categories are closed.
//! Boundary limits are declared in [`ADAPTER_LIMITS`].

use std::collections::BTreeSet;
use std::fmt;
use std::io::{self, Write};

use serde::de::{self, DeserializeOwned, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use slipbox_core::{
    BacklinksParams, BacklinksResult, ExploreParams, ExploreResult, ForwardLinksParams,
    ForwardLinksResult, GenerationBinding, GlossaryTermParams, GlossaryTermResult, IndexFileParams,
    IndexFileResult, IndexStats, IndexedFilesResult, ListGlossaryTermsParams,
    ListGlossaryTermsResult, NodeFromIdParams, NodeFromKeyParams, NodeRecord, ReadNodeSourceParams,
    ReadNodeSourceResult, SearchGlossaryParams, SearchGlossaryResult, SearchNodeContentParams,
    SearchNodeContentResult, SearchNodesParams, SearchNodesResult, StatusInfo,
};

use crate::{
    JsonRpcErrorKind, METHOD_BACKLINKS, METHOD_EXPLORE, METHOD_FORWARD_LINKS, METHOD_GLOSSARY_TERM,
    METHOD_INDEX, METHOD_INDEX_FILE, METHOD_INDEXED_FILES, METHOD_LIST_GLOSSARY_TERMS,
    METHOD_NODE_FROM_ID, METHOD_NODE_FROM_KEY, METHOD_READ_NODE_SOURCE, METHOD_SEARCH_GLOSSARY,
    METHOD_SEARCH_NODE_CONTENT, METHOD_SEARCH_NODES, METHOD_STATUS,
};

/// The supported Android adapter protocol version.
pub const ADAPTER_PROTOCOL_VERSION: u32 = 1;

/// Every finite bound the adapter boundary enforces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterLimits {
    pub max_request_bytes: usize,
    pub max_response_bytes: usize,
    pub max_page_entries: usize,
    pub max_relation_entries: usize,
    pub max_note_source_lines: usize,
    pub max_context_lines: u32,
    pub max_path_bytes: usize,
    pub max_open_sessions: usize,
    pub max_queued_requests: usize,
}

/// The page, relation, source and context bounds are the canonical clamps of the
/// operations they govern, so a request inside them is answered exactly as the
/// engine would answer it, and a request outside them is refused rather than
/// silently narrowed.
pub const ADAPTER_LIMITS: AdapterLimits = AdapterLimits {
    max_request_bytes: 64 * 1024,
    max_response_bytes: 4 * 1024 * 1024,
    max_page_entries: 200,
    max_relation_entries: 1_000,
    max_note_source_lines: 1_000,
    max_context_lines: 200,
    max_path_bytes: 4_096,
    max_open_sessions: 8,
    max_queued_requests: 32,
};

/// A monotonically allocated, non-reused table identity, not a pointer.
pub type SessionIdentity = i64;

/// Which half of the adapter a session speaks to. Reading and internal index
/// maintenance are separate capabilities: one session never admits both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AdapterCapability {
    Read,
    Maintenance,
}

impl AdapterCapability {
    /// The vocabulary this capability admits, as (discriminant, method) pairs.
    #[must_use]
    pub fn vocabulary(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Self::Read => READ_VOCABULARY,
            Self::Maintenance => MAINTENANCE_VOCABULARY,
        }
    }
}

/// The explicit root and database a session is constructed against. There is no
/// ambient root: a session that names neither cannot be opened.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionContext {
    pub root: String,
    pub database: String,
}

impl SessionContext {
    /// The bound this context violates, if any.
    #[must_use]
    pub fn violated_bound(&self) -> Option<AdapterBound> {
        let too_long = self.root.len() > ADAPTER_LIMITS.max_path_bytes
            || self.database.len() > ADAPTER_LIMITS.max_path_bytes;
        too_long.then_some(AdapterBound::PathBytes)
    }
}

/// The empty payload of an operation the engine answers from session state
/// alone.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateOnlyParams {}

/// Declares one closed operation vocabulary: the request enum Kotlin mirrors,
/// the answer enum its results are typed into, the discriminant-to-method table
/// the inventory checks read, and the canonical payloads each variant carries.
macro_rules! operation_vocabulary {
    (
        $(#[$enum_docs:meta])*
        $name:ident,
        $(#[$answer_docs:meta])*
        $answer:ident,
        $vocabulary:ident,
        { $( $variant:ident, $discriminant:literal, $method:ident, $params:ty, $result:ty; )* }
    ) => {
        $(#[$enum_docs])*
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(tag = "kind")]
        pub enum $name {
            $( #[serde(rename = $discriminant)] $variant($params), )*
        }

        $(#[$answer_docs])*
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[serde(tag = "kind", content = "result", deny_unknown_fields)]
        pub enum $answer {
            $( #[serde(rename = $discriminant)] $variant($result), )*
        }

        impl $answer {
            #[must_use]
            pub fn kind(&self) -> &'static str {
                match self { $( Self::$variant(_) => $discriminant, )* }
            }
        }

        impl $name {
            #[must_use]
            pub fn kind(&self) -> &'static str {
                match self { $( Self::$variant(_) => $discriminant, )* }
            }

            /// The canonical engine method this operation dispatches to.
            #[must_use]
            pub fn method(&self) -> &'static str {
                match self { $( Self::$variant(_) => $method, )* }
            }

            /// The canonical parameters of [`Self::method`].
            pub fn params(&self) -> Result<Value, AdapterRefusal> {
                match self {
                    $( Self::$variant(params) => serde_json::to_value(params), )*
                }
                .map_err(|_| AdapterRefusal::of(RefusalReason::EncodingFailed))
            }

            /// Require re-encoding equality to reject foreign payloads that an
            /// all-optional canonical result could otherwise accept.
            pub fn answered(&self, result: Value) -> Result<$answer, AdapterRefusal> {
                let uncanonical = || AdapterRefusal::of(RefusalReason::UncanonicalResult);
                let (answer, canonical) = match self {
                    $(
                        Self::$variant(_) => {
                            let typed = <$result as Deserialize>::deserialize(&result)
                                .map_err(|_| uncanonical())?;
                            let canonical =
                                serde_json::to_value(&typed).map_err(|_| uncanonical())?;
                            ($answer::$variant(typed), canonical)
                        }
                    )*
                };
                if canonical == result {
                    Ok(answer)
                } else {
                    Err(uncanonical())
                }
            }
        }

        /// Every operation of this vocabulary as a (discriminant, canonical
        /// method) pair, in declaration order.
        pub const $vocabulary: &[(&str, &str)] = &[$( ($discriminant, $method), )*];
    };
}

operation_vocabulary! {
    /// The operations a read session admits: what Notes, Glossary, Relations and
    /// live Explorations need, and nothing that writes or that belongs to the
    /// review and asset side stores.
    ReadOperation,
    /// The canonical result of one admitted read operation.
    ///
    /// `nodeFromId` and `nodeFromKey` carry an absent node as a null result,
    /// because their canonical lookup admits absence; no other answer does.
    ReadAnswer,
    READ_VOCABULARY,
    {
        Status, "status", METHOD_STATUS, StateOnlyParams, StatusInfo;
        IndexedFiles, "indexedFiles", METHOD_INDEXED_FILES, StateOnlyParams, IndexedFilesResult;
        SearchNodes, "searchNodes", METHOD_SEARCH_NODES, SearchNodesParams, SearchNodesResult;
        SearchNodeContent, "searchNodeContent", METHOD_SEARCH_NODE_CONTENT,
            SearchNodeContentParams, SearchNodeContentResult;
        NodeFromId, "nodeFromId", METHOD_NODE_FROM_ID, NodeFromIdParams, Option<NodeRecord>;
        NodeFromKey, "nodeFromKey", METHOD_NODE_FROM_KEY, NodeFromKeyParams, Option<NodeRecord>;
        ReadNodeSource, "readNodeSource", METHOD_READ_NODE_SOURCE,
            ReadNodeSourceParams, ReadNodeSourceResult;
        ListGlossaryTerms, "listGlossaryTerms", METHOD_LIST_GLOSSARY_TERMS,
            ListGlossaryTermsParams, ListGlossaryTermsResult;
        SearchGlossary, "searchGlossary", METHOD_SEARCH_GLOSSARY,
            SearchGlossaryParams, SearchGlossaryResult;
        GlossaryTerm, "glossaryTerm", METHOD_GLOSSARY_TERM, GlossaryTermParams, GlossaryTermResult;
        Backlinks, "backlinks", METHOD_BACKLINKS, BacklinksParams, BacklinksResult;
        ForwardLinks, "forwardLinks", METHOD_FORWARD_LINKS, ForwardLinksParams, ForwardLinksResult;
        Explore, "explore", METHOD_EXPLORE, ExploreParams, ExploreResult;
    }
}

operation_vocabulary! {
    /// The operations a maintenance session admits: indexing a fixture once, and
    /// updating or removing the index of one named file.
    MaintenanceOperation,
    /// The canonical result of one admitted maintenance operation.
    MaintenanceAnswer,
    MAINTENANCE_VOCABULARY,
    {
        Index, "index", METHOD_INDEX, StateOnlyParams, IndexStats;
        IndexFile, "indexFile", METHOD_INDEX_FILE, IndexFileParams, IndexFileResult;
    }
}

/// Either capability's answer. The two vocabularies share no discriminant, so an
/// answer names exactly one operation of exactly one capability.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OperationAnswer {
    /// A reading answer is much the larger of the two, so it is held behind a
    /// pointer rather than widening every answer to its size.
    Read(Box<ReadAnswer>),
    Maintenance(MaintenanceAnswer),
}

impl OperationAnswer {
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Read(answer) => answer.kind(),
            Self::Maintenance(answer) => answer.kind(),
        }
    }
}

impl From<ReadAnswer> for OperationAnswer {
    fn from(answer: ReadAnswer) -> Self {
        Self::Read(Box::new(answer))
    }
}

impl From<MaintenanceAnswer> for OperationAnswer {
    fn from(answer: MaintenanceAnswer) -> Self {
        Self::Maintenance(answer)
    }
}

impl ReadOperation {
    /// The bound this request violates, if any.
    ///
    /// A page or relation count of zero is a violation as much as an oversized
    /// one: the canonical clamp would answer one entry, which is not what the
    /// caller asked for.
    #[must_use]
    pub fn violated_bound(&self) -> Option<AdapterBound> {
        match self {
            Self::Status(_)
            | Self::IndexedFiles(_)
            | Self::NodeFromId(_)
            | Self::NodeFromKey(_)
            | Self::GlossaryTerm(_) => None,
            Self::SearchNodes(params) => page_bound(params.limit),
            Self::SearchNodeContent(params) => page_bound(params.limit),
            Self::ListGlossaryTerms(params) => page_bound(params.limit),
            Self::SearchGlossary(params) => page_bound(params.limit),
            Self::Backlinks(params) => relation_bound(params.limit),
            Self::ForwardLinks(params) => relation_bound(params.limit),
            Self::Explore(params) => relation_bound(params.limit),
            Self::ReadNodeSource(params) => source_bound(params),
        }
    }
}

impl MaintenanceOperation {
    /// The bound this request violates, if any.
    #[must_use]
    pub fn violated_bound(&self) -> Option<AdapterBound> {
        match self {
            Self::Index(_) => None,
            Self::IndexFile(params) => (params.file_path.len() > ADAPTER_LIMITS.max_path_bytes)
                .then_some(AdapterBound::PathBytes),
        }
    }
}

fn page_bound(limit: usize) -> Option<AdapterBound> {
    (limit == 0 || limit > ADAPTER_LIMITS.max_page_entries).then_some(AdapterBound::PageEntries)
}

fn relation_bound(limit: usize) -> Option<AdapterBound> {
    (limit == 0 || limit > ADAPTER_LIMITS.max_relation_entries)
        .then_some(AdapterBound::RelationEntries)
}

fn source_bound(params: &ReadNodeSourceParams) -> Option<AdapterBound> {
    let context = [params.context_before, params.context_after]
        .into_iter()
        .flatten()
        .any(|lines| lines > ADAPTER_LIMITS.max_context_lines);
    if context {
        return Some(AdapterBound::ContextLines);
    }
    let lines = params
        .max_lines
        .is_some_and(|lines| lines == 0 || lines > ADAPTER_LIMITS.max_note_source_lines);
    lines.then_some(AdapterBound::NoteSourceLines)
}

/// A request to open one session against one root, database and binding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenRequest {
    pub version: u32,
    pub capability: AdapterCapability,
    pub binding: GenerationBinding,
    pub context: SessionContext,
}

/// A read request against one open session.
///
/// The binding is carried so a caller that has confused two sessions is refused
/// rather than answered: it is compared against the binding the handle retains,
/// never substituted for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadRequest {
    pub version: u32,
    pub handle: SessionIdentity,
    pub binding: GenerationBinding,
    pub operation: ReadOperation,
}

/// A maintenance request against one open session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaintenanceRequest {
    pub version: u32,
    pub handle: SessionIdentity,
    pub binding: GenerationBinding,
    pub operation: MaintenanceOperation,
}

/// A request to retire one session. Repeating it is defined, not an error.
///
/// A live session compares the binding before retiring anything, so a caller
/// that has confused two sessions is refused and the live handle stays usable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CloseRequest {
    pub version: u32,
    pub handle: SessionIdentity,
    pub binding: GenerationBinding,
}

/// One answer to one request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdapterResponse {
    pub version: u32,
    #[serde(flatten)]
    pub outcome: AdapterOutcome,
}

impl AdapterResponse {
    #[must_use]
    pub fn new(outcome: AdapterOutcome) -> Self {
        Self {
            version: ADAPTER_PROTOCOL_VERSION,
            outcome,
        }
    }
}

impl From<AdapterRefusal> for AdapterResponse {
    fn from(refusal: AdapterRefusal) -> Self {
        Self::new(AdapterOutcome::Refused(refusal))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum AdapterOutcome {
    Contract(AdapterContract),
    Opened(OpenedSession),
    Answered(AnsweredOperation),
    Closed(ClosedSession),
    Refused(AdapterRefusal),
}

/// What the native library admits, reported so the Kotlin mirror can refuse a
/// library that does not match it instead of discovering the difference one
/// failed request at a time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterContract {
    pub limits: AdapterLimits,
    pub read_operations: Vec<String>,
    pub maintenance_operations: Vec<String>,
}

impl Default for AdapterContract {
    fn default() -> Self {
        Self {
            limits: ADAPTER_LIMITS,
            read_operations: discriminants(READ_VOCABULARY),
            maintenance_operations: discriminants(MAINTENANCE_VOCABULARY),
        }
    }
}

fn discriminants(vocabulary: &[(&str, &str)]) -> Vec<String> {
    vocabulary
        .iter()
        .map(|(discriminant, _)| (*discriminant).to_owned())
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenedSession {
    pub handle: SessionIdentity,
    pub capability: AdapterCapability,
    pub binding: GenerationBinding,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnsweredOperation {
    pub handle: SessionIdentity,
    pub binding: GenerationBinding,
    /// The canonical engine result, under the discriminant of the operation that
    /// produced it.
    pub answer: OperationAnswer,
}

impl AnsweredOperation {
    #[must_use]
    pub fn kind(&self) -> &'static str {
        self.answer.kind()
    }
}

/// The report of one retirement.
///
/// A retiring call reports the binding it retired, which is what lets a caller
/// confirm it closed the session it meant to. A repeat reports no binding: the
/// table keeps no record of a session it has already released.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClosedSession {
    pub handle: SessionIdentity,
    /// True on the call that retired the session, false on a later repeat.
    pub retired: bool,
    pub binding: Option<GenerationBinding>,
}

/// A refused request.
///
/// Nothing here derives from the request: the reason and bound are closed
/// vocabularies, and an engine refusal contributes only its numeric code and
/// classified kind. Engine prose is dropped because it quotes the node keys,
/// paths and queries the caller supplied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdapterRefusal {
    pub reason: RefusalReason,
    #[serde(default)]
    pub bound: Option<AdapterBound>,
    #[serde(default)]
    pub engine: Option<EngineRefusal>,
}

impl AdapterRefusal {
    #[must_use]
    pub fn of(reason: RefusalReason) -> Self {
        Self {
            reason,
            bound: None,
            engine: None,
        }
    }

    #[must_use]
    pub fn bounded(bound: AdapterBound) -> Self {
        Self {
            reason: RefusalReason::OutOfBounds,
            bound: Some(bound),
            engine: None,
        }
    }

    #[must_use]
    pub fn engine(reason: RefusalReason, engine: EngineRefusal) -> Self {
        Self {
            reason,
            bound: None,
            engine: Some(engine),
        }
    }
}

impl fmt::Display for AdapterRefusal {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.reason.summary())?;
        if let Some(bound) = self.bound {
            write!(formatter, " (bound {})", bound.summary())?;
        }
        if let Some(engine) = self.engine {
            write!(formatter, " (engine code {})", engine.code)?;
        }
        Ok(())
    }
}

/// The classified numbers of an engine refusal, without its prose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineRefusal {
    pub code: i64,
    #[serde(default)]
    pub kind: Option<JsonRpcErrorKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RefusalReason {
    /// The request named a protocol version this library does not speak.
    UnsupportedVersion,
    /// The request was not a decodable document of this contract.
    MalformedRequest,
    /// The request named an operation outside the closed vocabulary.
    UnknownOperation,
    /// The request violated a declared bound.
    OutOfBounds,
    /// The root or database the session named could not be opened as given.
    InvalidContext,
    /// The request named a binding other than the one the handle retains.
    BindingMismatch,
    /// The operation belongs to the other capability.
    CapabilityMismatch,
    /// No session was ever allocated under that identity.
    UnknownHandle,
    /// The session under that identity has been retired.
    RetiredHandle,
    /// Every session slot is occupied.
    SessionsExhausted,
    /// No further session identity can be allocated.
    IdentitiesExhausted,
    /// The engine declined the operation.
    EngineRefused,
    /// The engine could not carry the operation out.
    EngineFailed,
    /// The engine result was not the canonical result of that operation.
    UncanonicalResult,
    /// A request or answer could not be encoded.
    EncodingFailed,
    /// Native code panicked and the boundary contained it.
    Panicked,
}

impl RefusalReason {
    /// A fixed sentence for this reason. It is chosen here rather than composed
    /// from the request, so no diagnostic can quote caller input.
    #[must_use]
    pub fn summary(self) -> &'static str {
        match self {
            Self::UnsupportedVersion => "the request names an unsupported protocol version",
            Self::MalformedRequest => "the request is not a document of this contract",
            Self::UnknownOperation => "the request names an operation outside the vocabulary",
            Self::OutOfBounds => "the request violates a declared bound",
            Self::InvalidContext => "the session root or database cannot be opened as given",
            Self::BindingMismatch => "the request names a binding the session does not hold",
            Self::CapabilityMismatch => "the operation belongs to the other capability",
            Self::UnknownHandle => "no session holds that identity",
            Self::RetiredHandle => "that session has been retired",
            Self::SessionsExhausted => "every session slot is occupied",
            Self::IdentitiesExhausted => "no further session identity can be allocated",
            Self::EngineRefused => "the engine declined the operation",
            Self::EngineFailed => "the engine could not carry the operation out",
            Self::UncanonicalResult => "the result is not the canonical result of that operation",
            Self::EncodingFailed => "the answer could not be encoded",
            Self::Panicked => "native code failed and the boundary contained it",
        }
    }
}

impl fmt::Display for RefusalReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.summary())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AdapterBound {
    RequestBytes,
    ResponseBytes,
    PageEntries,
    RelationEntries,
    NoteSourceLines,
    ContextLines,
    PathBytes,
    OpenSessions,
    QueuedRequests,
}

impl AdapterBound {
    #[must_use]
    pub fn summary(self) -> &'static str {
        match self {
            Self::RequestBytes => "request bytes",
            Self::ResponseBytes => "response bytes",
            Self::PageEntries => "page entries",
            Self::RelationEntries => "relation entries",
            Self::NoteSourceLines => "note source lines",
            Self::ContextLines => "context lines",
            Self::PathBytes => "path bytes",
            Self::OpenSessions => "open sessions",
            Self::QueuedRequests => "queued requests",
        }
    }
}

impl fmt::Display for AdapterBound {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.summary())
    }
}

/// The one answer this contract can produce without an encoder.
pub const ENCODE_FAILURE: &[u8] =
    br#"{"version":1,"outcome":"refused","reason":"encoding-failed","bound":null,"engine":null}"#;

/// What writing one answer under the response bound produced. `held` is the
/// number of bytes the encoder had actually reserved when it stopped, which is
/// what an oversized-answer test reads to see that the bound governs storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResponseEncoding {
    Written(Vec<u8>),
    Oversized { held: usize },
    Failed { held: usize },
}

/// Writes one answer directly into storage bounded by
/// [`AdapterLimits::max_response_bytes`], so an answer that does not fit is
/// stopped while it is written rather than after it has been built in full. An
/// oversized answer is never truncated and never partially delivered.
#[must_use]
pub fn write_response(response: &AdapterResponse) -> ResponseEncoding {
    write_bounded(response, ADAPTER_LIMITS.max_response_bytes)
}

/// Writes one document into storage bounded by `limit`.
#[must_use]
pub fn write_bounded<T: Serialize>(document: &T, limit: usize) -> ResponseEncoding {
    let mut writer = BoundedWriter {
        bytes: Vec::new(),
        limit,
    };
    match serde_json::to_writer(&mut writer, document) {
        Ok(()) => ResponseEncoding::Written(writer.bytes),
        Err(failure) if failure.is_io() => ResponseEncoding::Oversized {
            held: writer.bytes.capacity(),
        },
        Err(_) => ResponseEncoding::Failed {
            held: writer.bytes.capacity(),
        },
    }
}

/// Encodes one answer, or a refusal in its place when the answer does not fit
/// the declared response bound.
#[must_use]
pub fn encode_response(response: &AdapterResponse) -> Vec<u8> {
    match write_response(response) {
        ResponseEncoding::Written(bytes) => bytes,
        ResponseEncoding::Oversized { .. } => {
            encode_refusal(AdapterRefusal::bounded(AdapterBound::ResponseBytes))
        }
        ResponseEncoding::Failed { .. } => {
            encode_refusal(AdapterRefusal::of(RefusalReason::EncodingFailed))
        }
    }
}

/// Storage that accepts at most one declared bound of bytes and reserves no more
/// than that, so a refused answer cannot be an unbounded allocation.
struct BoundedWriter {
    bytes: Vec<u8>,
    limit: usize,
}

impl Write for BoundedWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let filled = self.bytes.len();
        if buffer.len() > self.limit - filled {
            return Err(io::Error::from(io::ErrorKind::WriteZero));
        }
        // Grow geometrically but never past the bound, so no reservation of this
        // storage can exceed it.
        if buffer.len() > self.bytes.capacity() - filled {
            let doubled = self.bytes.capacity().max(4_096).saturating_mul(2);
            let wanted = doubled.min(self.limit).max(filled + buffer.len());
            self.bytes.reserve_exact(wanted - filled);
        }
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[must_use]
pub fn encode_refusal(refusal: AdapterRefusal) -> Vec<u8> {
    serde_json::to_vec(&AdapterResponse::from(refusal)).unwrap_or_else(|_| ENCODE_FAILURE.to_vec())
}

pub fn decode_open_request(bytes: &[u8]) -> Result<OpenRequest, AdapterRefusal> {
    decode(&versioned(bytes)?)
}

pub fn decode_read_request(bytes: &[u8]) -> Result<ReadRequest, AdapterRefusal> {
    let document = versioned(bytes)?;
    admitted(&document, READ_VOCABULARY)?;
    decode(&document)
}

pub fn decode_maintenance_request(bytes: &[u8]) -> Result<MaintenanceRequest, AdapterRefusal> {
    let document = versioned(bytes)?;
    admitted(&document, MAINTENANCE_VOCABULARY)?;
    decode(&document)
}

pub fn decode_close_request(bytes: &[u8]) -> Result<CloseRequest, AdapterRefusal> {
    decode(&versioned(bytes)?)
}

/// Validate request length, unique decoded keys and protocol version.
fn versioned(bytes: &[u8]) -> Result<Value, AdapterRefusal> {
    if bytes.len() > ADAPTER_LIMITS.max_request_bytes {
        return Err(AdapterRefusal::bounded(AdapterBound::RequestBytes));
    }
    unambiguous(bytes)?;
    let document: Value = serde_json::from_slice(bytes)
        .map_err(|_| AdapterRefusal::of(RefusalReason::MalformedRequest))?;
    match document.get("version").and_then(Value::as_u64) {
        Some(version) if version == u64::from(ADAPTER_PROTOCOL_VERSION) => Ok(document),
        Some(_) => Err(AdapterRefusal::of(RefusalReason::UnsupportedVersion)),
        None => Err(AdapterRefusal::of(RefusalReason::MalformedRequest)),
    }
}

/// Reject duplicate keys before parsing into a map that would overwrite them.
fn unambiguous(bytes: &[u8]) -> Result<(), AdapterRefusal> {
    let mut parser = serde_json::Deserializer::from_slice(bytes);
    UniqueMembers::deserialize(&mut parser)
        .map(|_| ())
        .map_err(|_| AdapterRefusal::of(RefusalReason::MalformedRequest))
}

/// Visit every object with decoded keys, discarding values after validation.
struct UniqueMembers;

impl<'de> Deserialize<'de> for UniqueMembers {
    fn deserialize<D: Deserializer<'de>>(parser: D) -> Result<Self, D::Error> {
        parser.deserialize_any(UniqueMembers)
    }
}

impl<'de> Visitor<'de> for UniqueMembers {
    type Value = Self;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a document whose objects each name a member once")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut members: A) -> Result<Self, A::Error> {
        let mut named = BTreeSet::new();
        while let Some(name) = members.next_key::<String>()? {
            if !named.insert(name) {
                return Err(de::Error::custom("one member named twice"));
            }
            members.next_value::<Self>()?;
        }
        Ok(self)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut entries: A) -> Result<Self, A::Error> {
        while entries.next_element::<Self>()?.is_some() {}
        Ok(self)
    }

    fn visit_bool<E: de::Error>(self, _: bool) -> Result<Self, E> {
        Ok(self)
    }

    fn visit_i64<E: de::Error>(self, _: i64) -> Result<Self, E> {
        Ok(self)
    }

    fn visit_u64<E: de::Error>(self, _: u64) -> Result<Self, E> {
        Ok(self)
    }

    fn visit_f64<E: de::Error>(self, _: f64) -> Result<Self, E> {
        Ok(self)
    }

    fn visit_str<E: de::Error>(self, _: &str) -> Result<Self, E> {
        Ok(self)
    }

    fn visit_unit<E: de::Error>(self) -> Result<Self, E> {
        Ok(self)
    }
}

/// Refuses an operation outside the capability's vocabulary as unknown rather
/// than as malformed, so a caller built against a newer library learns which of
/// the two it is. An operation the vocabulary does admit is left to typed
/// decoding, which is what validates its payload.
fn admitted(document: &Value, vocabulary: &[(&str, &str)]) -> Result<(), AdapterRefusal> {
    let Some(kind) = document
        .get("operation")
        .and_then(|operation| operation.get("kind"))
        .and_then(Value::as_str)
    else {
        return Ok(());
    };
    if vocabulary
        .iter()
        .any(|(discriminant, _)| *discriminant == kind)
    {
        Ok(())
    } else {
        Err(AdapterRefusal::of(RefusalReason::UnknownOperation))
    }
}

fn decode<T: DeserializeOwned>(document: &Value) -> Result<T, AdapterRefusal> {
    T::deserialize(document).map_err(|_| AdapterRefusal::of(RefusalReason::MalformedRequest))
}
