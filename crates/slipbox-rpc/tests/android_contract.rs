//! The Android adapter's wire contract, checked against the canonical operation
//! classification and against the JSON fixtures the Kotlin mirror shares.

use std::fs;
use std::path::{Path, PathBuf};

use serde::ser::{Error as _, SerializeSeq as _};
use serde::{Serialize, Serializer};
use serde_json::{Value, json};
use slipbox_core::{
    BacklinksParams, ExplorationLens, ExploreParams, ForwardLinksParams, GenerationBinding,
    GenerationId, GlossaryTermParams, IndexFileParams, IndexedFilesResult, ListGlossaryTermsParams,
    NodeFromIdParams, NodeFromKeyParams, ReadNodeSourceParams, SearchGlossaryParams,
    SearchNodeContentParams, SearchNodesParams, SearchNodesSort, SourceId,
};
use slipbox_rpc::android::{
    ADAPTER_LIMITS, ADAPTER_PROTOCOL_VERSION, AdapterBound, AdapterCapability, AdapterContract,
    AdapterOutcome, AdapterRefusal, AdapterResponse, AnsweredOperation, CloseRequest,
    ENCODE_FAILURE, EngineRefusal, MAINTENANCE_VOCABULARY, MaintenanceAnswer, MaintenanceOperation,
    MaintenanceRequest, OperationAnswer, READ_VOCABULARY, ReadAnswer, ReadOperation, ReadRequest,
    RefusalReason, ResponseEncoding, SessionContext, StateOnlyParams, decode_close_request,
    decode_maintenance_request, decode_open_request, decode_read_request, encode_response,
    write_bounded, write_response,
};
use slipbox_rpc::{
    JsonRpcErrorKind, OperationFamily, OperationMutation, is_read_only,
    operation_descriptor_by_method,
};

const FIXTURE_SOURCE: &str = "0102030405060708090a0b0c0d0e0f10";

fn binding(generation: &str) -> GenerationBinding {
    GenerationBinding::new(
        SourceId::parse(FIXTURE_SOURCE).expect("the fixture source identity parses"),
        GenerationId::parse(generation).expect("the fixture generation parses"),
    )
}

fn read_request(operation: ReadOperation) -> ReadRequest {
    ReadRequest {
        version: ADAPTER_PROTOCOL_VERSION,
        handle: 7,
        binding: binding("fixture-01"),
        operation,
    }
}

fn encoded(request: &ReadRequest) -> Vec<u8> {
    serde_json::to_vec(request).expect("a read request encodes")
}

fn refuse(bytes: &[u8]) -> AdapterRefusal {
    decode_read_request(bytes).expect_err("the document is refused")
}

fn answered(answer: OperationAnswer) -> AdapterResponse {
    AdapterResponse::new(AdapterOutcome::Answered(AnsweredOperation {
        handle: 7,
        binding: binding("fixture-01"),
        answer,
    }))
}

fn fixture_root(tree: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../android/app/src")
        .join(tree)
        .join("resources/engine")
}

fn fixtures(name: &str) -> Value {
    let unit =
        fs::read(fixture_root("test").join(name)).expect("the unit-test fixture is readable");
    let device = fs::read(fixture_root("androidTest").join(name))
        .expect("the instrumentation fixture is readable");
    assert_eq!(
        unit, device,
        "{name} differs between the unit-test and instrumentation resource trees"
    );
    serde_json::from_slice(&unit).expect("the fixture is a JSON document")
}

fn fixture(name: &str, key: &str) -> Value {
    fixtures(name)
        .get(key)
        .unwrap_or_else(|| panic!("{name} declares no {key} document"))
        .clone()
}

fn every_read_operation() -> Vec<ReadOperation> {
    vec![
        ReadOperation::Status(StateOnlyParams {}),
        ReadOperation::IndexedFiles(StateOnlyParams {}),
        ReadOperation::SearchNodes(SearchNodesParams {
            query: "omega".to_owned(),
            limit: 25,
            sort: Some(SearchNodesSort::Title),
        }),
        ReadOperation::SearchNodeContent(SearchNodeContentParams {
            query: "omega".to_owned(),
            limit: 25,
        }),
        ReadOperation::NodeFromId(NodeFromIdParams {
            id: "11111111-2222-3333-4444-555555555555".to_owned(),
        }),
        ReadOperation::NodeFromKey(NodeFromKeyParams {
            node_key: "heading:alpha.org:3".to_owned(),
        }),
        ReadOperation::ReadNodeSource(ReadNodeSourceParams {
            node_key: "heading:alpha.org:3".to_owned(),
            context_before: Some(0),
            context_after: Some(0),
            max_lines: Some(ADAPTER_LIMITS.max_note_source_lines),
        }),
        ReadOperation::ListGlossaryTerms(ListGlossaryTermsParams {
            limit: 50,
            after: None,
        }),
        ReadOperation::SearchGlossary(SearchGlossaryParams {
            query: "omega".to_owned(),
            limit: 50,
        }),
        ReadOperation::GlossaryTerm(GlossaryTermParams {
            node_key: "heading:riemann.org:2".to_owned(),
        }),
        ReadOperation::Backlinks(BacklinksParams {
            node_key: "heading:alpha.org:3".to_owned(),
            limit: 200,
            unique: false,
        }),
        ReadOperation::ForwardLinks(ForwardLinksParams {
            node_key: "heading:alpha.org:3".to_owned(),
            limit: 200,
            unique: false,
        }),
        ReadOperation::Explore(ExploreParams {
            node_key: "heading:alpha.org:3".to_owned(),
            lens: ExplorationLens::Structure,
            limit: 200,
            unique: true,
        }),
    ]
}

fn every_maintenance_operation() -> Vec<MaintenanceOperation> {
    vec![
        MaintenanceOperation::Index(StateOnlyParams {}),
        MaintenanceOperation::IndexFile(IndexFileParams {
            file_path: "alpha.org".to_owned(),
        }),
    ]
}

#[test]
fn every_read_operation_is_canonically_read_only() {
    for (kind, method) in READ_VOCABULARY {
        let descriptor = operation_descriptor_by_method(method)
            .unwrap_or_else(|| panic!("{kind} names the unclassified method {method}"));
        assert_eq!(
            descriptor.mutation,
            OperationMutation::ReadOnly,
            "{kind} names {method}, which is not canonically read-only"
        );
        assert!(
            is_read_only(method),
            "{method} is not canonically read-only"
        );
    }
}

#[test]
fn the_read_inventory_serves_the_reading_surfaces_and_no_side_store() {
    let families: Vec<OperationFamily> = READ_VOCABULARY
        .iter()
        .map(|(_, method)| {
            operation_descriptor_by_method(method)
                .expect("a classified method")
                .family
        })
        .collect();

    for required in [
        OperationFamily::System,
        OperationFamily::Files,
        OperationFamily::Notes,
        OperationFamily::Glossary,
        OperationFamily::Relations,
        OperationFamily::Exploration,
    ] {
        assert!(
            families.contains(&required),
            "the read inventory serves nothing of {required:?}"
        );
    }
    for refused in [
        OperationFamily::Reviews,
        OperationFamily::Assets,
        OperationFamily::Capture,
        OperationFamily::StructuralEdit,
        OperationFamily::LinkRewrite,
        OperationFamily::Diagnostics,
    ] {
        assert!(
            !families.contains(&refused),
            "the read inventory reaches into {refused:?}"
        );
    }
}

#[test]
fn every_maintenance_operation_refreshes_only_the_derived_index() {
    for (kind, method) in MAINTENANCE_VOCABULARY {
        let descriptor = operation_descriptor_by_method(method)
            .unwrap_or_else(|| panic!("{kind} names the unclassified method {method}"));
        assert_eq!(
            descriptor.mutation,
            OperationMutation::DerivedIndex,
            "{kind} names {method}, which changes more than the derived index"
        );
        assert!(
            !is_read_only(method),
            "{method} is classified read-only, so it does not belong to maintenance"
        );
    }
}

#[test]
fn the_two_capabilities_admit_disjoint_vocabularies() {
    for (read_kind, read_method) in READ_VOCABULARY {
        for (maintenance_kind, maintenance_method) in MAINTENANCE_VOCABULARY {
            assert_ne!(read_kind, maintenance_kind);
            assert_ne!(read_method, maintenance_method);
        }
    }
    assert_eq!(
        AdapterCapability::Read.vocabulary(),
        READ_VOCABULARY,
        "the read capability reports the wrong vocabulary"
    );
    assert_eq!(
        AdapterCapability::Maintenance.vocabulary(),
        MAINTENANCE_VOCABULARY,
        "the maintenance capability reports the wrong vocabulary"
    );
}

#[test]
fn every_operation_reports_the_pair_its_vocabulary_declares() {
    let read: Vec<(&str, &str)> = every_read_operation()
        .iter()
        .map(|operation| (operation.kind(), operation.method()))
        .collect();
    assert_eq!(read.as_slice(), READ_VOCABULARY);

    let maintenance: Vec<(&str, &str)> = every_maintenance_operation()
        .iter()
        .map(|operation| (operation.kind(), operation.method()))
        .collect();
    assert_eq!(maintenance.as_slice(), MAINTENANCE_VOCABULARY);
}

#[test]
fn every_operation_carries_the_canonical_payload_of_its_method() {
    for operation in every_read_operation() {
        let params = operation.params().expect("canonical parameters encode");
        let mut document =
            serde_json::to_value(&operation).expect("an operation encodes as a document");
        let object = document
            .as_object_mut()
            .expect("an operation encodes as an object");
        assert_eq!(
            object.remove("kind").expect("the discriminant is present"),
            Value::String(operation.kind().to_owned())
        );
        assert_eq!(
            document,
            params,
            "{} carries something other than its canonical payload",
            operation.kind()
        );
    }
}

#[test]
fn the_reported_contract_is_the_one_the_kotlin_mirror_shares() {
    let reported = AdapterResponse::new(AdapterOutcome::Contract(AdapterContract::default()));
    assert_eq!(
        serde_json::to_value(&reported).expect("the contract encodes"),
        fixture("responses.json", "contract")
    );
}

#[test]
fn every_shared_response_document_decodes_to_the_answer_it_describes() {
    let documents = fixtures("responses.json");
    let documents = documents
        .as_object()
        .expect("the shared answers are a JSON object");
    let mut outcomes = Vec::new();
    for (key, document) in documents {
        if key.starts_with("bad_") {
            continue;
        }
        let response: AdapterResponse = serde_json::from_value(document.clone())
            .unwrap_or_else(|failure| panic!("the {key} document does not decode: {failure}"));
        assert_eq!(response.version, ADAPTER_PROTOCOL_VERSION, "{key}");
        assert_eq!(
            serde_json::to_value(&response).expect("a response re-encodes"),
            *document,
            "the {key} document does not survive a round trip"
        );
        outcomes.push(
            document["outcome"]
                .as_str()
                .unwrap_or_else(|| panic!("the {key} document names no outcome"))
                .to_owned(),
        );
    }
    for required in ["contract", "opened", "answered", "closed", "refused"] {
        assert!(
            outcomes.iter().any(|outcome| outcome == required),
            "the shared answers describe no {required} outcome"
        );
    }

    let refused = fixture("responses.json", "refused");
    let response: AdapterResponse = serde_json::from_value(refused).expect("a refusal decodes");
    assert_eq!(
        response.outcome,
        AdapterOutcome::Refused(AdapterRefusal::engine(
            RefusalReason::EngineRefused,
            EngineRefusal {
                code: -32602,
                kind: Some(JsonRpcErrorKind::InvalidParams),
            },
        ))
    );
}

#[test]
fn every_shared_request_document_decodes_to_the_request_it_describes() {
    let open = decode_open_request(&document_bytes("requests.json", "open_read"))
        .expect("the shared open request decodes");
    assert_eq!(open.capability, AdapterCapability::Read);
    assert_eq!(open.binding, binding("fixture-01"));
    assert_eq!(
        open.context,
        SessionContext {
            root: "/data/fixture/root".to_owned(),
            database: "/data/fixture/slipbox.db".to_owned(),
        }
    );

    let maintenance_open =
        decode_open_request(&document_bytes("requests.json", "open_maintenance"))
            .expect("the shared maintenance open request decodes");
    assert_eq!(maintenance_open.capability, AdapterCapability::Maintenance);
    assert_eq!(maintenance_open.binding, binding("fixture-02"));

    let read = decode_read_request(&document_bytes("requests.json", "read_search_nodes"))
        .expect("the shared read request decodes");
    assert_eq!(
        read.operation,
        ReadOperation::SearchNodes(SearchNodesParams {
            query: "Ωμέγα théorie 漢字".to_owned(),
            limit: 25,
            sort: Some(SearchNodesSort::Title),
        })
    );
    assert_eq!(read.handle, 7);
    assert_eq!(read.binding, binding("fixture-01"));

    let source = decode_read_request(&document_bytes("requests.json", "read_source"))
        .expect("the shared source request decodes");
    assert_eq!(
        source.operation,
        ReadOperation::ReadNodeSource(ReadNodeSourceParams {
            node_key: "heading:alpha.org:3".to_owned(),
            context_before: Some(0),
            context_after: Some(0),
            max_lines: Some(ADAPTER_LIMITS.max_note_source_lines),
        })
    );

    let maintenance =
        decode_maintenance_request(&document_bytes("requests.json", "maintain_index_file"))
            .expect("the shared maintenance request decodes");
    assert_eq!(
        maintenance.operation,
        MaintenanceOperation::IndexFile(IndexFileParams {
            file_path: "alpha.org".to_owned(),
        })
    );
    assert_eq!(maintenance.binding, binding("fixture-02"));

    let close = decode_close_request(&document_bytes("requests.json", "close"))
        .expect("the shared close request decodes");
    assert_eq!(
        close,
        CloseRequest {
            version: 1,
            handle: 7,
            binding: binding("fixture-01"),
        }
    );
    let confused = decode_close_request(&document_bytes("requests.json", "close_other_binding"))
        .expect("the shared confused close request decodes");
    assert_eq!(confused.handle, close.handle);
    assert_ne!(confused.binding, close.binding);
}

#[test]
fn the_shared_requests_name_every_admitted_operation() {
    let documents = fixtures("requests.json");
    let documents = documents
        .as_object()
        .expect("the shared requests are a JSON object");
    let mut read = Vec::new();
    let mut maintenance = Vec::new();
    for (key, document) in documents {
        if key.starts_with("bad_") || document.get("operation").is_none() {
            continue;
        }
        let bytes = serde_json::to_vec(document).expect("a shared document encodes");
        if let Ok(request) = decode_read_request(&bytes) {
            assert_eq!(
                serde_json::to_value(&request).expect("a read request re-encodes"),
                *document,
                "the {key} request does not survive a round trip"
            );
            read.push(request.operation.kind());
        } else {
            let request = decode_maintenance_request(&bytes)
                .unwrap_or_else(|_| panic!("the {key} request decodes as neither capability"));
            assert_eq!(
                serde_json::to_value(&request).expect("a maintenance request re-encodes"),
                *document,
                "the {key} request does not survive a round trip"
            );
            maintenance.push(request.operation.kind());
        }
    }
    read.sort_unstable();
    maintenance.sort_unstable();
    assert_eq!(read, sorted_discriminants(READ_VOCABULARY));
    assert_eq!(maintenance, sorted_discriminants(MAINTENANCE_VOCABULARY));
}

fn sorted_discriminants(vocabulary: &[(&'static str, &str)]) -> Vec<&'static str> {
    let mut discriminants: Vec<&'static str> = vocabulary
        .iter()
        .map(|(discriminant, _)| *discriminant)
        .collect();
    discriminants.sort_unstable();
    discriminants
}

#[test]
fn every_shared_request_document_the_mirror_refuses_is_refused_here() {
    for (key, reason) in [
        (
            "bad_read_unknown_operation",
            RefusalReason::UnknownOperation,
        ),
        (
            "bad_read_crossed_operation",
            RefusalReason::UnknownOperation,
        ),
        ("bad_read_without_version", RefusalReason::MalformedRequest),
        (
            "bad_read_unsupported_version",
            RefusalReason::UnsupportedVersion,
        ),
        ("bad_read_extra_field", RefusalReason::MalformedRequest),
        ("bad_read_missing_binding", RefusalReason::MalformedRequest),
    ] {
        assert_eq!(
            refuse(&document_bytes("requests.json", key)),
            AdapterRefusal::of(reason),
            "{key}"
        );
    }
    assert_eq!(
        decode_close_request(&document_bytes(
            "requests.json",
            "bad_close_without_binding"
        ))
        .expect_err("a close request naming no binding is refused"),
        AdapterRefusal::of(RefusalReason::MalformedRequest)
    );
}

fn document_bytes(name: &str, key: &str) -> Vec<u8> {
    serde_json::to_vec(&fixture(name, key)).expect("a shared document encodes")
}

#[test]
fn an_operation_outside_the_vocabulary_is_refused_as_unknown() {
    let unknown = json!({
        "version": 1,
        "handle": 7,
        "binding": { "source": FIXTURE_SOURCE, "generation": "fixture-01" },
        "operation": { "kind": "captureNode", "template": "note" },
    });
    let bytes = serde_json::to_vec(&unknown).expect("a document encodes");
    assert_eq!(
        refuse(&bytes),
        AdapterRefusal::of(RefusalReason::UnknownOperation)
    );

    let crossed = json!({
        "version": 1,
        "handle": 7,
        "binding": { "source": FIXTURE_SOURCE, "generation": "fixture-01" },
        "operation": { "kind": "indexFile", "file_path": "alpha.org" },
    });
    let bytes = serde_json::to_vec(&crossed).expect("a document encodes");
    assert_eq!(
        refuse(&bytes),
        AdapterRefusal::of(RefusalReason::UnknownOperation),
        "a maintenance operation is not a read operation"
    );
    assert_eq!(
        decode_maintenance_request(
            &serde_json::to_vec(&json!({
                "version": 1,
                "handle": 7,
                "binding": { "source": FIXTURE_SOURCE, "generation": "fixture-01" },
                "operation": { "kind": "status" },
            }))
            .expect("a document encodes")
        )
        .expect_err("a read operation is not a maintenance operation"),
        AdapterRefusal::of(RefusalReason::UnknownOperation)
    );
}

#[test]
fn a_version_this_library_does_not_speak_is_refused_as_such() {
    for version in [0, 2, 99] {
        let document = json!({
            "version": version,
            "handle": 7,
            "binding": { "source": FIXTURE_SOURCE, "generation": "fixture-01" },
            "operation": { "kind": "status" },
        });
        let bytes = serde_json::to_vec(&document).expect("a document encodes");
        assert_eq!(
            refuse(&bytes),
            AdapterRefusal::of(RefusalReason::UnsupportedVersion),
            "version {version} was not refused as unsupported"
        );
    }
}

#[test]
fn a_document_this_contract_cannot_read_is_refused_structurally() {
    let malformed = AdapterRefusal::of(RefusalReason::MalformedRequest);

    assert_eq!(refuse(b""), malformed, "an empty request");
    assert_eq!(refuse(b"null"), malformed, "a null request");
    assert_eq!(refuse(b"{"), malformed, "a truncated document");
    assert_eq!(
        refuse(&[0x7b, 0x22, 0xff, 0x22, 0x7d]),
        malformed,
        "invalid UTF-8"
    );
    assert_eq!(
        refuse(br#"{"handle":7,"operation":{"kind":"status"}}"#),
        malformed,
        "a document naming no version"
    );
    assert_eq!(
        refuse(br#"{"version":"1","handle":7,"operation":{"kind":"status"}}"#),
        malformed,
        "a version of the wrong type"
    );

    for document in [
        json!({ "version": 1, "handle": "7", "binding": { "source": FIXTURE_SOURCE, "generation": "fixture-01" }, "operation": { "kind": "status" } }),
        json!({ "version": 1, "handle": 7, "binding": { "source": FIXTURE_SOURCE, "generation": "fixture-01" } }),
        json!({ "version": 1, "handle": 7, "operation": { "kind": "status" } }),
        json!({ "version": 1, "handle": 7, "binding": { "source": "not-a-source", "generation": "fixture-01" }, "operation": { "kind": "status" } }),
        json!({ "version": 1, "handle": 7, "binding": { "source": FIXTURE_SOURCE, "generation": "not a generation" }, "operation": { "kind": "status" } }),
        json!({ "version": 1, "handle": 7, "binding": { "source": FIXTURE_SOURCE, "generation": "fixture-01" }, "operation": { "query": "omega" } }),
        json!({ "version": 1, "handle": 7, "binding": { "source": FIXTURE_SOURCE, "generation": "fixture-01" }, "operation": { "kind": "searchNodes" } }),
        json!({ "version": 1, "handle": 7, "binding": { "source": FIXTURE_SOURCE, "generation": "fixture-01" }, "operation": { "kind": "searchNodes", "query": 7, "limit": 25 } }),
        json!({ "version": 1, "handle": 7, "binding": { "source": FIXTURE_SOURCE, "generation": "fixture-01" }, "operation": { "kind": "status" }, "extra": true }),
    ] {
        let bytes = serde_json::to_vec(&document).expect("a document encodes");
        assert_eq!(refuse(&bytes), malformed, "{document} was not refused");
    }
}

type Route = (
    &'static str,
    String,
    fn(&[u8]) -> Result<(), AdapterRefusal>,
);

fn production_routes() -> Vec<Route> {
    let bound = format!(r#""binding":{{"source":"{FIXTURE_SOURCE}","generation":"fixture-01"}}"#);
    vec![
        (
            "open",
            format!(
                r#"{{"version":1,"capability":"read",{bound},"context":{{"root":"/fixture","database":"/fixture/slipbox.db"}}}}"#
            ),
            |bytes| decode_open_request(bytes).map(|_| ()),
        ),
        (
            "read",
            format!(r#"{{"version":1,"handle":7,{bound},"operation":{{"kind":"status"}}}}"#),
            |bytes| decode_read_request(bytes).map(|_| ()),
        ),
        (
            "maintenance",
            format!(
                r#"{{"version":1,"handle":7,{bound},"operation":{{"kind":"indexFile","file_path":"alpha.org"}}}}"#
            ),
            |bytes| decode_maintenance_request(bytes).map(|_| ()),
        ),
        (
            "close",
            format!(r#"{{"version":1,"handle":7,{bound}}}"#),
            |bytes| decode_close_request(bytes).map(|_| ()),
        ),
    ]
}

const DECLARED_VERSION: &str = r#""version":1"#;
const OTHER_VERSION: &str = r#""version":2"#;

#[test]
fn a_document_naming_one_member_twice_is_refused_rather_than_resolved() {
    let malformed = AdapterRefusal::of(RefusalReason::MalformedRequest);
    let unsupported = AdapterRefusal::of(RefusalReason::UnsupportedVersion);
    let routes = production_routes();

    for (route, canonical, decode) in &routes {
        decode(canonical.as_bytes()).unwrap_or_else(|_| panic!("{route} reads its document"));
        let other = canonical.replace(DECLARED_VERSION, OTHER_VERSION);
        assert_eq!(decode(other.as_bytes()), Err(unsupported), "{route}");
    }

    for (case, versions) in [
        ("a contradicted version", r#""version":2,"version":1"#),
        (
            "the same contradiction reversed",
            r#""version":1,"version":2"#,
        ),
        ("one version named twice", r#""version":1,"version":1"#),
        (
            "an escaped spelling first",
            r#""\u0076ersion":2,"version":1"#,
        ),
        (
            "an escaped spelling second",
            r#""version":2,"\u0076ersion":1"#,
        ),
    ] {
        for (route, canonical, decode) in &routes {
            let ambiguous = canonical.replace(DECLARED_VERSION, versions);
            assert_eq!(
                decode(ambiguous.as_bytes()),
                Err(malformed),
                "{route}: {case}"
            );
        }
    }

    let source = format!(r#""source":"{FIXTURE_SOURCE}""#);
    let repeated = format!("{source},{source}");
    for (case, from, to) in [
        ("a repeated source", source.as_str(), repeated.as_str()),
        (
            "a repeated generation",
            r#""generation":"fixture-01""#,
            r#""generation":"fixture-02","generation":"fixture-01""#,
        ),
        (
            "a repeated generation reversed",
            r#""generation":"fixture-01""#,
            r#""generation":"fixture-01","generation":"fixture-02""#,
        ),
        (
            "a repeated handle",
            r#""handle":7"#,
            r#""handle":7,"handle":8"#,
        ),
        (
            "a repeated capability",
            r#""capability":"read""#,
            r#""capability":"maintenance","capability":"read""#,
        ),
        (
            "a repeated root",
            r#""root":"/fixture""#,
            r#""root":"/elsewhere","root":"/fixture""#,
        ),
        (
            "a repeated database",
            r#""database":"/fixture/slipbox.db""#,
            r#""database":"/elsewhere.db","database":"/fixture/slipbox.db""#,
        ),
        (
            "a repeated operation",
            r#""operation":{"kind":"status"}"#,
            r#""operation":{"kind":"captureNode"},"operation":{"kind":"status"}"#,
        ),
        (
            "a denied discriminant beside an admitted one",
            r#""kind":"status""#,
            r#""kind":"captureNode","kind":"status""#,
        ),
        (
            "the same pair reversed",
            r#""kind":"status""#,
            r#""kind":"status","kind":"captureNode""#,
        ),
        (
            "an escaped spelling of a discriminant",
            r#""kind":"status""#,
            r#""kin\u0064":"captureNode","kind":"status""#,
        ),
        (
            "a repeated maintenance discriminant",
            r#""kind":"indexFile""#,
            r#""kind":"index","kind":"indexFile""#,
        ),
        (
            "a repeated file path",
            r#""file_path":"alpha.org""#,
            r#""file_path":"beta.org","file_path":"alpha.org""#,
        ),
    ] {
        let mut varied = 0;
        for (route, canonical, decode) in &routes {
            if !canonical.contains(from) {
                continue;
            }
            varied += 1;
            let ambiguous = canonical.replace(from, to);
            assert_eq!(
                decode(ambiguous.as_bytes()),
                Err(malformed),
                "{route}: {case}"
            );
            // Duplicate-key refusal precedes version validation.
            let unread = ambiguous.replace(DECLARED_VERSION, OTHER_VERSION);
            assert_eq!(
                decode(unread.as_bytes()),
                Err(malformed),
                "{route}: {case}, naming another version"
            );
        }
        assert!(varied > 0, "{case} varied no route");
    }
}

#[test]
fn a_document_naming_each_member_once_is_read_whatever_its_values_hold() {
    let request = read_request(ReadOperation::SearchNodes(SearchNodesParams {
        query: r#"{"version":1,"version":2} "quoted" \ Ωμέγα"#.to_owned(),
        limit: 25,
        sort: Some(SearchNodesSort::Relevance),
    }));
    let bytes = encoded(&request);
    assert_eq!(
        decode_read_request(&bytes).expect("a query naming members is read"),
        request
    );

    let mut routes = production_routes();
    let (_, read, decode) = routes.remove(1);
    let escaped = read.replace(DECLARED_VERSION, r#""\u0076ersion":1"#);
    decode(escaped.as_bytes()).expect("an escaped name spelled once is the name it spells");

    // Visit primitive values and array entries as well as object keys.
    let kinds = r#""extra":[1,-1,1.5,true,null,"text",{"a":1},[]]"#;
    let carried = read
        .replace(DECLARED_VERSION, OTHER_VERSION)
        .replace(r#""handle":7"#, &format!(r#""handle":7,{kinds}"#));
    assert_eq!(
        decode(carried.as_bytes()),
        Err(AdapterRefusal::of(RefusalReason::UnsupportedVersion)),
        "every kind of value is read past"
    );
    let repeated = carried.replace(r#"{"a":1}"#, r#"{"a":1,"a":2}"#);
    assert_eq!(
        decode(repeated.as_bytes()),
        Err(AdapterRefusal::of(RefusalReason::MalformedRequest)),
        "a member repeated inside an array entry"
    );
}

#[test]
fn a_request_larger_than_the_declared_bound_is_refused_before_it_is_read() {
    let query = "q".repeat(ADAPTER_LIMITS.max_request_bytes);
    let request = read_request(ReadOperation::SearchNodes(SearchNodesParams {
        query,
        limit: 25,
        sort: None,
    }));
    let bytes = encoded(&request);
    assert!(bytes.len() > ADAPTER_LIMITS.max_request_bytes);
    assert_eq!(
        refuse(&bytes),
        AdapterRefusal::bounded(AdapterBound::RequestBytes)
    );

    let inside = read_request(ReadOperation::SearchNodes(SearchNodesParams {
        query: "q".repeat(64),
        limit: 25,
        sort: None,
    }));
    let bytes = encoded(&inside);
    assert!(bytes.len() <= ADAPTER_LIMITS.max_request_bytes);
    decode_read_request(&bytes).expect("a request inside the bound is read");
}

#[test]
fn each_bound_admits_its_last_value_and_refuses_the_first_beyond_it() {
    let page = ADAPTER_LIMITS.max_page_entries;
    let relation = ADAPTER_LIMITS.max_relation_entries;
    let lines = ADAPTER_LIMITS.max_note_source_lines;
    let context = ADAPTER_LIMITS.max_context_lines;

    let admitted: Vec<ReadOperation> = vec![
        ReadOperation::SearchNodes(SearchNodesParams {
            query: "omega".to_owned(),
            limit: page,
            sort: None,
        }),
        ReadOperation::SearchNodeContent(SearchNodeContentParams {
            query: "omega".to_owned(),
            limit: page,
        }),
        ReadOperation::ListGlossaryTerms(ListGlossaryTermsParams {
            limit: page,
            after: None,
        }),
        ReadOperation::SearchGlossary(SearchGlossaryParams {
            query: "omega".to_owned(),
            limit: page,
        }),
        ReadOperation::Backlinks(BacklinksParams {
            node_key: "heading:alpha.org:3".to_owned(),
            limit: relation,
            unique: false,
        }),
        ReadOperation::ForwardLinks(ForwardLinksParams {
            node_key: "heading:alpha.org:3".to_owned(),
            limit: relation,
            unique: false,
        }),
        ReadOperation::Explore(ExploreParams {
            node_key: "heading:alpha.org:3".to_owned(),
            lens: ExplorationLens::Refs,
            limit: relation,
            unique: false,
        }),
        ReadOperation::ReadNodeSource(ReadNodeSourceParams {
            node_key: "heading:alpha.org:3".to_owned(),
            context_before: Some(context),
            context_after: Some(context),
            max_lines: Some(lines),
        }),
    ];
    for operation in admitted {
        assert_eq!(
            operation.violated_bound(),
            None,
            "{} refused its own last admitted value",
            operation.kind()
        );
    }

    let refused: Vec<(ReadOperation, AdapterBound)> = vec![
        (
            ReadOperation::SearchNodes(SearchNodesParams {
                query: "omega".to_owned(),
                limit: page + 1,
                sort: None,
            }),
            AdapterBound::PageEntries,
        ),
        (
            ReadOperation::SearchNodes(SearchNodesParams {
                query: "omega".to_owned(),
                limit: 0,
                sort: None,
            }),
            AdapterBound::PageEntries,
        ),
        (
            ReadOperation::SearchNodeContent(SearchNodeContentParams {
                query: "omega".to_owned(),
                limit: page + 1,
            }),
            AdapterBound::PageEntries,
        ),
        (
            ReadOperation::ListGlossaryTerms(ListGlossaryTermsParams {
                limit: page + 1,
                after: None,
            }),
            AdapterBound::PageEntries,
        ),
        (
            ReadOperation::SearchGlossary(SearchGlossaryParams {
                query: "omega".to_owned(),
                limit: page + 1,
            }),
            AdapterBound::PageEntries,
        ),
        (
            ReadOperation::Backlinks(BacklinksParams {
                node_key: "heading:alpha.org:3".to_owned(),
                limit: relation + 1,
                unique: false,
            }),
            AdapterBound::RelationEntries,
        ),
        (
            ReadOperation::ForwardLinks(ForwardLinksParams {
                node_key: "heading:alpha.org:3".to_owned(),
                limit: 0,
                unique: false,
            }),
            AdapterBound::RelationEntries,
        ),
        (
            ReadOperation::Explore(ExploreParams {
                node_key: "heading:alpha.org:3".to_owned(),
                lens: ExplorationLens::Refs,
                limit: relation + 1,
                unique: false,
            }),
            AdapterBound::RelationEntries,
        ),
        (
            ReadOperation::ReadNodeSource(ReadNodeSourceParams {
                node_key: "heading:alpha.org:3".to_owned(),
                context_before: None,
                context_after: None,
                max_lines: Some(lines + 1),
            }),
            AdapterBound::NoteSourceLines,
        ),
        (
            ReadOperation::ReadNodeSource(ReadNodeSourceParams {
                node_key: "heading:alpha.org:3".to_owned(),
                context_before: Some(context + 1),
                context_after: None,
                max_lines: Some(lines),
            }),
            AdapterBound::ContextLines,
        ),
        (
            ReadOperation::ReadNodeSource(ReadNodeSourceParams {
                node_key: "heading:alpha.org:3".to_owned(),
                context_before: None,
                context_after: Some(context + 1),
                max_lines: Some(lines),
            }),
            AdapterBound::ContextLines,
        ),
    ];
    for (operation, bound) in refused {
        assert_eq!(
            operation.violated_bound(),
            Some(bound),
            "{} admitted a value outside {bound}",
            operation.kind()
        );
    }

    assert_eq!(
        MaintenanceOperation::IndexFile(IndexFileParams {
            file_path: "a".repeat(ADAPTER_LIMITS.max_path_bytes + 1),
        })
        .violated_bound(),
        Some(AdapterBound::PathBytes)
    );
    assert_eq!(
        MaintenanceOperation::IndexFile(IndexFileParams {
            file_path: "a".repeat(ADAPTER_LIMITS.max_path_bytes),
        })
        .violated_bound(),
        None
    );
    assert_eq!(
        SessionContext {
            root: "r".repeat(ADAPTER_LIMITS.max_path_bytes + 1),
            database: "/data/fixture/slipbox.db".to_owned(),
        }
        .violated_bound(),
        Some(AdapterBound::PathBytes)
    );
    assert_eq!(
        SessionContext {
            root: "/data/fixture/root".to_owned(),
            database: "d".repeat(ADAPTER_LIMITS.max_path_bytes),
        }
        .violated_bound(),
        None
    );
}

#[test]
fn an_answer_larger_than_the_declared_bound_is_refused_rather_than_truncated() {
    let files = vec!["n".repeat(ADAPTER_LIMITS.max_response_bytes); 2];
    let oversize = answered(ReadAnswer::IndexedFiles(IndexedFilesResult { files }).into());
    match write_response(&oversize) {
        // The bound governs the storage, not just the answer: the writer stops
        // at the bound rather than building the whole answer and measuring it.
        ResponseEncoding::Oversized { held } => assert!(
            held <= ADAPTER_LIMITS.max_response_bytes,
            "the encoder reserved {held} bytes for a refused answer"
        ),
        other => panic!("an oversized answer was encoded as {other:?}"),
    }
    let bytes = encode_response(&oversize);
    let response: AdapterResponse =
        serde_json::from_slice(&bytes).expect("the refusal in its place decodes");
    assert_eq!(
        response.outcome,
        AdapterOutcome::Refused(AdapterRefusal::bounded(AdapterBound::ResponseBytes))
    );

    let inside = answered(
        ReadAnswer::IndexedFiles(IndexedFilesResult {
            files: vec!["alpha.org".to_owned()],
        })
        .into(),
    );
    let bytes = encode_response(&inside);
    assert_eq!(
        serde_json::from_slice::<AdapterResponse>(&bytes).expect("an answer decodes"),
        inside
    );
}

#[test]
fn the_response_bound_admits_its_last_byte_and_refuses_the_first_beyond_it() {
    let response = answered(
        ReadAnswer::IndexedFiles(IndexedFilesResult {
            files: vec!["alpha.org".to_owned(), "beta.org".to_owned()],
        })
        .into(),
    );
    let exact = serde_json::to_vec(&response)
        .expect("an answer encodes")
        .len();

    match write_bounded(&response, exact) {
        ResponseEncoding::Written(bytes) => assert_eq!(bytes.len(), exact),
        other => panic!("an answer of exactly the bound was encoded as {other:?}"),
    }
    let tighter = exact - 1;
    match write_bounded(&response, tighter) {
        ResponseEncoding::Oversized { held } => assert!(
            held <= tighter,
            "the encoder reserved {held} bytes under a bound of {tighter}"
        ),
        other => panic!("an answer one byte beyond the bound was encoded as {other:?}"),
    }
    assert_eq!(
        write_bounded(&response, 0),
        ResponseEncoding::Oversized { held: 0 },
        "a bound of nothing reserves nothing"
    );
}

#[test]
fn an_answer_the_encoder_cannot_write_is_refused_without_unbounded_storage() {
    match write_bounded(&Unencodable, ADAPTER_LIMITS.max_response_bytes) {
        ResponseEncoding::Failed { held } => assert!(
            held <= ADAPTER_LIMITS.max_response_bytes,
            "the encoder reserved {held} bytes for an answer it could not write"
        ),
        other => panic!("an unencodable answer was encoded as {other:?}"),
    }
    // The bound applies to a failure as much as to a refusal: what was written
    // before the failure is held in the same bounded storage.
    match write_bounded(&Unencodable, 8) {
        ResponseEncoding::Failed { held } | ResponseEncoding::Oversized { held } => {
            assert!(
                held <= 4_096,
                "the encoder reserved {held} bytes past a small bound"
            );
        }
        other => panic!("an unencodable answer was encoded as {other:?}"),
    }
}

/// A document whose encoding fails after some of it has been written, which is
/// the failure the production writer reports apart from an oversized answer.
struct Unencodable;

impl Serialize for Unencodable {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(2))?;
        sequence.serialize_element("written")?;
        Err(S::Error::custom("this answer cannot be encoded"))
    }
}

#[test]
fn the_answer_of_last_resort_is_the_document_it_claims_to_be() {
    let refusal = AdapterRefusal::of(RefusalReason::EncodingFailed);
    assert_eq!(
        ENCODE_FAILURE,
        serde_json::to_vec(&AdapterResponse::from(refusal))
            .expect("a refusal encodes")
            .as_slice()
    );
    let response: AdapterResponse =
        serde_json::from_slice(ENCODE_FAILURE).expect("the answer of last resort decodes");
    assert_eq!(response.outcome, AdapterOutcome::Refused(refusal));
}

#[test]
fn a_refusal_repeats_nothing_of_the_request_it_refuses() {
    const SECRET: &str = "ghp_0123456789abcdefghijklmnopqrstuvwx";

    let request = read_request(ReadOperation::NodeFromKey(NodeFromKeyParams {
        node_key: SECRET.to_owned(),
    }));
    let mut document = serde_json::to_value(&request).expect("a request encodes");
    document["operation"]["kind"] = json!("captureNode");
    let bytes = serde_json::to_vec(&document).expect("a document encodes");

    let refusal = refuse(&bytes);
    let rendered = [
        refusal.to_string(),
        format!("{refusal:?}"),
        String::from_utf8(slipbox_rpc::android::encode_refusal(refusal))
            .expect("a refusal encodes as UTF-8"),
    ];
    // The fragment itself is never printed, whichever way the refusal is rendered.
    for text in rendered {
        assert!(
            !text.contains(SECRET),
            "a refusal repeated the request it refused"
        );
        assert!(
            !text.contains("ghp_"),
            "a refusal repeated a secret-shaped prefix"
        );
    }

    let engine = AdapterRefusal::engine(
        RefusalReason::EngineRefused,
        EngineRefusal {
            code: -32602,
            kind: Some(JsonRpcErrorKind::InvalidParams),
        },
    );
    assert_eq!(
        engine.to_string(),
        "the engine declined the operation (engine code -32602)"
    );
    let bounded = AdapterRefusal::bounded(AdapterBound::PageEntries);
    assert_eq!(
        bounded.to_string(),
        "the request violates a declared bound (bound page entries)"
    );
}

#[test]
fn a_maintenance_request_names_the_same_handle_and_binding_a_read_request_does() {
    let request = MaintenanceRequest {
        version: ADAPTER_PROTOCOL_VERSION,
        handle: 8,
        binding: binding("fixture-02"),
        operation: MaintenanceOperation::Index(StateOnlyParams {}),
    };
    let bytes = serde_json::to_vec(&request).expect("a maintenance request encodes");
    assert_eq!(
        decode_maintenance_request(&bytes).expect("a maintenance request decodes"),
        request
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&bytes).expect("a document"),
        json!({
            "version": 1,
            "handle": 8,
            "binding": { "source": FIXTURE_SOURCE, "generation": "fixture-02" },
            "operation": { "kind": "index" },
        })
    );
}

/// One operation of either capability, so a test can cross every admitted
/// operation with every canonical result.
enum Admitted {
    Read(ReadOperation),
    Maintenance(MaintenanceOperation),
}

impl Admitted {
    fn kind(&self) -> &'static str {
        match self {
            Self::Read(operation) => operation.kind(),
            Self::Maintenance(operation) => operation.kind(),
        }
    }

    fn answered(&self, result: Value) -> Result<OperationAnswer, AdapterRefusal> {
        match self {
            Self::Read(operation) => operation.answered(result).map(OperationAnswer::from),
            Self::Maintenance(operation) => operation.answered(result).map(OperationAnswer::from),
        }
    }
}

fn every_admitted_operation() -> Vec<Admitted> {
    every_read_operation()
        .into_iter()
        .map(Admitted::Read)
        .chain(
            every_maintenance_operation()
                .into_iter()
                .map(Admitted::Maintenance),
        )
        .collect()
}

/// Every shared answer fixture, as the discriminant it names and the canonical
/// result it carries.
fn shared_answers() -> Vec<(String, Value)> {
    let documents = fixtures("responses.json");
    let documents = documents
        .as_object()
        .expect("the shared answers are a JSON object");
    documents
        .iter()
        .filter(|(key, _)| key.starts_with("answered_") && !key.ends_with("_absent"))
        .map(|(key, document)| {
            let answer = document
                .get("answer")
                .unwrap_or_else(|| panic!("the {key} document carries no answer"));
            let kind = answer["kind"]
                .as_str()
                .unwrap_or_else(|| panic!("the {key} answer names no operation"));
            (kind.to_owned(), answer["result"].clone())
        })
        .collect()
}

#[test]
fn the_shared_answers_cover_every_admitted_operation() {
    let mut named: Vec<String> = shared_answers().into_iter().map(|(kind, _)| kind).collect();
    named.sort();
    let mut declared: Vec<String> = READ_VOCABULARY
        .iter()
        .chain(MAINTENANCE_VOCABULARY)
        .map(|(discriminant, _)| (*discriminant).to_owned())
        .collect();
    declared.sort();
    assert_eq!(named, declared);
}

#[test]
fn every_admitted_operation_types_the_canonical_result_of_its_own_method() {
    let operations = every_admitted_operation();
    for (kind, result) in shared_answers() {
        let operation = operations
            .iter()
            .find(|operation| operation.kind() == kind)
            .unwrap_or_else(|| panic!("{kind} is not an admitted operation"));
        let answer = operation
            .answered(result.clone())
            .unwrap_or_else(|refusal| panic!("{kind} refused its own canonical result: {refusal}"));
        assert_eq!(answer.kind(), kind);
        assert_eq!(
            serde_json::to_value(&answer).expect("an answer encodes"),
            json!({ "kind": kind, "result": result }),
            "{kind} does not carry its canonical result in full"
        );
    }
}

/// The only operations whose canonical results are literally the same type: both
/// node lookups answer with one record, so a record is an answer of either. Every
/// other pair is distinguishable and must be refused.
const INTERCHANGEABLE: &[(&str, &str)] =
    &[("nodeFromId", "nodeFromKey"), ("nodeFromKey", "nodeFromId")];

#[test]
fn an_operation_answered_with_another_operations_result_is_refused() {
    let uncanonical = AdapterRefusal::of(RefusalReason::UncanonicalResult);
    let answers = shared_answers();
    let operations = every_admitted_operation();
    let result_of = |wanted: &str| {
        answers
            .iter()
            .find(|(kind, _)| kind == wanted)
            .map(|(_, result)| result.clone())
            .unwrap_or_else(|| panic!("{wanted} has no shared answer"))
    };
    let operation_of = |wanted: &str| {
        operations
            .iter()
            .find(|operation| operation.kind() == wanted)
            .unwrap_or_else(|| panic!("{wanted} is not admitted"))
    };

    assert_eq!(
        operation_of("status")
            .answered(result_of("indexedFiles"))
            .expect_err("a status request is not answered by the file list"),
        uncanonical
    );
    assert_eq!(
        operation_of("indexedFiles")
            .answered(result_of("status"))
            .expect_err("a file list request is not answered by the state report"),
        uncanonical
    );

    for (kind, result) in &answers {
        for operation in &operations {
            if operation.kind() == kind {
                continue;
            }
            let pair = (kind.as_str(), operation.kind());
            match operation.answered(result.clone()) {
                Ok(answer) => assert!(
                    INTERCHANGEABLE.contains(&pair),
                    "{} was answered with the result of {kind} as {}",
                    operation.kind(),
                    answer.kind()
                ),
                Err(refusal) => {
                    assert_eq!(
                        refusal,
                        uncanonical,
                        "{} refused the result of {kind} for another reason",
                        operation.kind()
                    );
                    assert!(
                        !INTERCHANGEABLE.contains(&pair),
                        "{pair:?} is declared interchangeable but was refused"
                    );
                }
            }
        }
    }
}

#[test]
fn only_a_lookup_whose_canonical_answer_admits_absence_is_answered_with_nothing() {
    for (key, kind) in [
        ("answered_node_from_id_absent", "nodeFromId"),
        ("answered_node_from_key_absent", "nodeFromKey"),
        ("answered_glossary_term_absent", "glossaryTerm"),
    ] {
        let document = fixture("responses.json", key);
        let response: AdapterResponse = serde_json::from_value(document.clone())
            .unwrap_or_else(|failure| panic!("the {key} document does not decode: {failure}"));
        let AdapterOutcome::Answered(answer) = &response.outcome else {
            panic!("the {key} document is not an answer");
        };
        assert_eq!(answer.kind(), kind);
        assert_eq!(
            serde_json::to_value(&response).expect("an answer re-encodes"),
            document
        );
    }

    for operation in every_admitted_operation() {
        let typed = operation.answered(Value::Null);
        if matches!(operation.kind(), "nodeFromId" | "nodeFromKey") {
            assert!(
                typed.is_ok(),
                "{} refuses the absence its canonical lookup admits",
                operation.kind()
            );
        } else {
            assert_eq!(
                typed.expect_err("an operation whose result is a document is not answered by none"),
                AdapterRefusal::of(RefusalReason::UncanonicalResult),
                "{} succeeded with nothing",
                operation.kind()
            );
        }
    }
}

/// `glossaryTerm` carries one optional term, so decoding alone accepts any object
/// at all: absence is spelled, not inferred from a payload that happens to say
/// nothing about a term.
#[test]
fn an_answer_of_only_optional_fields_still_refuses_a_foreign_payload() {
    let uncanonical = AdapterRefusal::of(RefusalReason::UncanonicalResult);
    let lookup = every_admitted_operation()
        .into_iter()
        .find(|operation| operation.kind() == "glossaryTerm")
        .expect("the glossary lookup is admitted");
    let backlinks = shared_answers()
        .into_iter()
        .find(|(kind, _)| kind == "backlinks")
        .map(|(_, result)| result)
        .expect("the backlink page has a shared answer");

    for foreign in [backlinks, json!({}), json!({ "term": null, "total": 0 })] {
        assert_eq!(
            lookup
                .answered(foreign.clone())
                .expect_err("a document that is not a glossary term answer is refused"),
            uncanonical,
            "{foreign} was typed as a glossary term answer"
        );
    }

    let absent = fixture("responses.json", "answered_glossary_term_absent");
    let answer = lookup
        .answered(absent["answer"]["result"].clone())
        .expect("an explicitly absent term is this lookup's own answer");
    assert_eq!(answer.kind(), "glossaryTerm");
}

#[test]
fn an_answer_document_this_contract_cannot_read_is_refused() {
    for key in [
        "bad_answered_foreign_payload",
        "bad_answered_unknown_kind",
        "bad_answered_missing_field",
        "bad_answered_wrong_type",
        "bad_answered_null_result",
        "bad_answered_extra_answer_field",
        "bad_answered_no_kind",
    ] {
        assert!(
            serde_json::from_value::<AdapterResponse>(fixture("responses.json", key)).is_err(),
            "the {key} document was read as an answer"
        );
    }
}

#[test]
fn an_answer_names_one_capabilitys_operation_and_not_the_others() {
    for (kind, result) in shared_answers() {
        let document = json!({ "kind": kind, "result": result });
        let declared_read = READ_VOCABULARY
            .iter()
            .any(|(discriminant, _)| *discriminant == kind);
        assert_eq!(
            serde_json::from_value::<ReadAnswer>(document.clone()).is_ok(),
            declared_read,
            "{kind} as a read answer"
        );
        assert_eq!(
            serde_json::from_value::<MaintenanceAnswer>(document.clone()).is_ok(),
            !declared_read,
            "{kind} as a maintenance answer"
        );

        let answer: OperationAnswer =
            serde_json::from_value(document).expect("an answer names one of the two vocabularies");
        assert_eq!(answer.kind(), kind);
        assert_eq!(
            matches!(&answer, OperationAnswer::Read(_)),
            declared_read,
            "{kind} was typed into the other capability"
        );
    }

    let maintenance = fixture("responses.json", "crossed_answer_maintenance");
    assert!(
        serde_json::from_value::<ReadAnswer>(maintenance["answer"].clone()).is_err(),
        "a maintenance answer was read as a read answer"
    );
    let read = fixture("responses.json", "crossed_answer_read");
    assert!(
        serde_json::from_value::<MaintenanceAnswer>(read["answer"].clone()).is_err(),
        "a read answer was read as a maintenance answer"
    );
}

#[test]
fn an_answer_naming_no_version_is_not_read_as_version_one() {
    for key in ["bad_answered_without_version", "bad_opened_without_version"] {
        assert!(
            serde_json::from_value::<AdapterResponse>(fixture("responses.json", key)).is_err(),
            "the {key} document was read as a versioned answer"
        );
    }
    // The compatibility policy is finite: exactly one version is spoken, and an
    // answer naming another is refused rather than adapted to.
    assert_eq!(ADAPTER_PROTOCOL_VERSION, 1);
    let unsupported = fixture("responses.json", "bad_answered_unsupported_version");
    assert_ne!(
        unsupported["version"].as_u64(),
        Some(u64::from(ADAPTER_PROTOCOL_VERSION)),
        "the answer the mirror refuses names the version this library speaks"
    );
    assert_eq!(
        answered(ReadAnswer::IndexedFiles(IndexedFilesResult { files: Vec::new() }).into()).version,
        ADAPTER_PROTOCOL_VERSION
    );
}

#[test]
fn no_advertised_contract_the_mirror_refuses_is_the_one_this_library_reports() {
    let reported = serde_json::to_value(AdapterResponse::new(AdapterOutcome::Contract(
        AdapterContract::default(),
    )))
    .expect("the contract encodes");

    // A limit this contract has no field for, and a count no unsigned field
    // holds, are refused while the document is decoded.
    for key in ["bad_contract_missing_limit", "bad_contract_negative_queue"] {
        assert!(
            serde_json::from_value::<AdapterResponse>(fixture("responses.json", key)).is_err(),
            "the {key} document was read as a contract"
        );
    }
    // The rest are documents of this contract that advertise something else, so
    // what refuses them is the mirror's comparison against its own expectation.
    for key in [
        "bad_contract_zero_queue",
        "bad_contract_unbounded_queue",
        "bad_contract_excessive_sessions",
        "bad_contract_duplicate_operation",
        "bad_contract_unknown_operation",
        "bad_contract_short_vocabulary",
    ] {
        let document = round_trip(key);
        assert_ne!(
            document, reported,
            "the {key} document advertises this library's own contract"
        );
    }

    // The positive control: one document differs from the reported one in the
    // order of its vocabularies alone, and advertises the same contract.
    let reordered = round_trip("contract_reordered_vocabulary");
    assert_ne!(
        reordered, reported,
        "the reordered document is in the order this library reports"
    );
    assert_eq!(reordered["limits"], reported["limits"]);
    for field in ["read_operations", "maintenance_operations"] {
        assert_eq!(
            vocabulary(&reordered, field),
            vocabulary(&reported, field),
            "the reordered document advertises another {field} vocabulary"
        );
    }
}

/// A response fixture, held to decoding as a document of this contract and to
/// re-encoding as the same document.
fn round_trip(key: &str) -> Value {
    let document = fixture("responses.json", key);
    let response: AdapterResponse = serde_json::from_value(document.clone())
        .unwrap_or_else(|failure| panic!("the {key} document does not decode: {failure}"));
    assert_eq!(
        serde_json::to_value(&response).expect("a contract re-encodes"),
        document,
        "the {key} document does not survive a round trip"
    );
    document
}

fn vocabulary(document: &Value, field: &str) -> Vec<String> {
    let mut kinds: Vec<String> = document[field]
        .as_array()
        .expect("a contract lists its operations")
        .iter()
        .map(|kind| {
            kind.as_str()
                .expect("an operation is named by a string")
                .to_owned()
        })
        .collect();
    kinds.sort();
    kinds
}

#[test]
fn no_two_admitted_operations_share_a_discriminant_or_a_method() {
    let pairs: Vec<(&str, &str)> = READ_VOCABULARY
        .iter()
        .chain(MAINTENANCE_VOCABULARY)
        .copied()
        .collect();

    let mut discriminants: Vec<&str> = pairs.iter().map(|(kind, _)| *kind).collect();
    discriminants.sort_unstable();
    discriminants.dedup();
    assert_eq!(
        discriminants.len(),
        pairs.len(),
        "two admitted operations share a discriminant"
    );

    let mut methods: Vec<&str> = pairs.iter().map(|(_, method)| *method).collect();
    methods.sort_unstable();
    methods.dedup();
    assert_eq!(
        methods.len(),
        pairs.len(),
        "two admitted operations share a method"
    );
}
