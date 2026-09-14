//! Adapter round trips and comparison with the canonical engine.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs, thread};

use serde_json::{Value, json};
use slipbox_android::adapter::{
    Registry, contained, serve_close, serve_contract, serve_maintenance, serve_open, serve_read,
};
use slipbox_engine::service::SlipboxService;
use slipbox_engine::{DiscoveryPolicy, PlatformPolicy};
use slipbox_rpc::android::{
    ADAPTER_LIMITS, ADAPTER_PROTOCOL_VERSION, AdapterCapability, AdapterOutcome, AdapterResponse,
    MAINTENANCE_VOCABULARY, READ_VOCABULARY,
};
use slipbox_rpc::{
    METHOD_BACKLINKS, METHOD_EXPLORE, METHOD_FORWARD_LINKS, METHOD_GLOSSARY_TERM, METHOD_INDEX,
    METHOD_INDEX_FILE, METHOD_INDEXED_FILES, METHOD_LIST_GLOSSARY_TERMS, METHOD_NODE_FROM_ID,
    METHOD_NODE_FROM_KEY, METHOD_READ_NODE_SOURCE, METHOD_SEARCH_GLOSSARY,
    METHOD_SEARCH_NODE_CONTENT, METHOD_SEARCH_NODES,
};
use tempfile::TempDir;

const SOURCE: &str = "0102030405060708090a0b0c0d0e0f10";
const OTHER_SOURCE: &str = "1112131415161718191a1b1c1d1e1f20";
const GENERATION: &str = "fixture-01";
const OTHER_GENERATION: &str = "fixture-02";

const FIXTURE_FILES: &[&str] = &["alpha.org", "beta.org", "riemann.org"];

/// A file needing a decryptor no headless session authorizes.
const ENVELOPE: &str = "sealed.org.gpg";

/// Shaped like a credential and generated here so a diagnostic that reflects a
/// request is recognizable. Nothing prints it.
const SECRET: &str = "ghp_0123456789abcdefghijklmnopqrstuvwx";

/// The engine's own code for an operation it declines by authority, and for one
/// it could not carry out.
const REFUSED: i64 = -32600;
const FAILED: i64 = -32603;

/// The case whose diagnostics are examined in the process that wrote them, and
/// the line it prints once every route has been written.
const DRIVEN: &str = "every_failing_route_writes_its_diagnostic_where_that_process_reads";
const REPORTED: &str = "every failing route reported";

#[test]
fn the_served_contract_is_the_one_the_library_declares() {
    let contract = document(serve_contract());

    assert_eq!(contract["outcome"], "contract", "{contract}");
    assert_eq!(
        contract["limits"],
        serde_json::to_value(ADAPTER_LIMITS).expect("the limits encode")
    );
    assert_eq!(kinds(&contract["read_operations"]), read_inventory());
    assert_eq!(
        kinds(&contract["maintenance_operations"]),
        discriminants(MAINTENANCE_VOCABULARY)
    );
}

#[test]
fn every_read_operation_answers_what_the_engine_answers_directly() {
    let fixture = Fixture::new("one");
    let registry = Registry::new();
    let binding = binding(SOURCE, GENERATION);
    index(&registry, &fixture, &binding);

    let mut canonical = fixture.canonical();
    let keys = Keys::of(&mut canonical);
    let handle = handle_of(&open(
        &registry,
        AdapterCapability::Read,
        &fixture,
        &binding,
    ));

    let mut exercised = BTreeSet::from(["status".to_owned()]);
    for (operation, method, params) in read_cases(&keys) {
        let response = read(&registry, handle, &binding, &operation);
        let expected = canonical
            .invoke_value(method, params)
            .unwrap_or_else(|error| panic!("the engine refused {method}: {error}"));

        assert_eq!(result_of(&response), &expected, "{operation}");
        assert_eq!(response["answer"]["kind"], operation["kind"]);
        assert_eq!(response["handle"], handle);
        assert_eq!(response["binding"], binding);
        exercised.insert(kind_of(&operation));
    }

    assert_eq!(
        exercised,
        read_inventory(),
        "a declared read operation is never sent"
    );
}

#[test]
fn every_maintenance_operation_answers_what_the_engine_answers_directly() {
    let fixture = Fixture::new("one");
    let registry = Registry::new();
    let binding = binding(SOURCE, GENERATION);
    let mut canonical = fixture.canonical();

    let handle = handle_of(&open(
        &registry,
        AdapterCapability::Maintenance,
        &fixture,
        &binding,
    ));
    let mut exercised = BTreeSet::new();
    for (operation, method, params) in [
        (json!({"kind": "index"}), METHOD_INDEX, json!({})),
        (
            json!({"kind": "indexFile", "file_path": "alpha.org"}),
            METHOD_INDEX_FILE,
            json!({"file_path": "alpha.org"}),
        ),
    ] {
        let response = maintain(&registry, handle, &binding, &operation);
        let expected = canonical
            .invoke_value(method, params)
            .unwrap_or_else(|error| panic!("the engine refused {method}: {error}"));

        assert_eq!(result_of(&response), &expected, "{operation}");
        assert_eq!(response["answer"]["kind"], operation["kind"]);
        exercised.insert(kind_of(&operation));
    }

    assert_eq!(
        exercised,
        discriminants(MAINTENANCE_VOCABULARY),
        "a declared maintenance operation is never sent"
    );
}

#[test]
fn a_session_answers_for_the_root_and_database_it_was_opened_against() {
    let fixture = Fixture::new("one");
    let registry = Registry::new();
    let binding = binding(SOURCE, GENERATION);
    index(&registry, &fixture, &binding);

    let handle = handle_of(&open(
        &registry,
        AdapterCapability::Read,
        &fixture,
        &binding,
    ));
    let opened = read(&registry, handle, &binding, &json!({"kind": "status"}));
    assert_eq!(result_of(&opened)["root"], text(&resolved(&fixture.root())));
    assert_eq!(result_of(&opened)["db"], text(&fixture.database()));
    assert_eq!(result_of(&opened)["files_indexed"], 3);

    for (operation, _, _) in read_cases(&Keys::of(&mut fixture.canonical())) {
        read(&registry, handle, &binding, &operation);
    }
    let last = read(&registry, handle, &binding, &json!({"kind": "status"}));
    assert_eq!(result_of(&last), result_of(&opened));
}

#[test]
fn a_request_naming_another_binding_is_refused_rather_than_answered() {
    let fixture = Fixture::new("one");
    let registry = Registry::new();
    let held = binding(SOURCE, GENERATION);
    index(&registry, &fixture, &held);

    let handle = handle_of(&open(&registry, AdapterCapability::Read, &fixture, &held));
    for other in [
        binding(SOURCE, OTHER_GENERATION),
        binding(OTHER_SOURCE, GENERATION),
        binding(OTHER_SOURCE, OTHER_GENERATION),
    ] {
        let response = read(&registry, handle, &other, &json!({"kind": "indexedFiles"}));
        assert_eq!(reason_of(&response), "binding-mismatch", "{other}");
    }
}

#[test]
fn two_sources_that_share_a_node_identifier_answer_only_for_their_own() {
    let registry = Registry::new();
    let first = Fixture::new("first");
    let second = Fixture::new("second");
    let bindings = [
        binding(SOURCE, GENERATION),
        binding(OTHER_SOURCE, GENERATION),
    ];

    for (fixture, binding) in [&first, &second].into_iter().zip(&bindings) {
        index(&registry, fixture, binding);
    }
    let handles: Vec<i64> = [&first, &second]
        .into_iter()
        .zip(&bindings)
        .map(|(fixture, binding)| {
            handle_of(&open(&registry, AdapterCapability::Read, fixture, binding))
        })
        .collect();

    for (position, fixture) in [&first, &second].into_iter().enumerate() {
        let response = read(
            &registry,
            handles[position],
            &bindings[position],
            &json!({"kind": "nodeFromId", "id": "beta-target"}),
        );
        assert_eq!(
            result_of(&response)["title"],
            Value::from(fixture.title("Target heading"))
        );
        assert_eq!(response["binding"], bindings[position]);
    }
}

#[test]
fn two_generations_of_one_source_answer_only_for_their_own() {
    let registry = Registry::new();
    let earlier = Fixture::new("earlier");
    let later = Fixture::new("later");
    let bindings = [
        binding(SOURCE, GENERATION),
        binding(SOURCE, OTHER_GENERATION),
    ];

    for (fixture, binding) in [&earlier, &later].into_iter().zip(&bindings) {
        index(&registry, fixture, binding);
    }
    let handles: Vec<i64> = [&earlier, &later]
        .into_iter()
        .zip(&bindings)
        .map(|(fixture, binding)| {
            handle_of(&open(&registry, AdapterCapability::Read, fixture, binding))
        })
        .collect();

    for (position, fixture) in [&earlier, &later].into_iter().enumerate() {
        let response = read(
            &registry,
            handles[position],
            &bindings[position],
            &json!({"kind": "nodeFromId", "id": "alpha-first"}),
        );
        assert_eq!(
            result_of(&response)["title"],
            Value::from(fixture.title("First heading"))
        );
    }

    // The generation is part of the identity, not a label beside it.
    let crossed = read(
        &registry,
        handles[0],
        &bindings[1],
        &json!({"kind": "nodeFromId", "id": "alpha-first"}),
    );
    assert_eq!(reason_of(&crossed), "binding-mismatch");
}

#[test]
fn every_session_answers_its_own_corpus_when_they_are_queried_at_once() {
    let registry = Registry::new();
    let first = Fixture::new("first");
    let second = Fixture::new("second");
    let sessions = [
        (&first, binding(SOURCE, GENERATION)),
        (&second, binding(OTHER_SOURCE, GENERATION)),
    ];

    let opened: Vec<i64> = sessions
        .iter()
        .map(|(fixture, binding)| {
            index(&registry, fixture, binding);
            handle_of(&open(&registry, AdapterCapability::Read, fixture, binding))
        })
        .collect();

    thread::scope(|scope| {
        for (position, (fixture, binding)) in sessions.iter().enumerate() {
            let registry = &registry;
            let handle = opened[position];
            scope.spawn(move || {
                for _ in 0..16 {
                    let response = read(
                        registry,
                        handle,
                        binding,
                        &json!({"kind": "nodeFromId", "id": "beta-target"}),
                    );
                    assert_eq!(
                        result_of(&response)["title"],
                        Value::from(fixture.title("Target heading"))
                    );
                    assert_eq!(response["handle"], handle);
                    assert_eq!(response["binding"], *binding);
                }
            });
        }
    });
}

#[test]
fn a_handle_no_session_holds_is_refused() {
    let fixture = Fixture::new("one");
    let registry = Registry::new();
    let binding = binding(SOURCE, GENERATION);
    let handle = handle_of(&open(
        &registry,
        AdapterCapability::Read,
        &fixture,
        &binding,
    ));

    for unknown in [0, -1, i64::MIN, i64::MAX, handle + 1] {
        let response = read(&registry, unknown, &binding, &json!({"kind": "status"}));
        assert_eq!(reason_of(&response), "unknown-handle", "handle {unknown}");
        let closed = close(&registry, unknown, &binding);
        assert_eq!(reason_of(&closed), "unknown-handle", "handle {unknown}");
    }
    assert_eq!(
        result_of(&read(
            &registry,
            handle,
            &binding,
            &json!({"kind": "status"})
        ))["files_indexed"],
        0
    );
}

#[test]
fn a_request_naming_one_member_twice_is_refused_before_anything_is_allocated() {
    let fixture = Fixture::new("one");
    let registry = Registry::new();
    let binding = binding(SOURCE, GENERATION);
    let held = handle_of(&open(
        &registry,
        AdapterCapability::Read,
        &fixture,
        &binding,
    ));

    let opening = ambiguous(&open_request(AdapterCapability::Read, &fixture, &binding));
    let reading = ambiguous(&request(held, &binding, &json!({"kind": "status"})));
    let maintaining = ambiguous(&request(held, &binding, &json!({"kind": "index"})));
    let closing = ambiguous(&json!({
        "version": ADAPTER_PROTOCOL_VERSION,
        "handle": held,
        "binding": binding,
    }));

    for (route, response) in [
        (
            "open",
            serve_open(&registry, AdapterCapability::Read, &opening).response,
        ),
        ("read", serve_read(&registry, &reading)),
        ("maintenance", serve_maintenance(&registry, &maintaining)),
        ("close", serve_close(&registry, &closing)),
    ] {
        assert_eq!(
            reason_of(&document(response)),
            "malformed-request",
            "{route}"
        );
    }

    let next = handle_of(&open(
        &registry,
        AdapterCapability::Read,
        &fixture,
        &binding,
    ));
    assert_eq!(next, held + 1);
    assert_eq!(
        result_of(&read(&registry, held, &binding, &json!({"kind": "status"})))["files_indexed"],
        0
    );
}

#[test]
fn a_retired_session_answers_nothing_and_its_identity_is_never_reissued() {
    let fixture = Fixture::new("one");
    let registry = Registry::new();
    let binding = binding(SOURCE, GENERATION);
    index(&registry, &fixture, &binding);

    let handle = handle_of(&open(
        &registry,
        AdapterCapability::Read,
        &fixture,
        &binding,
    ));
    assert!(
        result_of(&read(
            &registry,
            handle,
            &binding,
            &json!({"kind": "status"})
        ))
        .is_object()
    );

    let closed = close(&registry, handle, &binding);
    assert_eq!(closed["outcome"], "closed", "{closed}");
    assert_eq!(closed["retired"], true);
    assert_eq!(closed["handle"], handle);

    let after = read(&registry, handle, &binding, &json!({"kind": "status"}));
    assert_eq!(reason_of(&after), "retired-handle");

    // Retiring twice is defined: the second call finds nothing left to retire.
    let again = close(&registry, handle, &binding);
    assert_eq!(again["outcome"], "closed", "{again}");
    assert_eq!(again["retired"], false);

    let reopened = handle_of(&open(
        &registry,
        AdapterCapability::Read,
        &fixture,
        &binding,
    ));
    assert!(reopened > handle, "{reopened} reuses a retired identity");
    let refused = read(&registry, handle, &binding, &json!({"kind": "status"}));
    assert_eq!(reason_of(&refused), "retired-handle");
}

#[test]
fn an_operation_belonging_to_the_other_capability_is_refused_through_both_symbols() {
    let fixture = Fixture::new("one");
    let registry = Registry::new();
    let binding = binding(SOURCE, GENERATION);
    let reader = handle_of(&open(
        &registry,
        AdapterCapability::Read,
        &fixture,
        &binding,
    ));
    let maintainer = handle_of(&open(
        &registry,
        AdapterCapability::Maintenance,
        &fixture,
        &binding,
    ));

    // A session admits one capability, so the symbol and the handle must agree.
    let crossed = maintain(&registry, reader, &binding, &json!({"kind": "index"}));
    assert_eq!(reason_of(&crossed), "capability-mismatch");
    let crossed = read(&registry, maintainer, &binding, &json!({"kind": "status"}));
    assert_eq!(reason_of(&crossed), "capability-mismatch");

    // Neither vocabulary admits the other's operations, whatever handle is named.
    let outside = read(&registry, reader, &binding, &json!({"kind": "index"}));
    assert_eq!(reason_of(&outside), "unknown-operation");
    let outside = maintain(&registry, maintainer, &binding, &json!({"kind": "status"}));
    assert_eq!(reason_of(&outside), "unknown-operation");

    // A capability declared against the other symbol opens nothing.
    let request = json!({
        "version": ADAPTER_PROTOCOL_VERSION,
        "capability": AdapterCapability::Maintenance,
        "binding": binding,
        "context": fixture.context(),
    });
    let refused = serve_open(&registry, AdapterCapability::Read, &encoded(&request));
    assert_eq!(refused.opened, None);
    assert_eq!(
        reason_of(&document(refused.response)),
        "capability-mismatch"
    );
}

#[test]
fn no_operation_that_writes_reaches_either_capability() {
    let fixture = Fixture::new("one");
    let registry = Registry::new();
    let binding = binding(SOURCE, GENERATION);
    let reader = handle_of(&open(
        &registry,
        AdapterCapability::Read,
        &fixture,
        &binding,
    ));
    let maintainer = handle_of(&open(
        &registry,
        AdapterCapability::Maintenance,
        &fixture,
        &binding,
    ));

    for kind in [
        "captureNode",
        "gradeTerm",
        "slipboxStructuralEditApply",
        "slipboxLinkRewriteApply",
        "reviewRun",
        "runWorkflow",
        "invented",
    ] {
        let operation = json!({"kind": kind, "node_key": "heading:alpha.org:3"});
        let refused = read(&registry, reader, &binding, &operation);
        assert_eq!(reason_of(&refused), "unknown-operation", "{kind}");
        let refused = maintain(&registry, maintainer, &binding, &operation);
        assert_eq!(reason_of(&refused), "unknown-operation", "{kind}");
    }
}

#[test]
fn a_context_that_names_no_openable_root_opens_nothing() {
    let fixture = Fixture::new("one");
    let registry = Registry::new();
    let binding = binding(SOURCE, GENERATION);

    for context in [
        json!({"root": text(&fixture.root().join("absent")), "database": text(&fixture.database())}),
        json!({"root": "notes", "database": text(&fixture.database())}),
        json!({"root": text(&fixture.root()), "database": "slipbox.sqlite"}),
        json!({"root": text(&fixture.root()), "database": text(&fixture.root())}),
    ] {
        let request = json!({
            "version": ADAPTER_PROTOCOL_VERSION,
            "capability": AdapterCapability::Read,
            "binding": binding,
            "context": context,
        });
        let refused = serve_open(&registry, AdapterCapability::Read, &encoded(&request));
        assert_eq!(refused.opened, None, "{context}");
        assert_eq!(
            reason_of(&document(refused.response)),
            "invalid-context",
            "{context}"
        );
    }

    // The positive control for the same construction.
    let opened = open(&registry, AdapterCapability::Read, &fixture, &binding);
    assert_eq!(opened["outcome"], "opened", "{opened}");
}

#[test]
fn no_declared_bound_is_exceeded_and_none_is_silently_narrowed() {
    let fixture = Fixture::new("one");
    let registry = Registry::new();
    let binding = binding(SOURCE, GENERATION);
    index(&registry, &fixture, &binding);
    let key = Keys::of(&mut fixture.canonical()).beta;

    let reader = handle_of(&open(
        &registry,
        AdapterCapability::Read,
        &fixture,
        &binding,
    ));
    let page = ADAPTER_LIMITS.max_page_entries;
    let relations = ADAPTER_LIMITS.max_relation_entries;
    let context = ADAPTER_LIMITS.max_context_lines;
    let lines = ADAPTER_LIMITS.max_note_source_lines;
    let cases = [
        (
            json!({"kind": "searchNodes", "query": "target", "limit": page}),
            None,
        ),
        (
            json!({"kind": "searchNodes", "query": "target", "limit": page + 1}),
            Some("page-entries"),
        ),
        (
            json!({"kind": "searchNodes", "query": "target", "limit": 0}),
            Some("page-entries"),
        ),
        (
            json!({"kind": "searchGlossary", "query": "riemann", "limit": page + 1}),
            Some("page-entries"),
        ),
        (
            json!({"kind": "listGlossaryTerms", "limit": page + 1}),
            Some("page-entries"),
        ),
        (
            json!({"kind": "searchNodeContent", "query": "body", "limit": page + 1}),
            Some("page-entries"),
        ),
        (
            json!({"kind": "backlinks", "node_key": key, "limit": relations}),
            None,
        ),
        (
            json!({"kind": "backlinks", "node_key": key, "limit": relations + 1}),
            Some("relation-entries"),
        ),
        (
            json!({"kind": "forwardLinks", "node_key": key, "limit": 0}),
            Some("relation-entries"),
        ),
        (
            json!({"kind": "explore", "node_key": key, "lens": "structure", "limit": relations + 1}),
            Some("relation-entries"),
        ),
        (
            json!({"kind": "readNodeSource", "node_key": key, "context_before": context}),
            None,
        ),
        (
            json!({"kind": "readNodeSource", "node_key": key, "context_before": context + 1}),
            Some("context-lines"),
        ),
        (
            json!({"kind": "readNodeSource", "node_key": key, "context_after": context + 1}),
            Some("context-lines"),
        ),
        (
            json!({"kind": "readNodeSource", "node_key": key, "max_lines": lines}),
            None,
        ),
        (
            json!({"kind": "readNodeSource", "node_key": key, "max_lines": lines + 1}),
            Some("note-source-lines"),
        ),
        (
            json!({"kind": "readNodeSource", "node_key": key, "max_lines": 0}),
            Some("note-source-lines"),
        ),
    ];

    for (operation, bound) in cases {
        let response = read(&registry, reader, &binding, &operation);
        match bound {
            None => assert_eq!(response["outcome"], "answered", "{operation} {response}"),
            Some(bound) => {
                assert_eq!(reason_of(&response), "out-of-bounds", "{operation}");
                assert_eq!(response["bound"], Value::from(bound), "{operation}");
            }
        }
    }

    let maintainer = handle_of(&open(
        &registry,
        AdapterCapability::Maintenance,
        &fixture,
        &binding,
    ));
    let long = "a".repeat(ADAPTER_LIMITS.max_path_bytes + 1);
    let refused = maintain(
        &registry,
        maintainer,
        &binding,
        &json!({"kind": "indexFile", "file_path": long}),
    );
    assert_eq!(reason_of(&refused), "out-of-bounds");
    assert_eq!(refused["bound"], "path-bytes");
}

#[test]
fn a_note_the_engine_had_to_cut_short_is_refused_rather_than_partly_answered() {
    let fixture = Fixture::new("one");
    let registry = Registry::new();
    let binding = binding(SOURCE, GENERATION);
    let body: String = (0..ADAPTER_LIMITS.max_note_source_lines + 200)
        .map(|line| format!("Body line {line}.\n"))
        .collect();
    fixture.write(
        "long.org",
        &format!("#+title: Long\n\n* Long heading\n:PROPERTIES:\n:ID: long-note\n:END:\n{body}"),
    );
    index(&registry, &fixture, &binding);

    let mut canonical = fixture.canonical();
    let keys = Keys::of(&mut canonical);
    let long = key_of(&mut canonical, "long-note");
    let reader = handle_of(&open(
        &registry,
        AdapterCapability::Read,
        &fixture,
        &binding,
    ));

    let refused = read(
        &registry,
        reader,
        &binding,
        &json!({"kind": "readNodeSource", "node_key": long, "max_lines": ADAPTER_LIMITS.max_note_source_lines}),
    );
    assert_eq!(reason_of(&refused), "out-of-bounds");
    assert_eq!(refused["bound"], "note-source-lines");

    // A note inside the bound is answered whole, to its last line.
    let whole = read(
        &registry,
        reader,
        &binding,
        &json!({"kind": "readNodeSource", "node_key": keys.beta, "max_lines": ADAPTER_LIMITS.max_note_source_lines}),
    );
    let source = &result_of(&whole)["source"];
    assert_eq!(source["truncated_before"], false, "{whole}");
    assert_eq!(source["truncated_after"], false, "{whole}");
    assert!(
        source["content"]
            .as_str()
            .expect("a source answer carries its content")
            .contains("Target body."),
        "{whole}"
    );
}

#[test]
fn the_table_holds_no_more_sessions_than_it_declares() {
    let fixture = Fixture::new("one");
    let registry = Registry::new();
    let binding = binding(SOURCE, GENERATION);

    let held: Vec<i64> = (0..ADAPTER_LIMITS.max_open_sessions)
        .map(|_| {
            handle_of(&open(
                &registry,
                AdapterCapability::Read,
                &fixture,
                &binding,
            ))
        })
        .collect();
    assert_eq!(held, (1..=held.len() as i64).collect::<Vec<i64>>());

    let refused = open(&registry, AdapterCapability::Read, &fixture, &binding);
    assert_eq!(reason_of(&refused), "sessions-exhausted");
    assert_eq!(refused["bound"], "open-sessions");

    // Every session the table already held is still answering.
    for handle in &held {
        let response = read(&registry, *handle, &binding, &json!({"kind": "status"}));
        assert_eq!(response["handle"], *handle, "{response}");
    }

    // A refused open spends no identity, and a retired one is not reissued.
    assert_eq!(close(&registry, held[0], &binding)["retired"], true);
    let opened = handle_of(&open(
        &registry,
        AdapterCapability::Read,
        &fixture,
        &binding,
    ));
    assert_eq!(opened, held.len() as i64 + 1);
}

#[test]
fn a_close_naming_another_binding_retires_nothing_and_leaves_the_session_usable() {
    let fixture = Fixture::new("one");
    let registry = Registry::new();
    let held = binding(SOURCE, GENERATION);
    index(&registry, &fixture, &held);
    let handle = handle_of(&open(&registry, AdapterCapability::Read, &fixture, &held));

    for other in [
        binding(SOURCE, OTHER_GENERATION),
        binding(OTHER_SOURCE, GENERATION),
        binding(OTHER_SOURCE, OTHER_GENERATION),
    ] {
        let refused = close(&registry, handle, &other);
        assert_eq!(reason_of(&refused), "binding-mismatch", "{other}");
        assert!(
            result_of(&read(&registry, handle, &held, &json!({"kind": "status"}))).is_object(),
            "a close naming {other} retired the session it did not name"
        );
    }

    let closed = close(&registry, handle, &held);
    assert_eq!(closed["retired"], true, "{closed}");
    assert_eq!(closed["binding"], held);

    // The binding is reported by the call that retired the session, and by no
    // later one: nothing is kept to report it again.
    let again = close(&registry, handle, &held);
    assert_eq!(again["retired"], false, "{again}");
    assert_eq!(again["binding"], Value::Null);
}

#[test]
fn a_session_whose_answer_never_reached_its_caller_holds_no_slot_afterwards() {
    let fixture = Fixture::new("one");
    let registry = Registry::new();
    let binding = binding(SOURCE, GENERATION);
    index(&registry, &fixture, &binding);
    let kept = handle_of(&open(
        &registry,
        AdapterCapability::Read,
        &fixture,
        &binding,
    ));

    // What the JNI seam does when the JVM cannot take the answer that would have
    // named the handle: the session is retired here, because a caller that never
    // learned the handle cannot retire it.
    for _ in 0..ADAPTER_LIMITS.max_open_sessions {
        let request = encoded(&open_request(AdapterCapability::Read, &fixture, &binding));
        let served = serve_open(&registry, AdapterCapability::Read, &request);
        let handle = handle_of(&document(served.response));
        assert_eq!(served.opened, Some(handle), "an opened session is unowned");
        assert!(registry.abandon(handle), "{handle} was not retired");
        assert!(!registry.abandon(handle), "{handle} was retired twice");
    }

    // A refused open owns nothing to retire and spends no slot.
    let crossed = encoded(&open_request(AdapterCapability::Read, &fixture, &binding));
    let refused = serve_open(&registry, AdapterCapability::Maintenance, &crossed);
    assert_eq!(refused.opened, None);
    assert_eq!(
        reason_of(&document(refused.response)),
        "capability-mismatch"
    );

    // Every slot an abandoned session held is available again, and the session
    // that was never abandoned still answers.
    let reopened: Vec<i64> = (1..ADAPTER_LIMITS.max_open_sessions)
        .map(|_| {
            handle_of(&open(
                &registry,
                AdapterCapability::Read,
                &fixture,
                &binding,
            ))
        })
        .collect();
    let exhausted = open(&registry, AdapterCapability::Read, &fixture, &binding);
    assert_eq!(reason_of(&exhausted), "sessions-exhausted");
    for handle in std::iter::once(kept).chain(reopened) {
        let response = read(&registry, handle, &binding, &json!({"kind": "status"}));
        assert_eq!(result_of(&response)["files_indexed"], 3, "{response}");
    }
}

#[test]
fn one_file_is_refreshed_without_pruning_the_rest_and_a_read_never_scans() {
    let fixture = Fixture::new("one");
    let registry = Registry::new();
    let binding = binding(SOURCE, GENERATION);
    let maintainer = handle_of(&open(
        &registry,
        AdapterCapability::Maintenance,
        &fixture,
        &binding,
    ));
    assert_eq!(
        result_of(&maintain(
            &registry,
            maintainer,
            &binding,
            &json!({"kind": "index"})
        ))["files_indexed"],
        3
    );

    let reader = handle_of(&open(
        &registry,
        AdapterCapability::Read,
        &fixture,
        &binding,
    ));
    assert_eq!(
        indexed_files(&registry, reader, &binding),
        owned(FIXTURE_FILES)
    );

    // A file on disk that no maintenance call has covered stays unindexed: reads
    // answer from the index, they do not go looking.
    fixture.write("gamma.org", "#+title: Gamma\n");
    assert_eq!(
        indexed_files(&registry, reader, &binding),
        owned(FIXTURE_FILES),
        "a read operation scanned the root"
    );

    let updated = maintain(
        &registry,
        maintainer,
        &binding,
        &json!({"kind": "indexFile", "file_path": "gamma.org"}),
    );
    assert_eq!(result_of(&updated)["file_path"], "gamma.org");
    let mut expected = owned(FIXTURE_FILES);
    expected.push("gamma.org".to_owned());
    expected.sort();
    assert_eq!(indexed_files(&registry, reader, &binding), expected);

    // Removing one file's index leaves every other file indexed.
    fixture.remove("gamma.org");
    let removed = maintain(
        &registry,
        maintainer,
        &binding,
        &json!({"kind": "indexFile", "file_path": "gamma.org"}),
    );
    assert_eq!(result_of(&removed)["file_path"], "gamma.org");
    assert_eq!(
        indexed_files(&registry, reader, &binding),
        owned(FIXTURE_FILES)
    );
}

#[test]
fn an_engine_refusal_carries_its_code_and_none_of_the_request() {
    let fixture = Fixture::new("one");
    let registry = Registry::new();
    let binding = binding(SOURCE, GENERATION);
    // Opened before the envelope exists: a canonical engine cannot index a
    // corpus the engine refuses.
    let mut canonical = fixture.canonical();
    let maintainer = handle_of(&open(
        &registry,
        AdapterCapability::Maintenance,
        &fixture,
        &binding,
    ));

    fixture.write(ENVELOPE, "sealed bytes\n");
    let refused = serve_maintenance(
        &registry,
        &encoded(&request(maintainer, &binding, &json!({"kind": "index"}))),
    );
    let declined = document(refused.clone());
    assert_eq!(reason_of(&declined), "engine-refused");
    assert_eq!(declined["engine"]["code"], REFUSED);
    assert_eq!(declined["engine"]["kind"], "invalid-params");
    assert_absent(&refused, &["gpg", ENVELOPE, "sealed"]);

    // The positive control: the engine's own diagnostic does name both.
    let quoted = engine_message(&mut canonical, METHOD_INDEX, json!({}));
    assert!(
        quoted.contains("gpg") && quoted.contains(ENVELOPE),
        "the engine no longer quotes the input this case redacts"
    );

    fixture.remove(ENVELOPE);
    let reader = handle_of(&open(
        &registry,
        AdapterCapability::Read,
        &fixture,
        &binding,
    ));
    let refused = serve_read(
        &registry,
        &encoded(&request(
            reader,
            &binding,
            &json!({"kind": "readNodeSource", "node_key": SECRET}),
        )),
    );
    let unknown = document(refused.clone());
    assert_eq!(reason_of(&unknown), "engine-refused");
    assert_eq!(unknown["engine"]["code"], REFUSED);
    assert_eq!(unknown["engine"]["kind"], "not-found");
    assert_absent(&refused, &[SECRET]);

    let quoted = engine_message(
        &mut canonical,
        METHOD_READ_NODE_SOURCE,
        json!({"node_key": SECRET}),
    );
    assert!(
        quoted.contains(SECRET),
        "the engine no longer quotes the request this case redacts"
    );

    let outside = format!("/{SECRET}.org");
    let failed = serve_maintenance(
        &registry,
        &encoded(&request(
            maintainer,
            &binding,
            &json!({"kind": "indexFile", "file_path": outside}),
        )),
    );
    let unfinished = document(failed.clone());
    assert_eq!(reason_of(&unfinished), "engine-failed");
    assert_eq!(unfinished["engine"]["code"], FAILED);
    assert_absent(&failed, &[SECRET, "resolve"]);

    let quoted = engine_message(
        &mut canonical,
        METHOD_INDEX_FILE,
        json!({"file_path": outside}),
    );
    assert!(
        quoted.contains("resolve"),
        "the engine no longer explains the failure this case reduces to a code"
    );
}

#[test]
fn a_panic_inside_a_call_is_contained_as_a_refusal() {
    let refusal = document(contained(|| panic!("a native call failed")));
    assert_eq!(reason_of(&refusal), "panicked");
    assert_eq!(refusal["bound"], Value::Null);
    assert_eq!(refusal["engine"], Value::Null);

    // The positive control: containment returns what the call answered.
    assert_eq!(document(contained(serve_contract))["outcome"], "contract");
}

/// Inspect child-process diagnostics, including the default panic hook and backtrace.
#[test]
fn no_diagnostic_of_a_failing_call_reaches_the_output_of_its_process() {
    let binary = env::current_exe().expect("this test binary has a path");
    let child = Command::new(&binary)
        .args([DRIVEN, "--exact", "--ignored", "--nocapture"])
        .env("RUST_BACKTRACE", "full")
        .output()
        .unwrap_or_else(|error| panic!("failed to run {}: {error}", binary.display()));
    let printed = String::from_utf8_lossy(&child.stdout).into_owned();
    let logged = String::from_utf8_lossy(&child.stderr).into_owned();

    assert!(child.status.success(), "{}", redacted(&logged));
    assert!(printed.contains(REPORTED), "{}", redacted(&printed));
    assert!(
        logged.contains("panicked at") && logged.contains("stack backtrace"),
        "neither the contained panic nor its backtrace reached the process output"
    );
    for stream in [&printed, &logged] {
        for fragment in [SECRET, "ghp_"] {
            assert!(
                !stream.contains(fragment),
                "a diagnostic repeats {} bytes of the request",
                fragment.len()
            );
        }
    }
}

/// Every route out of a failing call, written where the test above reads it.
#[test]
#[ignore = "driven as a child process by the test above"]
fn every_failing_route_writes_its_diagnostic_where_that_process_reads() {
    let fixture = Fixture::new("child");
    let registry = Registry::new();
    let held = binding(SOURCE, GENERATION);
    let named = binding(OTHER_SOURCE, SECRET);
    let reader = handle_of(&open(&registry, AdapterCapability::Read, &fixture, &held));
    let maintainer = handle_of(&open(
        &registry,
        AdapterCapability::Maintenance,
        &fixture,
        &held,
    ));
    let absent = json!({"kind": "readNodeSource", "node_key": SECRET});

    let unopenable = json!({
        "version": ADAPTER_PROTOCOL_VERSION,
        "capability": AdapterCapability::Read,
        "binding": named,
        "context": {
            "root": text(&fixture.root().join(SECRET)),
            "database": text(&fixture.database()),
        },
    });
    reported(
        "invalid-context",
        serve_open(&registry, AdapterCapability::Read, &encoded(&unopenable)).response,
    );
    reported(
        "malformed-request",
        serve_open(
            &registry,
            AdapterCapability::Read,
            format!("{{\"version\": 1, \"root\": \"{SECRET}\"}}").as_bytes(),
        )
        .response,
    );
    reported(
        "unknown-operation",
        serve_read(
            &registry,
            &encoded(&request(reader, &held, &json!({"kind": SECRET}))),
        ),
    );
    reported(
        "out-of-bounds",
        serve_read(
            &registry,
            &encoded(&request(
                reader,
                &held,
                &json!({
                    "kind": "searchNodes",
                    "query": SECRET,
                    "limit": ADAPTER_LIMITS.max_page_entries + 1,
                }),
            )),
        ),
    );
    reported(
        "binding-mismatch",
        serve_read(&registry, &encoded(&request(reader, &named, &absent))),
    );
    reported(
        "engine-refused",
        serve_read(&registry, &encoded(&request(reader, &held, &absent))),
    );
    reported(
        "engine-failed",
        serve_maintenance(
            &registry,
            &encoded(&request(
                maintainer,
                &held,
                &json!({"kind": "indexFile", "file_path": format!("/{SECRET}.org")}),
            )),
        ),
    );
    reported(
        "unknown-handle",
        serve_close(
            &registry,
            &encoded(&json!({
                "version": ADAPTER_PROTOCOL_VERSION,
                "handle": i64::MAX,
                "binding": named,
            })),
        ),
    );

    // A panic with a request in flight, which the process hook reports as well
    // as the refusal does.
    let in_flight = encoded(&request(reader, &held, &absent));
    reported(
        "panicked",
        contained(|| {
            let _request = in_flight;
            panic!("a native call failed")
        }),
    );

    // The positive control: the same routes carry an answer that is not a refusal.
    let contract = document(serve_contract());
    assert_eq!(contract["outcome"], "contract", "{contract}");
    println!("{contract}");
    println!("{REPORTED}");
}

/// Writes one refusal down every route a caller has to it.
fn reported(expected: &str, response: Vec<u8>) {
    let refusal = document(response.clone());
    assert_eq!(reason_of(&refusal), expected, "{refusal}");
    let decoded: AdapterResponse =
        serde_json::from_slice(&response).expect("a refusal decodes as one");
    let AdapterOutcome::Refused(refused) = decoded.outcome else {
        panic!("{expected} is refused");
    };
    let answered = String::from_utf8(response).expect("an answer is UTF-8");
    println!("{answered}\n{refused}\n{refused:?}\n{refusal}");
    eprintln!("{answered}\n{refused}\n{refused:?}\n{refusal}");
}

/// Child output with the synthetic credential taken out, so reporting a failure
/// of this test does not print what the test is looking for.
fn redacted(output: &str) -> String {
    output
        .replace(SECRET, "<request>")
        .replace("ghp_", "<shape>")
}

/// The synthetic corpus and the paths one session is opened against.
struct Fixture {
    directory: TempDir,
    mark: String,
}

impl Fixture {
    /// Writes the corpus with `mark` in every title, so an answer names the
    /// source it came from.
    fn new(mark: &str) -> Self {
        let directory = tempfile::tempdir().expect("a temporary directory");
        let fixture = Self {
            directory,
            mark: mark.to_owned(),
        };
        fs::create_dir(fixture.root()).expect("the corpus root is created");
        fixture.write(
            "alpha.org",
            &format!(
                "#+title: {}\n\n* {}\n:PROPERTIES:\n:ID: alpha-first\n:END:\nSee [[id:beta-target][Beta]].\n",
                fixture.title("Alpha"),
                fixture.title("First heading"),
            ),
        );
        fixture.write(
            "beta.org",
            &format!(
                "#+title: {}\n\n* {}\n:PROPERTIES:\n:ID: beta-target\n:END:\nTarget body.\n",
                fixture.title("Beta"),
                fixture.title("Target heading"),
            ),
        );
        fixture.write(
            "riemann.org",
            &format!(
                "#+title: {}\n#+glossary: t\n:PROPERTIES:\n:ID:              riemann-integral\n:GLOSSARY_STATUS: confirmed\n:END:\n\nA definite integral.\n",
                fixture.title("Riemann integral"),
            ),
        );
        fixture
    }

    fn title(&self, stem: &str) -> String {
        format!("{stem} {}", self.mark)
    }

    fn root(&self) -> PathBuf {
        self.directory.path().join("notes")
    }

    fn database(&self) -> PathBuf {
        self.directory.path().join("session.sqlite")
    }

    fn context(&self) -> Value {
        json!({"root": text(&self.root()), "database": text(&self.database())})
    }

    fn write(&self, name: &str, body: &str) {
        let path = self.root().join(name);
        fs::write(&path, body)
            .unwrap_or_else(|error| panic!("failed to write {}: {error}", path.display()));
    }

    fn remove(&self, name: &str) {
        let path = self.root().join(name);
        fs::remove_file(&path)
            .unwrap_or_else(|error| panic!("failed to remove {}: {error}", path.display()));
    }

    /// An engine on the same corpus with a database of its own, indexed once:
    /// the answer the adapter is held against.
    fn canonical(&self) -> SlipboxService {
        let mut service = SlipboxService::with_platform(
            self.root(),
            self.directory.path().join("canonical.sqlite"),
            Vec::new(),
            DiscoveryPolicy::default(),
            PlatformPolicy::headless(),
        )
        .expect("a canonical engine opens on the fixture");
        service
            .invoke_value(METHOD_INDEX, json!({}))
            .expect("the canonical engine indexes the fixture");
        service
    }
}

/// The node keys the engine gives the fixture identifiers, which are the engine's
/// to spell rather than this test's to guess.
struct Keys {
    alpha: String,
    beta: String,
    term: String,
}

impl Keys {
    fn of(canonical: &mut SlipboxService) -> Self {
        Self {
            alpha: key_of(canonical, "alpha-first"),
            beta: key_of(canonical, "beta-target"),
            term: key_of(canonical, "riemann-integral"),
        }
    }
}

fn key_of(canonical: &mut SlipboxService, id: &str) -> String {
    let node = canonical
        .invoke_value(METHOD_NODE_FROM_ID, json!({"id": id}))
        .unwrap_or_else(|error| panic!("the engine refused to resolve {id}: {error}"));
    node["node_key"]
        .as_str()
        .unwrap_or_else(|| panic!("{id} resolves to no indexed node"))
        .to_owned()
}

/// Every read operation but `status`, with the canonical method and payload the
/// adapter must dispatch it to. `status` reports the session's own root and
/// database, so it is held against those instead.
fn read_cases(keys: &Keys) -> Vec<(Value, &'static str, Value)> {
    let Keys { alpha, beta, term } = keys;
    vec![
        (
            json!({"kind": "indexedFiles"}),
            METHOD_INDEXED_FILES,
            json!({}),
        ),
        (
            json!({"kind": "searchNodes", "query": "target", "limit": 20, "sort": "title"}),
            METHOD_SEARCH_NODES,
            json!({"query": "target", "limit": 20, "sort": "title"}),
        ),
        (
            json!({"kind": "searchNodeContent", "query": "body", "limit": 20}),
            METHOD_SEARCH_NODE_CONTENT,
            json!({"query": "body", "limit": 20}),
        ),
        (
            json!({"kind": "nodeFromId", "id": "beta-target"}),
            METHOD_NODE_FROM_ID,
            json!({"id": "beta-target"}),
        ),
        (
            json!({"kind": "nodeFromKey", "node_key": beta}),
            METHOD_NODE_FROM_KEY,
            json!({"node_key": beta}),
        ),
        (
            json!({"kind": "readNodeSource", "node_key": beta, "context_before": 2, "context_after": 2, "max_lines": 400}),
            METHOD_READ_NODE_SOURCE,
            json!({"node_key": beta, "context_before": 2, "context_after": 2, "max_lines": 400}),
        ),
        (
            json!({"kind": "listGlossaryTerms", "limit": 20}),
            METHOD_LIST_GLOSSARY_TERMS,
            json!({"limit": 20}),
        ),
        (
            json!({"kind": "searchGlossary", "query": "riemann", "limit": 20}),
            METHOD_SEARCH_GLOSSARY,
            json!({"query": "riemann", "limit": 20}),
        ),
        (
            json!({"kind": "glossaryTerm", "node_key": term}),
            METHOD_GLOSSARY_TERM,
            json!({"node_key": term}),
        ),
        (
            json!({"kind": "backlinks", "node_key": beta, "limit": 50, "unique": true}),
            METHOD_BACKLINKS,
            json!({"node_key": beta, "limit": 50, "unique": true}),
        ),
        (
            json!({"kind": "forwardLinks", "node_key": alpha, "limit": 50}),
            METHOD_FORWARD_LINKS,
            json!({"node_key": alpha, "limit": 50}),
        ),
        (
            json!({"kind": "explore", "node_key": alpha, "lens": "structure", "limit": 50, "unique": true}),
            METHOD_EXPLORE,
            json!({"node_key": alpha, "lens": "structure", "limit": 50, "unique": true}),
        ),
    ]
}

fn binding(source: &str, generation: &str) -> Value {
    json!({"source": source, "generation": generation})
}

fn request(handle: i64, binding: &Value, operation: &Value) -> Value {
    json!({
        "version": ADAPTER_PROTOCOL_VERSION,
        "handle": handle,
        "binding": binding,
        "operation": operation,
    })
}

fn open_request(capability: AdapterCapability, fixture: &Fixture, binding: &Value) -> Value {
    json!({
        "version": ADAPTER_PROTOCOL_VERSION,
        "capability": capability,
        "binding": binding,
        "context": fixture.context(),
    })
}

fn open(
    registry: &Registry,
    capability: AdapterCapability,
    fixture: &Fixture,
    binding: &Value,
) -> Value {
    let request = open_request(capability, fixture, binding);
    document(serve_open(registry, capability, &encoded(&request)).response)
}

fn read(registry: &Registry, handle: i64, binding: &Value, operation: &Value) -> Value {
    document(serve_read(
        registry,
        &encoded(&request(handle, binding, operation)),
    ))
}

fn maintain(registry: &Registry, handle: i64, binding: &Value, operation: &Value) -> Value {
    document(serve_maintenance(
        registry,
        &encoded(&request(handle, binding, operation)),
    ))
}

fn close(registry: &Registry, handle: i64, binding: &Value) -> Value {
    let request = json!({
        "version": ADAPTER_PROTOCOL_VERSION,
        "handle": handle,
        "binding": binding,
    });
    document(serve_close(registry, &encoded(&request)))
}

/// Indexes the fixture through a maintenance session that is then retired.
fn index(registry: &Registry, fixture: &Fixture, binding: &Value) {
    let handle = handle_of(&open(
        registry,
        AdapterCapability::Maintenance,
        fixture,
        binding,
    ));
    let indexed = maintain(registry, handle, binding, &json!({"kind": "index"}));
    assert!(
        result_of(&indexed)["files_indexed"]
            .as_u64()
            .is_some_and(|files| files >= 3),
        "{indexed}"
    );
    assert_eq!(close(registry, handle, binding)["retired"], true);
}

fn indexed_files(registry: &Registry, handle: i64, binding: &Value) -> Vec<String> {
    let response = read(registry, handle, binding, &json!({"kind": "indexedFiles"}));
    let mut files: Vec<String> = result_of(&response)["files"]
        .as_array()
        .expect("an indexed file list is an array")
        .iter()
        .map(|file| {
            file.as_str()
                .expect("an indexed file is named by a string")
                .to_owned()
        })
        .collect();
    files.sort();
    files
}

fn encoded(request: &Value) -> Vec<u8> {
    serde_json::to_vec(request).expect("a request document encodes")
}

/// Duplicate the version in text before any map can overwrite it.
fn ambiguous(request: &Value) -> Vec<u8> {
    let text = serde_json::to_string(request).expect("a request document encodes");
    let named_twice = text.replace(r#""version":1"#, r#""version":1,"version":2"#);
    assert_ne!(
        text, named_twice,
        "the canonical request names no version to repeat"
    );
    named_twice.into_bytes()
}

fn document(response: Vec<u8>) -> Value {
    let document: Value =
        serde_json::from_slice(&response).expect("the adapter answers one JSON document");
    assert_eq!(document["version"], ADAPTER_PROTOCOL_VERSION, "{document}");
    document
}

fn handle_of(response: &Value) -> i64 {
    assert_eq!(response["outcome"], "opened", "{response}");
    response["handle"]
        .as_i64()
        .expect("an opened session carries its identity")
}

/// The canonical result of an answer, which the answer carries under the
/// operation it answers.
fn result_of(response: &Value) -> &Value {
    assert_eq!(response["outcome"], "answered", "{response}");
    &response["answer"]["result"]
}

fn reason_of(response: &Value) -> &str {
    assert_eq!(response["outcome"], "refused", "{response}");
    response["reason"]
        .as_str()
        .expect("a refusal names its reason")
}

fn kind_of(operation: &Value) -> String {
    operation["kind"]
        .as_str()
        .expect("an operation names its kind")
        .to_owned()
}

fn kinds(operations: &Value) -> BTreeSet<String> {
    operations
        .as_array()
        .expect("a contract lists its operations")
        .iter()
        .map(|kind| {
            kind.as_str()
                .expect("an operation is named by a string")
                .to_owned()
        })
        .collect()
}

fn read_inventory() -> BTreeSet<String> {
    discriminants(READ_VOCABULARY)
}

fn discriminants(vocabulary: &[(&str, &str)]) -> BTreeSet<String> {
    vocabulary
        .iter()
        .map(|(discriminant, _)| (*discriminant).to_owned())
        .collect()
}

/// The engine's own message for a request it declines, which the adapter drops.
fn engine_message(canonical: &mut SlipboxService, method: &str, params: Value) -> String {
    canonical
        .invoke_value(method, params)
        .err()
        .map(|error| error.into_inner().message)
        .unwrap_or_else(|| panic!("the engine answered {method} instead of declining it"))
}

/// Neither the answer's bytes nor its text carry anything the request supplied.
fn assert_absent(answer: &[u8], quoted: &[&str]) {
    let text = String::from_utf8(answer.to_vec()).expect("an answer is UTF-8");
    for fragment in quoted {
        assert!(
            !text.contains(fragment),
            "the answer reflects {} bytes of the request it refused",
            fragment.len()
        );
    }
}

fn owned(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

fn resolved(path: &Path) -> PathBuf {
    path.canonicalize()
        .unwrap_or_else(|error| panic!("failed to canonicalize {}: {error}", path.display()))
}

fn text(path: &Path) -> String {
    path.to_str()
        .unwrap_or_else(|| panic!("{} is not UTF-8", path.display()))
        .to_owned()
}
