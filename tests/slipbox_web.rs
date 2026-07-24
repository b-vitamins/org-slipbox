use std::fs;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use serde_json::Value;
use slipbox_core::{
    BacklinksParams, ForwardLinksParams, GlossaryDueParams, GlossaryTermParams,
    ListGlossaryTermsParams, NodeFromIdParams, NodeFromKeyParams, NodeFromTitleOrAliasParams,
    NoteContextParams, ReflinksParams, SearchGlossaryParams, SearchNodesParams,
    UnlinkedReferencesParams,
};
use slipbox_daemon_client::DaemonServeConfig;
use slipbox_index::scan_root;
use slipbox_store::Database;
use slipbox_web::{ReadingBridge, ReadingServer};
use tempfile::{TempDir, tempdir};

fn daemon_binary() -> &'static str {
    env!("CARGO_BIN_EXE_slipbox")
}

/// Four files: a linked note pair, a weak peer sharing a ref, and one glossary
/// term. The index is built here, so the daemon under test only serves it.
fn build_reading_fixture() -> Result<(TempDir, PathBuf, PathBuf)> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;

    fs::write(
        root.join("alpha.org"),
        r#":PROPERTIES:
:ID: alpha-id
:ROAM_REFS: cite:shared2024 cite:alpha2024
:END:
#+title: Alpha
#+filetags: :alpha:shared:

See [[id:beta-id][Beta]].
"#,
    )?;
    fs::write(
        root.join("beta.org"),
        r#":PROPERTIES:
:ID: beta-id
:ROAM_REFS: cite:shared2024 cite:beta2024
:END:
#+title: Beta

Beta stands on its own.
"#,
    )?;
    fs::write(
        root.join("weak.org"),
        r#":PROPERTIES:
:ID: weak-id
:ROAM_REFS: cite:shared2024
:END:
#+title: Weak

Weakly integrated peer with shared references and no direct links.
Alpha appears here as unlinked text.
"#,
    )?;
    fs::write(
        root.join("riemann.org"),
        r#"#+title: Riemann integral
#+glossary: t
:PROPERTIES:
:ID:              riemann-id
:GLOSSARY_STATUS: confirmed
:SR_DUE:          2026-08-01
:SR_EASE:         2.50
:SR_INTERVAL:     6
:SR_REPS:         3
:SR_LAST:         2026-07-26
:END:

A definite integral defined as the limit of Riemann sums.
"#,
    )?;

    let files = scan_root(&root)?;
    let db = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&db)?;
    database.sync_index(&files)?;

    Ok((workspace, root, db))
}

#[test]
fn reading_bridge_serves_the_read_only_note_and_glossary_surface() -> Result<()> {
    let (_workspace, root, db) = build_reading_fixture()?;
    let bridge = ReadingBridge::spawn(daemon_binary(), DaemonServeConfig::new(&root, &db))?;

    // The bridge always spawns a read-only daemon, so every answer below is also
    // evidence the method passes the read-only dispatch guard.
    let ping = bridge.ping()?;
    assert_eq!(ping.root, root.canonicalize()?.display().to_string());

    let status = bridge.status()?;
    assert_eq!(status.files_indexed, 4);
    assert_eq!(status.nodes_indexed, 4);

    let alpha = bridge
        .search_nodes(&SearchNodesParams {
            query: "Alpha".to_owned(),
            limit: 10,
            sort: None,
        })?
        .nodes
        .into_iter()
        .find(|node| node.title == "Alpha")
        .context("Alpha should resolve from search")?;

    let random = bridge.random_node()?;
    assert!(random.node.is_some());

    let by_id = bridge
        .node_from_id(&NodeFromIdParams {
            id: "alpha-id".to_owned(),
        })?
        .context("Alpha should resolve by ID")?;
    assert_eq!(by_id.node_key, alpha.node_key);

    let by_key = bridge
        .node_from_key(&NodeFromKeyParams {
            node_key: alpha.node_key.clone(),
        })?
        .context("Alpha should resolve by key")?;
    assert_eq!(by_key.node_key, alpha.node_key);

    let by_title = bridge
        .node_from_title_or_alias(&NodeFromTitleOrAliasParams {
            title_or_alias: "Beta".to_owned(),
            nocase: false,
        })?
        .context("Beta should resolve by title")?;
    assert_eq!(by_title.title, "Beta");

    let context = bridge.note_context(&NoteContextParams {
        node_key: alpha.node_key.clone(),
        source_context_before: Some(0),
        source_context_after: Some(0),
        source_max_lines: Some(20),
        relation_limit: Some(10),
    })?;
    assert_eq!(context.note.title, "Alpha");
    assert!(context.source.content.contains("#+title: Alpha"));
    assert_eq!(context.forward_links.len(), 1);

    let backlinks = bridge.backlinks(&BacklinksParams {
        node_key: by_title.node_key.clone(),
        limit: 10,
        unique: false,
    })?;
    assert_eq!(backlinks.backlinks.len(), 1);
    assert_eq!(backlinks.backlinks[0].source_note.title, "Alpha");

    let forward_links = bridge.forward_links(&ForwardLinksParams {
        node_key: alpha.node_key.clone(),
        limit: 10,
        unique: false,
    })?;
    assert_eq!(forward_links.forward_links.len(), 1);
    assert_eq!(
        forward_links.forward_links[0].destination_note.title,
        "Beta"
    );

    let reflinks = bridge.reflinks(&ReflinksParams {
        node_key: alpha.node_key.clone(),
        limit: 10,
    })?;
    assert!(
        reflinks
            .reflinks
            .iter()
            .any(|record| record.source_anchor.title == "Weak")
    );

    let unlinked = bridge.unlinked_references(&UnlinkedReferencesParams {
        node_key: alpha.node_key.clone(),
        limit: 10,
    })?;
    assert!(
        unlinked
            .unlinked_references
            .iter()
            .any(|record| record.source_anchor.title == "Weak")
    );

    let terms = bridge.list_glossary_terms(&ListGlossaryTermsParams { limit: 50 })?;
    assert_eq!(terms.terms.len(), 1);
    let riemann_key = terms.terms[0].node_key.clone();
    assert_eq!(terms.terms[0].title, "Riemann integral");

    let searched = bridge.search_glossary(&SearchGlossaryParams {
        query: "Riemann".to_owned(),
        limit: 50,
    })?;
    assert_eq!(searched.terms.len(), 1);
    assert_eq!(searched.terms[0].node_key, riemann_key);

    let term = bridge
        .glossary_term(&GlossaryTermParams {
            node_key: riemann_key.clone(),
        })?
        .term
        .context("known glossary term should resolve")?;
    assert_eq!(term.sr_due.as_deref(), Some("2026-08-01"));

    // The fixture term's `SR_DUE` is 2026-08-01, so these two dates straddle it.
    let not_yet = bridge.glossary_due(&GlossaryDueParams {
        today: Some("2026-07-26".to_owned()),
        limit: 50,
    })?;
    assert!(not_yet.terms.is_empty());
    let due = bridge.glossary_due(&GlossaryDueParams {
        today: Some("2026-08-02".to_owned()),
        limit: 50,
    })?;
    assert_eq!(due.terms.len(), 1);
    assert_eq!(due.terms[0].node_key, riemann_key);

    bridge.shutdown()?;
    Ok(())
}

#[test]
fn reading_bridge_serves_concurrent_readers_over_one_pipe() -> Result<()> {
    let (_workspace, root, db) = build_reading_fixture()?;
    let bridge = Arc::new(ReadingBridge::spawn(
        daemon_binary(),
        DaemonServeConfig::new(&root, &db),
    )?);

    // Each reader asks for a different note, so an answer delivered to the wrong
    // thread fails an assertion. Readers all asking for the same note would pass
    // even if the pipe handed every response to whichever thread arrived next.
    let asked = [
        ("alpha-id", "Alpha"),
        ("beta-id", "Beta"),
        ("weak-id", "Weak"),
        ("riemann-id", "Riemann integral"),
    ];
    let mut handles = Vec::new();
    for reader in 0..16 {
        let bridge = Arc::clone(&bridge);
        let (id, title) = asked[reader % asked.len()];
        handles.push(thread::spawn(move || -> Result<()> {
            for _ in 0..8 {
                let node = bridge
                    .node_from_id(&NodeFromIdParams { id: id.to_owned() })?
                    .with_context(|| format!("{id} should resolve by ID"))?;
                assert_eq!(node.title, title, "reader asked for {id}");
            }
            Ok(())
        }));
    }
    for handle in handles {
        handle.join().expect("reader thread should not panic")?;
    }

    Arc::into_inner(bridge)
        .context("all reader threads have finished, so the bridge is uniquely owned")?
        .shutdown()?;
    Ok(())
}

/// One parsed HTTP response from the reading server.
struct HttpResponse {
    status: u16,
    body: String,
}

impl HttpResponse {
    /// Parse the response body as JSON, failing the test on malformed output.
    fn json(&self) -> Result<Value> {
        serde_json::from_str(&self.body)
            .with_context(|| format!("response body should be JSON: {}", self.body))
    }
}

/// A minimal HTTP/1.1 client, so the suite needs no HTTP-client dependency.
///
/// The reading server frames every response with `Content-Length` and honours
/// `Connection: close`, so reading the socket to EOF yields exactly one response
/// with no chunked-transfer decoding to do.
fn http_get(addr: SocketAddr, target: &str) -> Result<HttpResponse> {
    http_request(addr, "GET", target)
}

fn http_request(addr: SocketAddr, method: &str, target: &str) -> Result<HttpResponse> {
    http_send(addr, method, target, None)
}

fn http_head_with(
    addr: SocketAddr,
    method: &str,
    target: &str,
    header: &str,
) -> Result<HttpResponse> {
    http_send(addr, method, target, Some(header))
}

fn http_send(
    addr: SocketAddr,
    method: &str,
    target: &str,
    header: Option<&str>,
) -> Result<HttpResponse> {
    let mut stream = TcpStream::connect(addr)?;
    write!(stream, "{method} {target} HTTP/1.1\r\nHost: localhost\r\n")?;
    if let Some(header) = header {
        write!(stream, "{header}\r\n")?;
    }
    write!(stream, "Connection: close\r\n\r\n")?;
    stream.flush()?;
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw)?;
    parse_response(&raw)
}

/// Split a raw HTTP response into its status code and body.
fn parse_response(raw: &[u8]) -> Result<HttpResponse> {
    let text = String::from_utf8(raw.to_vec()).context("response should be UTF-8")?;
    let (head, body) = text
        .split_once("\r\n\r\n")
        .context("response should have a header/body separator")?;
    let status_line = head
        .lines()
        .next()
        .context("response should have a status line")?;
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse::<u16>().ok())
        .ok_or_else(|| anyhow!("status line should carry a numeric code: {status_line}"))?;
    Ok(HttpResponse {
        status,
        body: body.to_owned(),
    })
}

/// Start a reading server over the fixture on an ephemeral loopback port.
fn start_reading_server(root: &PathBuf, db: &PathBuf) -> Result<ReadingServer> {
    let bridge = ReadingBridge::spawn(daemon_binary(), DaemonServeConfig::new(root, db))?;
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    ReadingServer::start(addr, bridge, 4).map_err(Into::into)
}

#[test]
fn reading_server_serves_the_note_and_glossary_surface_over_http() -> Result<()> {
    let (_workspace, root, db) = build_reading_fixture()?;
    let server = start_reading_server(&root, &db)?;
    let addr = server.local_addr();

    let health = http_get(addr, "/api/healthz")?;
    assert_eq!(health.status, 200);
    assert_eq!(health.json()?["status"], "ok");

    let status = http_get(addr, "/api/status")?;
    assert_eq!(status.status, 200);
    assert_eq!(status.json()?["nodes_indexed"], 4);

    let search = http_get(addr, "/api/search/nodes?q=Alpha&limit=10")?;
    assert_eq!(search.status, 200);
    let alpha_key = search.json()?["nodes"]
        .as_array()
        .and_then(|nodes| nodes.iter().find(|node| node["title"] == "Alpha"))
        .map(|node| node["node_key"].as_str().unwrap_or_default().to_owned())
        .context("Alpha should appear in search results")?;

    let by_id = http_get(addr, "/api/node?id=alpha-id")?;
    assert_eq!(by_id.status, 200);
    assert_eq!(by_id.json()?["node_key"], alpha_key);

    let by_key = http_get(addr, &format!("/api/node?key={}", encode(&alpha_key)))?;
    assert_eq!(by_key.status, 200);
    assert_eq!(by_key.json()?["title"], "Alpha");

    let by_title = http_get(addr, "/api/node?title=Beta")?;
    assert_eq!(by_title.status, 200);
    let beta_key = by_title.json()?["node_key"]
        .as_str()
        .context("Beta should resolve by title")?
        .to_owned();

    // "sums" lives in Riemann's body, never in a title or ref, so the two search
    // routes cannot both answer it: content search matches bodies, node search
    // metadata only.
    let content = http_get(addr, "/api/search/content?q=sums&limit=10")?;
    assert_eq!(content.status, 200);
    let top = content.json()?["hits"][0].clone();
    assert_eq!(top["node"]["title"], "Riemann integral");
    assert!(
        top["snippet"]["segments"]
            .as_array()
            .context("a content hit carries snippet segments")?
            .iter()
            .any(|segment| segment["matched"] == true),
        "the excerpt highlights the matched term"
    );
    let node_only = http_get(addr, "/api/search/nodes?q=sums&limit=10")?;
    assert_eq!(node_only.status, 200);
    assert_eq!(
        node_only.json()?["nodes"].as_array().map(Vec::len),
        Some(0),
        "node search matches metadata only, so a body-only word finds nothing"
    );

    let context = http_get(
        addr,
        &format!(
            "/api/note/context?key={}&before=0&after=0",
            encode(&alpha_key)
        ),
    )?;
    assert_eq!(context.status, 200);
    let context_body = context.json()?;
    assert_eq!(context_body["note"]["title"], "Alpha");
    assert_eq!(
        context_body["forward_links"].as_array().map(Vec::len),
        Some(1)
    );

    let backlinks = http_get(addr, &format!("/api/backlinks?key={}", encode(&beta_key)))?;
    assert_eq!(backlinks.status, 200);
    assert_eq!(
        backlinks.json()?["backlinks"][0]["source_note"]["title"],
        "Alpha"
    );

    let forward = http_get(
        addr,
        &format!("/api/forward-links?key={}", encode(&alpha_key)),
    )?;
    assert_eq!(forward.status, 200);
    assert_eq!(
        forward.json()?["forward_links"][0]["destination_note"]["title"],
        "Beta"
    );

    let reflinks = http_get(addr, &format!("/api/reflinks?key={}", encode(&alpha_key)))?;
    assert_eq!(reflinks.status, 200);
    assert!(reflinks.body.contains("Weak"));

    let unlinked = http_get(
        addr,
        &format!("/api/unlinked-references?key={}", encode(&alpha_key)),
    )?;
    assert_eq!(unlinked.status, 200);
    assert!(unlinked.body.contains("Weak"));

    let random = http_get(addr, "/api/random")?;
    assert_eq!(random.status, 200);
    assert!(random.json()?["node"].is_object());

    let terms = http_get(addr, "/api/glossary/terms")?;
    assert_eq!(terms.status, 200);
    let riemann_key = terms.json()?["terms"][0]["node_key"]
        .as_str()
        .context("the one glossary term should be listed")?
        .to_owned();

    let glossary_search = http_get(addr, "/api/glossary/search?q=Riemann")?;
    assert_eq!(glossary_search.status, 200);
    assert_eq!(glossary_search.json()?["terms"][0]["node_key"], riemann_key);

    let term = http_get(
        addr,
        &format!("/api/glossary/term?key={}", encode(&riemann_key)),
    )?;
    assert_eq!(term.status, 200);
    assert_eq!(term.json()?["term"]["sr_due"], "2026-08-01");

    let not_due = http_get(addr, "/api/glossary/due?today=2026-07-26")?;
    assert_eq!(not_due.status, 200);
    assert_eq!(not_due.json()?["terms"].as_array().map(Vec::len), Some(0));
    let due = http_get(addr, "/api/glossary/due?today=2026-08-02")?;
    assert_eq!(due.status, 200);
    assert_eq!(due.json()?["terms"][0]["node_key"], riemann_key);

    server.shutdown()?;
    Ok(())
}

#[test]
fn reading_server_walks_a_hop_bounded_neighborhood() -> Result<()> {
    let (_workspace, root, db) = build_reading_fixture()?;
    let server = start_reading_server(&root, &db)?;
    let addr = server.local_addr();

    let alpha = http_get(addr, "/api/node?id=alpha-id")?.json()?;
    let alpha_key = alpha["node_key"]
        .as_str()
        .context("Alpha has a key")?
        .to_owned();

    let neighborhood = http_get(
        addr,
        &format!("/api/neighborhood?key={}", encode(&alpha_key)),
    )?;
    assert_eq!(neighborhood.status, 200);
    let body = neighborhood.json()?;

    assert_eq!(body["origin"], alpha_key);
    assert_eq!(body["hops"], 1);
    let nodes = body["nodes"].as_array().context("neighborhood has nodes")?;
    assert!(
        nodes
            .iter()
            .any(|node| node["node"]["title"] == "Alpha" && node["distance"] == 0)
    );
    assert!(
        nodes
            .iter()
            .any(|node| node["node"]["title"] == "Beta" && node["distance"] == 1)
    );
    assert!(
        body["edges"]
            .as_array()
            .context("neighborhood has edges")?
            .iter()
            .any(|edge| edge["kind"] == "forward")
    );
    // 32 is the route's default fan-out, which Alpha's one link fits well inside.
    assert_eq!(body["fanout"], 32);
    assert_eq!(body["truncated"], false);

    // A fan-out of one admits Alpha's single link and fills the budget doing it.
    // The walk cannot tell that from a second link it never asked for, so the
    // reply reports `truncated` and echoes the bound that did the cutting.
    let bounded = http_get(
        addr,
        &format!("/api/neighborhood?key={}&fanout=1", encode(&alpha_key)),
    )?;
    assert_eq!(bounded.status, 200);
    let bounded_body = bounded.json()?;
    assert_eq!(bounded_body["fanout"], 1);
    assert_eq!(bounded_body["truncated"], true);

    server.shutdown()?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn reading_server_recovers_from_a_daemon_that_died_under_it() -> Result<()> {
    let (_workspace, root, db) = build_reading_fixture()?;
    let server = start_reading_server(&root, &db)?;
    let addr = server.local_addr();

    assert_eq!(http_get(addr, "/api/status")?.status, 200);
    kill_daemon_serving(&db)?;

    // The next read finds a dead pipe; the bridge kept the spawn recipe, so it
    // starts a replacement daemon and answers from that.
    let recovered = http_get(addr, "/api/node?id=alpha-id")?;
    assert_eq!(recovered.status, 200);
    assert_eq!(recovered.json()?["title"], "Alpha");
    assert_eq!(http_get(addr, "/api/status")?.json()?["nodes_indexed"], 4);

    server.shutdown()?;
    Ok(())
}

/// Kill the `slipbox serve` daemon working against `db`. The database path is
/// unique per fixture, so this reaches no other test's daemon.
#[cfg(unix)]
fn kill_daemon_serving(db: &Path) -> Result<()> {
    let pattern = db.to_str().context("the fixture database path is UTF-8")?;
    let killed = Command::new("pkill")
        .args(["-KILL", "-f", pattern])
        .status()?;
    if !killed.success() {
        bail!("no daemon was serving {pattern}");
    }
    // The pipe closes only as the kernel reaps the child, so wait for the
    // process to be gone rather than racing the next request against it.
    for _ in 0..200 {
        if !Command::new("pgrep")
            .args(["-f", pattern])
            .status()?
            .success()
        {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(20));
    }
    bail!("the daemon serving {pattern} did not exit")
}

#[test]
fn reading_server_maps_failures_to_http_status_codes() -> Result<()> {
    let (_workspace, root, db) = build_reading_fixture()?;
    let server = start_reading_server(&root, &db)?;
    let addr = server.local_addr();

    assert_eq!(http_get(addr, "/api/nope")?.status, 404);
    assert_eq!(http_get(addr, "/api/node")?.status, 400);
    assert_eq!(http_get(addr, "/api/node?id=does-not-exist")?.status, 404);
    assert_eq!(
        http_get(addr, "/api/glossary/term?key=file:missing.org")?.status,
        404
    );
    assert_eq!(
        http_get(addr, "/api/search/nodes?q=abc&limit=big")?.status,
        400
    );
    assert_eq!(
        http_get(addr, "/api/search/nodes?q=abc&sort=sideways")?.status,
        400
    );
    // A non-GET verb is refused before routing.
    assert_eq!(http_request(addr, "POST", "/api/status")?.status, 405);

    let error = http_get(addr, "/api/node")?;
    assert_eq!(error.json()?["error"]["kind"], "bad-request");

    server.shutdown()?;
    Ok(())
}

#[test]
fn reading_server_refuses_a_parameter_it_cannot_honour_as_written() -> Result<()> {
    let (_workspace, root, db) = build_reading_fixture()?;
    let server = start_reading_server(&root, &db)?;
    let addr = server.local_addr();

    // A target outside each of the routes' bounds: search limit 1..=200,
    // relation limit 1..=1000, hops 1..=3, fanout 1..=200, max_lines 1..=1000,
    // before 0..=200. Every one is a 400, never a clamp.
    for target in [
        "/api/search/nodes?q=alpha&limit=0",
        "/api/search/nodes?q=alpha&limit=201",
        "/api/backlinks?key=file:alpha.org&limit=1001",
        "/api/neighborhood?key=file:alpha.org&hops=0",
        "/api/neighborhood?key=file:alpha.org&hops=9",
        "/api/neighborhood?key=file:alpha.org&fanout=0",
        "/api/neighborhood?key=file:alpha.org&fanout=201",
        "/api/note/context?key=file:alpha.org&max_lines=1001",
        "/api/note/context?key=file:alpha.org&before=201",
    ] {
        let refused = http_get(addr, target)?;
        assert_eq!(refused.status, 400, "{target} should be refused");
        assert_eq!(refused.json()?["error"]["kind"], "bad-request");
    }

    // A parameter present with no value reports as its own fault, distinct from
    // one left out entirely.
    let empty = http_get(addr, "/api/node?id=")?;
    assert_eq!(empty.status, 400);
    assert!(
        empty.json()?["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("given with no value")),
        "{}",
        empty.body
    );

    // `%FF` is not valid UTF-8, so it cannot be decoded into a query at all.
    assert_eq!(http_get(addr, "/api/search/nodes?q=%FF")?.status, 400);

    // SQLite reads a query as a NUL-terminated string, so a NUL cannot survive
    // the trip to the index. Refused here, it is the caller's 400 rather than an
    // upstream fault reported against the server.
    for target in [
        "/api/search/nodes?q=alpha%00beta",
        "/api/search/content?q=alpha%00beta",
        "/api/glossary/search?q=alpha%00beta",
        "/api/node?key=file:alpha.org%00",
    ] {
        let refused = http_get(addr, target)?;
        assert_eq!(refused.status, 400, "{target} should be refused");
        assert_eq!(refused.json()?["error"]["kind"], "bad-request");
    }

    // The due predicate compares ISO date strings, so a non-date `today` does
    // not fail downstream: it answers with the wrong set of terms.
    for target in [
        "/api/glossary/due?today=garbage",
        "/api/glossary/due?today=2026-13-01",
        "/api/glossary/due?today=2026-02-30",
    ] {
        assert_eq!(http_get(addr, target)?.status, 400, "{target}");
    }

    // The FTS indexes match on words of two characters or more, so a shorter
    // query would list arbitrary notes rather than matches. The bound counts
    // characters, not bytes: one three-byte CJK character is refused too.
    for target in [
        "/api/search/nodes?q=a",
        "/api/search/content?q=a",
        "/api/glossary/search?q=a",
        "/api/search/nodes?q=%E7%8C%AB",
    ] {
        let refused = http_get(addr, target)?;
        assert_eq!(refused.status, 400, "{target} should be refused");
        assert!(
            refused.json()?["error"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("at least 2 characters")),
            "{}",
            refused.body
        );
    }

    // Two characters is at the bound, not past it, so a query the length of a
    // technical acronym reaches the index.
    for target in [
        "/api/search/nodes?q=KL",
        "/api/search/content?q=KL",
        "/api/glossary/search?q=KL",
    ] {
        assert_eq!(http_get(addr, target)?.status, 200, "{target}");
    }

    server.shutdown()?;
    Ok(())
}

#[test]
fn reading_server_refuses_a_request_that_declares_a_body() -> Result<()> {
    let (_workspace, root, db) = build_reading_fixture()?;
    let server = start_reading_server(&root, &db)?;
    let addr = server.local_addr();

    // A reading route consumes no body, so a declaration is refused with 413 on
    // its own strength: the announced length is never read or allocated against,
    // which is why `i64::MAX` and a value past `u64` are refused the same way.
    for declaration in [
        "Content-Length: 9223372036854775807",
        "Content-Length: 18446744073709551616",
        "Content-Length: 1",
        "Transfer-Encoding: chunked",
        "Expect: 100-continue",
    ] {
        let refused = http_head_with(addr, "GET", "/api/status", declaration)?;
        assert_eq!(refused.status, 413, "{declaration} should be refused");
        assert_eq!(refused.json()?["error"]["kind"], "payload-too-large");
    }

    // A zero length announces no body, which a client library that always sets
    // the field sends.
    let served = http_head_with(addr, "GET", "/api/status", "Content-Length: 0")?;
    assert_eq!(served.status, 200);

    // A refusal ends one connection and touches neither the worker pool nor the
    // daemon session behind it.
    assert_eq!(http_get(addr, "/api/node?id=alpha-id")?.status, 200);

    server.shutdown()?;
    Ok(())
}

#[test]
fn reading_server_keeps_serving_while_every_worker_is_held_by_a_silent_client() -> Result<()> {
    let (_workspace, root, db) = build_reading_fixture()?;
    let server = start_reading_server(&root, &db)?;
    let addr = server.local_addr();

    // Eight sockets against the fixture server's four workers, each a worker
    // waiting for a head that never arrives.
    let parked: Vec<TcpStream> = (0..8)
        .map(|_| TcpStream::connect(addr))
        .collect::<std::io::Result<Vec<_>>>()?;

    // The 5s ceiling is a small multiple of the grace a silent connection gets,
    // which is what frees the workers holding these. Without that bound this
    // call never returns.
    let started = Instant::now();
    let answered = http_get(addr, "/api/node?id=alpha-id")?;
    let waited = started.elapsed();
    assert_eq!(answered.status, 200);
    assert_eq!(answered.json()?["title"], "Alpha");
    assert!(
        waited < Duration::from_secs(5),
        "a reader waited {waited:?} behind silent clients"
    );

    // The sockets are still parked here, so shutdown must not wait on them.
    let started = Instant::now();
    server.shutdown()?;
    let stopped = started.elapsed();
    assert!(
        stopped < Duration::from_secs(5),
        "shutdown waited {stopped:?} on silent clients"
    );
    drop(parked);
    Ok(())
}

#[test]
fn reading_server_cannot_reach_a_mutating_operation_over_http() -> Result<()> {
    let (_workspace, root, db) = build_reading_fixture()?;
    let server = start_reading_server(&root, &db)?;
    let addr = server.local_addr();

    // The router is a strict allowlist of read routes, so a mutating path
    // dead-ends at the not-found handler rather than reaching the daemon.
    for probe in [
        "/api/capture",
        "/api/grade",
        "/api/index",
        "/api/glossary/grade?key=file:riemann.org&grade=5",
    ] {
        assert_eq!(http_get(addr, probe)?.status, 404, "no route for {probe}");
    }
    assert_eq!(http_request(addr, "DELETE", "/api/status")?.status, 405);

    // 4 is the fixture's file count, so the corpus is untouched.
    let status = http_get(addr, "/api/status")?;
    assert_eq!(status.json()?["files_indexed"], 4);

    server.shutdown()?;
    Ok(())
}

#[test]
fn reading_server_answers_concurrent_http_clients() -> Result<()> {
    let (_workspace, root, db) = build_reading_fixture()?;
    let server = start_reading_server(&root, &db)?;
    let addr = server.local_addr();

    // Each client asks for a different note, so an answer routed to the wrong
    // connection fails an assertion. Identical requests would satisfy this test
    // even if every response went to the wrong socket.
    let asked = [
        ("alpha-id", "Alpha"),
        ("beta-id", "Beta"),
        ("weak-id", "Weak"),
        ("riemann-id", "Riemann integral"),
    ];
    let mut handles = Vec::new();
    for client in 0..16 {
        let (id, title) = asked[client % asked.len()];
        handles.push(thread::spawn(move || -> Result<()> {
            for _ in 0..8 {
                let response = http_get(addr, &format!("/api/node?id={id}"))?;
                if response.status != 200 {
                    bail!("expected 200 for {id}, got {}", response.status);
                }
                let answered = response.json()?["title"].clone();
                if answered != title {
                    bail!("client asked for {id} and was answered with {answered}");
                }
            }
            Ok(())
        }));
    }
    for handle in handles {
        handle.join().expect("client thread should not panic")?;
    }

    server.shutdown()?;
    Ok(())
}

/// Percent-encode a slipbox key. Keys embed `:` and `/`, so they cannot be
/// placed raw in a query string.
fn encode(value: &str) -> String {
    urlencoding::encode(value).into_owned()
}
