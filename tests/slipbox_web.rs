use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
#[cfg(unix)]
use std::path::Path;
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use serde_json::Value;
use slipbox_core::{
    BacklinksParams, ExplorationEntry, ExplorationLens, ExploreParams, ForwardLinksParams,
    GlossaryDueParams, GlossaryTermParams, ListGlossaryTermsParams, NodeFromIdParams,
    NodeFromKeyParams, NodeFromTitleOrAliasParams, NoteContextParams, ReflinksParams,
    SearchGlossaryParams, SearchNodesParams, UnlinkedReferencesParams,
};
use slipbox_daemon_client::DaemonServeConfig;
use slipbox_index::scan_root;
use slipbox_store::Database;
use slipbox_web::{Assets, ReadingBridge, ReadingServer};
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
    // Alpha is filed first of the four notes, so only its later neighbor exists.
    assert_eq!((context.place.ordinal, context.place.total), (1, 4));
    assert!(context.place.earlier.is_none());
    assert_eq!(
        context.place.later.map(|neighbor| neighbor.title),
        Some("Beta".to_owned())
    );

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

    // The lens is a parameter of one method, so one call clears the whole set
    // through the read-only dispatch guard.
    let explored = bridge.explore(&ExploreParams {
        node_key: alpha.node_key.clone(),
        lens: ExplorationLens::Unresolved,
        limit: 10,
        unique: false,
    })?;
    assert_eq!(explored.lens, ExplorationLens::Unresolved);
    assert_eq!(explored.sections.len(), 2);
    assert!(matches!(
        explored.sections[1].entries.first(),
        Some(ExplorationEntry::Anchor { record }) if record.anchor.title == "Weak"
    ));

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

struct HttpResponse {
    status: u16,
    headers: Vec<(String, String)>,
    body: String,
}

impl HttpResponse {
    fn json(&self) -> Result<Value> {
        serde_json::from_str(&self.body)
            .with_context(|| format!("response body should be JSON: {}", self.body))
    }

    /// HTTP header names are case-insensitive, so the match is too.
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
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
    http_send(addr, method, target, LOOPBACK_HOST, None)
}

fn http_head_with(
    addr: SocketAddr,
    method: &str,
    target: &str,
    header: &str,
) -> Result<HttpResponse> {
    http_send(addr, method, target, LOOPBACK_HOST, Some(header))
}

/// A `GET` sent under an authority of the caller's choosing, for the `Host` check.
fn http_get_as_host(addr: SocketAddr, target: &str, host: &str) -> Result<HttpResponse> {
    http_send(addr, "GET", target, host, None)
}

/// The authority a reader's browser sends, and what every other request uses.
const LOOPBACK_HOST: &str = "localhost";

fn http_send(
    addr: SocketAddr,
    method: &str,
    target: &str,
    host: &str,
    header: Option<&str>,
) -> Result<HttpResponse> {
    let mut stream = TcpStream::connect(addr)?;
    write!(stream, "{method} {target} HTTP/1.1\r\nHost: {host}\r\n")?;
    if let Some(header) = header {
        write!(stream, "{header}\r\n")?;
    }
    write!(stream, "Connection: close\r\n\r\n")?;
    stream.flush()?;
    let mut raw = Vec::new();
    stream.read_to_end(&mut raw)?;
    parse_response(&raw)
}

fn parse_response(raw: &[u8]) -> Result<HttpResponse> {
    let text = String::from_utf8(raw.to_vec()).context("response should be UTF-8")?;
    let (head, body) = text
        .split_once("\r\n\r\n")
        .context("response should have a header/body separator")?;
    let mut lines = head.lines();
    let status_line = lines.next().context("response should have a status line")?;
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse::<u16>().ok())
        .ok_or_else(|| anyhow!("status line should carry a numeric code: {status_line}"))?;
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_owned(), value.trim().to_owned()))
        .collect();
    Ok(HttpResponse {
        status,
        headers,
        body: body.to_owned(),
    })
}

/// Port 0, so the OS assigns an ephemeral loopback port per test.
fn start_reading_server(root: &PathBuf, db: &PathBuf) -> Result<ReadingServer> {
    start_reading_server_with_assets(root, db, Assets::embedded())
}

/// Caller-supplied assets, so static serving is exercised without a client build.
fn start_reading_server_with_assets(
    root: &PathBuf,
    db: &PathBuf,
    assets: Assets,
) -> Result<ReadingServer> {
    let bridge = ReadingBridge::spawn(daemon_binary(), DaemonServeConfig::new(root, db))?;
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
    ReadingServer::start(addr, bridge, assets, 4).map_err(Into::into)
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
    // The same request answers the position, and an absent neighbor is missing
    // from the payload rather than null.
    let place = &context_body["place"];
    assert_eq!(place["ordinal"], 1);
    assert_eq!(place["total"], 4);
    assert_eq!(place["later"]["title"], "Beta");
    assert!(
        place.get("earlier").is_none(),
        "the first filed note carries no earlier neighbor: {place}"
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
fn reading_server_reads_a_note_through_every_exploration_lens() -> Result<()> {
    let (_workspace, root, db) = build_reading_fixture()?;
    let server = start_reading_server(&root, &db)?;
    let addr = server.local_addr();

    // Each lens defines its own sections, in order.
    for (lens, expected) in [
        ("structure", &["backlinks", "forward-links"][..]),
        ("refs", &["reflinks", "unlinked-references"][..]),
        ("time", &["time-neighbors"][..]),
        ("tasks", &["task-neighbors"][..]),
        ("bridges", &["bridge-candidates"][..]),
        ("dormant", &["dormant-notes"][..]),
        (
            "unresolved",
            &["unresolved-tasks", "weakly-integrated-notes"][..],
        ),
    ] {
        let read = http_get(
            addr,
            &format!("/api/explore?key=file:alpha.org&lens={lens}"),
        )?;
        assert_eq!(read.status, 200, "{lens}: {}", read.body);
        let body = read.json()?;
        assert_eq!(body["lens"], lens);
        let kinds = body["sections"]
            .as_array()
            .context("an exploration answers with sections")?
            .iter()
            .map(|section| section["kind"].as_str().unwrap_or_default().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(kinds, expected, "{lens}");
    }

    // Nothing links to Alpha, so backlinks is an empty list, not a missing one.
    let structure = http_get(addr, "/api/explore?key=file:alpha.org&lens=structure")?.json()?;
    assert_eq!(structure["sections"][0]["entries"], Value::Array(vec![]));
    assert_eq!(
        structure["sections"][1]["entries"][0]["destination_note"]["title"],
        "Beta"
    );

    // Weak shares a reference with Alpha and names it unlinked in prose, so it
    // reaches both refs sections.
    let refs = http_get(addr, "/api/explore?key=file:alpha.org&lens=refs")?.json()?;
    for section in [0, 1] {
        assert!(
            refs["sections"][section]["entries"]
                .as_array()
                .context("a refs section carries entries")?
                .iter()
                .any(|entry| entry["source_anchor"]["title"] == "Weak"),
            "{refs}"
        );
    }

    let unresolved = http_get(addr, "/api/explore?key=file:alpha.org&lens=unresolved")?.json()?;
    assert_eq!(unresolved["sections"][0]["entries"], Value::Array(vec![]));
    let weak = unresolved["sections"][1]["entries"][0].clone();
    // Serde flattens an entry's record beside the tag, not under a field.
    assert_eq!(weak["kind"], "anchor");
    assert_eq!(weak["anchor"]["title"], "Weak");
    assert_eq!(
        weak["explanation"]["kind"],
        "weakly-integrated-shared-reference"
    );

    // The operation clamps to `1..=1_000`; the route admits that range and
    // refuses either side of it rather than pulling a value in.
    assert_eq!(
        http_get(addr, "/api/explore?key=file:alpha.org&lens=refs&limit=1000")?.status,
        200
    );
    for target in [
        "/api/explore?key=file:alpha.org&lens=refs&limit=0",
        "/api/explore?key=file:alpha.org&lens=refs&limit=1001",
    ] {
        let refused = http_get(addr, target)?;
        assert_eq!(refused.status, 400, "{target}");
        assert!(
            refused.json()?["error"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("`limit`")),
            "{}",
            refused.body
        );
    }

    for (target, faulty) in [
        ("/api/explore?lens=structure", "`key`"),
        ("/api/explore?key=file:alpha.org", "`lens`"),
        ("/api/explore?key=file:alpha.org&lens=sideways", "`lens`"),
        ("/api/explore?key=file:alpha.org&lens=Structure", "`lens`"),
    ] {
        let refused = http_get(addr, target)?;
        assert_eq!(refused.status, 400, "{target}");
        assert!(
            refused.json()?["error"]["message"]
                .as_str()
                .is_some_and(|message| message.contains(faulty)),
            "{target}: {}",
            refused.body
        );
    }

    // The refusal lists what it would have taken.
    let unknown = http_get(addr, "/api/explore?key=file:alpha.org&lens=sideways")?;
    let message = unknown.json()?["error"]["message"]
        .as_str()
        .context("a refusal carries a message")?
        .to_owned();
    for spelling in [
        "structure",
        "refs",
        "time",
        "tasks",
        "bridges",
        "dormant",
        "unresolved",
    ] {
        assert!(message.contains(spelling), "{spelling} missing: {message}");
    }

    // A key that resolves to no note is a 404, as on every other relation route.
    let missing = http_get(addr, "/api/explore?key=file:missing.org&lens=structure")?;
    assert_eq!(missing.status, 404, "{}", missing.body);
    assert_eq!(missing.json()?["error"]["kind"], "not-found");
    // The anchor-aware lenses resolve the key separately, so check one of those too.
    let missing_anchor = http_get(addr, "/api/explore?key=file:missing.org&lens=refs")?;
    assert_eq!(missing_anchor.status, 404, "{}", missing_anchor.body);

    server.shutdown()?;
    Ok(())
}

// Kills the serving daemon through a signal, so it holds only on unix.
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
    // `/api` is the API's own path, so a probe there gets the JSON envelope
    // rather than falling through to the client's app shell.
    let api_root = http_get(addr, "/api")?;
    assert_eq!(api_root.status, 404);
    assert_eq!(api_root.json()?["error"]["kind"], "not-found");
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
    // A 405 must name the verbs that do work, per RFC 9110's `Allow`.
    let refused = http_request(addr, "POST", "/api/status")?;
    assert_eq!(refused.status, 405);
    assert_eq!(refused.header("Allow"), Some("GET, HEAD"));

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
    // relation limit 1..=1000, max_lines 1..=1000, before 0..=200. Every one is a
    // 400, never a clamp.
    for target in [
        "/api/search/nodes?q=alpha&limit=0",
        "/api/search/nodes?q=alpha&limit=201",
        "/api/backlinks?key=file:alpha.org&limit=1001",
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
fn reading_server_answers_only_to_a_loopback_host() -> Result<()> {
    let (_workspace, root, db) = build_reading_fixture()?;
    let server = start_reading_server(&root, &db)?;
    let addr = server.local_addr();
    let port = addr.port();

    // A DNS rebinding attack turns a name the attacker controls into a loopback
    // address, so the browser treats their page as this server's origin and sends
    // their name as the `Host`. Binding loopback does not stop it; refusing the
    // foreign authority does, before any note content is read.
    for host in [
        "notes.attacker.example".to_owned(),
        format!("notes.attacker.example:{port}"),
        "localhost.attacker.example".to_owned(),
        "10.0.0.7".to_owned(),
        "[2001:db8::1]".to_owned(),
    ] {
        let refused = http_get_as_host(addr, "/api/node?id=alpha-id", &host)?;
        assert_eq!(refused.status, 403, "{host} should be refused");
        assert_eq!(refused.json()?["error"]["kind"], "forbidden");
        // The refusal precedes the route, so no note is disclosed in the body.
        assert!(
            !refused.body.contains("Alpha"),
            "{host} should learn nothing about the note"
        );
    }

    // A non-API path is refused on the same terms: the app shell is what a
    // rebound page would need to bootstrap a reader against this port.
    assert_eq!(
        http_get_as_host(addr, "/", "notes.attacker.example")?.status,
        403
    );

    // Every authority a reader's own browser sends is served, including the
    // literal forms and the port the server is actually on.
    for host in [
        "localhost".to_owned(),
        format!("localhost:{port}"),
        format!("127.0.0.1:{port}"),
        format!("[::1]:{port}"),
    ] {
        let served = http_get_as_host(addr, "/api/node?id=alpha-id", &host)?;
        assert_eq!(served.status, 200, "{host} names this server");
    }

    // A refusal ends one connection and leaves the pool and daemon serving.
    assert_eq!(http_get(addr, "/api/status")?.status, 200);

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
fn reading_server_answers_a_head_probe_with_headers_and_no_body() -> Result<()> {
    let (_workspace, root, db) = build_reading_fixture()?;
    let assets = Assets::in_memory([("/index.html", b"<!doctype html>shell".to_vec())]);
    let server = start_reading_server_with_assets(&root, &db, assets)?;
    let addr = server.local_addr();

    // RFC 9110: HEAD routes exactly as GET and carries the identical header
    // fields, only with the body suppressed.
    let get = http_get(addr, "/api/status")?;
    let head = http_request(addr, "HEAD", "/api/status")?;
    assert_eq!(head.status, 200);
    assert!(head.body.is_empty(), "a HEAD reply carries no body");
    assert_eq!(head.header("Content-Type"), get.header("Content-Type"));
    assert_eq!(head.header("Content-Length"), get.header("Content-Length"));

    let shell = http_request(addr, "HEAD", "/")?;
    assert_eq!(shell.status, 200);
    assert!(shell.body.is_empty());

    assert_eq!(http_request(addr, "HEAD", "/api/nope")?.status, 404);

    server.shutdown()?;
    Ok(())
}

#[test]
fn reading_server_marks_api_bodies_as_never_stored() -> Result<()> {
    let (_workspace, root, db) = build_reading_fixture()?;
    let server = start_reading_server(&root, &db)?;
    let addr = server.local_addr();

    // `no-store`, not `no-cache`: an API body is live index state, so no store
    // may keep a copy to revalidate. Error bodies are covered too.
    for target in ["/api/status", "/api/node?id=alpha-id", "/api/nope"] {
        assert_eq!(
            http_get(addr, target)?.header("Cache-Control"),
            Some("no-store"),
            "{target}"
        );
    }

    server.shutdown()?;
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

#[test]
fn reading_server_serves_the_embedded_client_beneath_the_api() -> Result<()> {
    let (_workspace, root, db) = build_reading_fixture()?;
    // A built client stands in as an app shell and one hashed asset.
    let assets = Assets::in_memory([
        (
            "/index.html",
            b"<!doctype html><div id=root></div>".to_vec(),
        ),
        ("/assets/index-abc123.js", b"export default 1;".to_vec()),
    ]);
    let server = start_reading_server_with_assets(&root, &db, assets)?;
    let addr = server.local_addr();

    // `no-cache` on the shell: cacheable, but revalidated before every reuse,
    // since a new build changes the asset URLs it names.
    let root_page = http_get(addr, "/")?;
    assert_eq!(root_page.status, 200);
    assert!(root_page.body.contains("<div id=root>"));
    assert_eq!(
        root_page.header("Content-Type"),
        Some("text/html; charset=utf-8")
    );
    assert_eq!(root_page.header("Cache-Control"), Some("no-cache"));

    // A hashed asset's bytes never change for its URL, so it is `immutable` with
    // a one-year max-age.
    let asset = http_get(addr, "/assets/index-abc123.js")?;
    assert_eq!(asset.status, 200);
    assert_eq!(asset.body, "export default 1;");
    assert_eq!(
        asset.header("Content-Type"),
        Some("text/javascript; charset=utf-8")
    );
    assert_eq!(
        asset.header("Cache-Control"),
        Some("public, max-age=31536000, immutable")
    );

    // A miss whose last segment carries no extension is a client route, so it
    // falls back to the shell.
    let deep_link = http_get(addr, "/some/note?note=alpha-id")?;
    assert_eq!(deep_link.status, 200);
    assert!(deep_link.body.contains("<div id=root>"));

    // A miss that names a file is not: a browser asking for a script must get a
    // 404 rather than HTML it would try to execute.
    let missing = http_get(addr, "/assets/missing.js")?;
    assert_eq!(missing.status, 404);
    assert!(!missing.body.contains("<div id=root>"));

    let status = http_get(addr, "/api/status")?;
    assert_eq!(status.status, 200);
    assert_eq!(status.json()?["nodes_indexed"], 4);
    // Nor does `/api/` fall back: an unknown route there is a JSON not-found.
    let unknown_api = http_get(addr, "/api/nope")?;
    assert_eq!(unknown_api.status, 404);
    assert_eq!(unknown_api.json()?["error"]["kind"], "not-found");

    server.shutdown()?;
    Ok(())
}

#[test]
fn reading_server_without_a_client_serves_the_api_alone() -> Result<()> {
    let (_workspace, root, db) = build_reading_fixture()?;
    // An empty asset set is what a binary built without the asset feature has.
    let server = start_reading_server_with_assets(&root, &db, Assets::in_memory(EMPTY_ASSETS))?;
    let addr = server.local_addr();

    let root_page = http_get(addr, "/")?;
    assert_eq!(root_page.status, 404);

    let status = http_get(addr, "/api/status")?;
    assert_eq!(status.status, 200);
    assert_eq!(status.json()?["nodes_indexed"], 4);

    server.shutdown()?;
    Ok(())
}

#[test]
fn web_command_serves_the_surface_it_announces() -> Result<()> {
    let (_workspace, root, db) = build_reading_fixture()?;

    // Port 0 lets the OS assign one, so the address under test can only come
    // from the banner the command prints.
    let mut child = Command::new(daemon_binary())
        .args([
            "web",
            "--root",
            root.to_str().context("utf-8 root")?,
            "--db",
            db.to_str().context("utf-8 db")?,
            "--port",
            "0",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut banner = BufReader::new(child.stdout.take().context("stdout should be piped")?);

    let mut announced = String::new();
    banner.read_line(&mut announced)?;
    let addr: SocketAddr = announced
        .trim()
        .rsplit_once("http://")
        .map(|(_, addr)| addr.to_owned())
        .context("the banner announces the bound address")?
        .parse()?;

    // The second banner line is printed only after the daemon has answered.
    let mut served = String::new();
    banner.read_line(&mut served)?;
    assert!(served.contains("4 notes"), "{served}");

    let status = http_get(addr, "/api/status")?;
    assert_eq!(status.status, 200);
    assert_eq!(status.json()?["nodes_indexed"], 4);

    child.kill()?;
    child.wait()?;
    Ok(())
}

#[test]
fn web_command_reports_a_slipbox_it_cannot_read_instead_of_announcing_a_surface() -> Result<()> {
    let workspace = tempdir()?;
    let db = workspace.path().join("slipbox.sqlite");

    // Spawning the daemon only starts a child, so the command's own startup
    // round trip is the only thing that can catch an unreadable root.
    let mut child = Command::new(daemon_binary())
        .args([
            "web",
            "--root",
            workspace
                .path()
                .join("absent")
                .to_str()
                .context("utf-8 root")?,
            "--db",
            db.to_str().context("utf-8 db")?,
            "--port",
            "0",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let status = match wait_briefly(&mut child)? {
        Some(status) => status,
        None => {
            child.kill()?;
            child.wait()?;
            bail!("the command kept serving a slipbox the daemon cannot read");
        }
    };
    let output = child.wait_with_output()?;

    assert!(!status.success(), "{status:?}");
    assert!(
        output.stdout.is_empty(),
        "no surface is announced: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("could not read the slipbox"), "{stderr}");

    Ok(())
}

/// Wait up to 5s for a command to exit, so one that keeps running fails the test
/// rather than hanging the suite.
fn wait_briefly(child: &mut Child) -> Result<Option<ExitStatus>> {
    for _ in 0..250 {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        thread::sleep(Duration::from_millis(20));
    }
    Ok(None)
}

/// An empty asset set, spelled with a concrete item type so the compiler can
/// infer one for the empty `in_memory` iterator.
const EMPTY_ASSETS: [(&str, &[u8]); 0] = [];

/// Percent-encode a slipbox key. Keys embed `:` and `/`, so they cannot be
/// placed raw in a query string.
fn encode(value: &str) -> String {
    urlencoding::encode(value).into_owned()
}
