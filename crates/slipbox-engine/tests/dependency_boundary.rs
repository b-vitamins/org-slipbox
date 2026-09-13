//! The headless dependency boundary, as Cargo resolves it.
//!
//! Workspace feature unification may enlarge the host-filtered graph; absence
//! assertions apply to that superset.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const FORBIDDEN: &[(&str, &str)] = &[
    ("clap", "command-line parsing"),
    ("slipbox-daemon-client", "daemon process coordination"),
    ("slipbox-web", "reading server and client bundle"),
    ("slipbox", "root package adapters"),
];

const EXPECTED_WORKSPACE_CLOSURE: &[&str] = &[
    "slipbox-core",
    "slipbox-engine",
    "slipbox-index",
    "slipbox-rpc",
    "slipbox-store",
    "slipbox-write",
];

#[test]
fn the_resolved_graph_is_rooted_at_the_engine_and_carries_transitives() {
    let graph = ResolvedGraph::rooted_at(&engine_manifest());

    assert_eq!(graph.name(graph.root()), "slipbox-engine");
    let resolved_manifest = Path::new(
        graph.package(graph.root())["manifest_path"]
            .as_str()
            .expect("a package reports its manifest path"),
    );
    assert_eq!(
        fs::canonicalize(resolved_manifest).expect("the resolved manifest exists"),
        fs::canonicalize(engine_manifest()).expect("this manifest exists"),
        "the graph is rooted at another copy of the engine"
    );

    let reachable = graph.reachable_names();
    for expected in EXPECTED_WORKSPACE_CLOSURE {
        assert!(
            reachable.contains(*expected),
            "{expected} is missing from the resolved closure: {reachable:?}"
        );
    }

    // A direct-only walk must not satisfy the boundary check.
    let direct = graph.direct_registry_names();
    let registry = graph.reachable_registry_names();
    assert!(direct.is_subset(&registry));
    assert!(
        registry.len() > direct.len(),
        "the closure carries no transitive registry package: {registry:?}"
    );
}

#[test]
fn no_desktop_adapter_is_reachable_from_the_engine() {
    let engine = ResolvedGraph::rooted_at(&engine_manifest()).reachable_names();
    for (package, role) in FORBIDDEN {
        assert!(
            !engine.contains(*package),
            "{package} ({role}) is reachable from the headless engine"
        );
    }

    // The desktop host is a positive control for the same traversal.
    let root = ResolvedGraph::rooted_at(&root_manifest()).reachable_names();
    for (package, role) in FORBIDDEN {
        assert!(
            root.contains(*package),
            "{package} ({role}) is unreachable even from the root package"
        );
    }
}

#[test]
fn the_engine_reaches_exactly_its_own_workspace_crates() {
    let graph = ResolvedGraph::rooted_at(&engine_manifest());

    assert_eq!(
        graph.reachable_path_names(),
        owned(EXPECTED_WORKSPACE_CLOSURE)
    );

    for id in graph.reachable() {
        if id == graph.root() {
            continue;
        }
        assert!(
            !graph.followed_edges(&id).contains(graph.root()),
            "{} depends on the engine, which would close a cycle",
            graph.name(&id)
        );
    }
}

#[test]
fn the_engine_neither_declares_nor_requests_a_desktop_capability() {
    let graph = ResolvedGraph::rooted_at(&engine_manifest());
    let engine = graph.package(graph.root());

    let declared: BTreeSet<&str> = engine["features"]
        .as_object()
        .expect("a package declares a feature table")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        declared,
        BTreeSet::from(["bundled-sqlite", "default", "system-sqlite"]),
        "the engine owns a feature beyond its SQLite selection"
    );

    let index = declared_dependency(engine, "slipbox-index");
    assert_eq!(
        requested_features(index),
        BTreeSet::new(),
        "the engine requests an index feature instead of leaving the choice to the embedding"
    );

    let index_id = graph
        .id_of("slipbox-index")
        .expect("the index is in the closure");
    let index_features = graph.package(&index_id)["features"]
        .as_object()
        .expect("a package declares a feature table");
    assert!(index_features.contains_key("desktop-decryptors"));
    assert!(
        !index_features.contains_key("default"),
        "the index declares a default feature: {:?}",
        index_features.keys().collect::<Vec<_>>()
    );

    let store = declared_dependency(engine, "slipbox-store");
    assert_eq!(
        store["uses_default_features"], false,
        "the engine takes the store's default SQLite selection rather than forwarding its own"
    );

    let selected: BTreeSet<String> = graph.selected_features(graph.root()).into_iter().collect();
    assert_eq!(selected, owned(&["bundled-sqlite", "default"]));
}

#[test]
fn the_engine_declares_no_transport_target() {
    let graph = ResolvedGraph::rooted_at(&engine_manifest());
    let engine = graph.package(graph.root());

    let kinds: BTreeSet<&str> = engine["targets"]
        .as_array()
        .expect("a package declares targets")
        .iter()
        .flat_map(|target| target["kind"].as_array().expect("a target has a kind"))
        .map(|kind| kind.as_str().expect("a target kind is a string"))
        .collect();
    assert!(kinds.contains("lib"));
    assert!(
        !kinds.contains("bin"),
        "the engine declares a binary, which would make it a process rather than a library: {kinds:?}"
    );

    let dev: Vec<&str> = engine["dependencies"]
        .as_array()
        .expect("a package lists its dependency declarations")
        .iter()
        .filter(|dependency| dependency["kind"] == "dev")
        .map(|dependency| dependency["name"].as_str().expect("a dependency is named"))
        .collect();
    assert_eq!(
        dev,
        vec!["tempfile"],
        "the engine's test build pulls more than a temporary-directory helper"
    );
}

struct ResolvedGraph {
    root: String,
    packages: BTreeMap<String, Value>,
    nodes: BTreeMap<String, Value>,
}

impl ResolvedGraph {
    fn rooted_at(manifest: &Path) -> Self {
        let output = Command::new(env!("CARGO"))
            .args(["metadata", "--locked", "--format-version", "1"])
            .arg("--filter-platform")
            .arg(host_triple())
            .arg("--manifest-path")
            .arg(manifest)
            .output()
            .unwrap_or_else(|error| panic!("failed to run cargo metadata: {error}"));
        assert!(
            output.status.success(),
            "cargo metadata for {} failed: {}",
            manifest.display(),
            String::from_utf8_lossy(&output.stderr)
        );
        let metadata: Value =
            serde_json::from_slice(&output.stdout).expect("cargo metadata emits JSON");

        let root = metadata["resolve"]["root"]
            .as_str()
            .expect("a manifest query resolves a root package")
            .to_owned();
        let packages = by_id(&metadata["packages"]);
        let nodes = by_id(&metadata["resolve"]["nodes"]);
        assert!(nodes.contains_key(&root), "the root package has a node");

        Self {
            root,
            packages,
            nodes,
        }
    }

    fn root(&self) -> &str {
        &self.root
    }

    fn package(&self, id: &str) -> &Value {
        self.packages
            .get(id)
            .unwrap_or_else(|| panic!("{id} is a resolved package"))
    }

    fn name(&self, id: &str) -> &str {
        self.package(id)["name"]
            .as_str()
            .expect("a package is named")
    }

    fn id_of(&self, name: &str) -> Option<String> {
        self.reachable()
            .into_iter()
            .find(|id| self.name(id) == name)
    }

    fn selected_features(&self, id: &str) -> Vec<String> {
        self.nodes[id]["features"]
            .as_array()
            .expect("a node lists its selected features")
            .iter()
            .map(|feature| {
                feature
                    .as_str()
                    .expect("a selected feature is a string")
                    .to_owned()
            })
            .collect()
    }

    // Only the root package's dev-dependencies participate in its test build.
    fn followed_edges(&self, id: &str) -> BTreeSet<String> {
        let is_root = id == self.root;
        self.nodes[id]["deps"]
            .as_array()
            .expect("a node lists its dependency edges")
            .iter()
            .filter(|dep| {
                dep["dep_kinds"]
                    .as_array()
                    .expect("a resolved edge states its kinds")
                    .iter()
                    .any(|dep_kind| match dep_kind["kind"].as_str() {
                        None | Some("build") => true,
                        Some("dev") => is_root,
                        Some(_) => false,
                    })
            })
            .map(|dep| {
                dep["pkg"]
                    .as_str()
                    .expect("an edge names a package id")
                    .to_owned()
            })
            .collect()
    }

    fn reachable(&self) -> BTreeSet<String> {
        let mut seen = BTreeSet::new();
        let mut queue = VecDeque::from([self.root.clone()]);
        while let Some(id) = queue.pop_front() {
            if !seen.insert(id.clone()) {
                continue;
            }
            queue.extend(self.followed_edges(&id));
        }
        seen
    }

    fn reachable_names(&self) -> BTreeSet<String> {
        self.reachable()
            .iter()
            .map(|id| self.name(id).to_owned())
            .collect()
    }

    fn reachable_path_names(&self) -> BTreeSet<String> {
        self.reachable()
            .iter()
            .filter(|id| self.package(id)["source"].is_null())
            .map(|id| self.name(id).to_owned())
            .collect()
    }

    fn registry_names(&self, ids: impl IntoIterator<Item = String>) -> BTreeSet<String> {
        ids.into_iter()
            .filter(|id| !self.package(id)["source"].is_null())
            .map(|id| self.name(&id).to_owned())
            .collect()
    }

    fn direct_registry_names(&self) -> BTreeSet<String> {
        self.registry_names(self.followed_edges(&self.root))
    }

    fn reachable_registry_names(&self) -> BTreeSet<String> {
        self.registry_names(self.reachable())
    }
}

fn declared_dependency<'a>(package: &'a Value, name: &str) -> &'a Value {
    package["dependencies"]
        .as_array()
        .expect("a package lists its dependency declarations")
        .iter()
        .find(|dependency| dependency["name"] == *name)
        .unwrap_or_else(|| panic!("{name} is declared"))
}

fn requested_features(dependency: &Value) -> BTreeSet<String> {
    dependency["features"]
        .as_array()
        .expect("a declaration lists the features it requests")
        .iter()
        .map(|feature| {
            feature
                .as_str()
                .expect("a requested feature is a string")
                .to_owned()
        })
        .collect()
}

fn owned(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

fn by_id(entries: &Value) -> BTreeMap<String, Value> {
    entries
        .as_array()
        .expect("cargo metadata reports an array")
        .iter()
        .map(|entry| {
            (
                entry["id"]
                    .as_str()
                    .expect("an entry carries a package id")
                    .to_owned(),
                entry.clone(),
            )
        })
        .collect()
}

fn host_triple() -> String {
    let output = Command::new(env!("CARGO"))
        .arg("-vV")
        .output()
        .unwrap_or_else(|error| panic!("failed to run cargo -vV: {error}"));
    assert!(output.status.success(), "cargo -vV failed");
    let report = String::from_utf8(output.stdout).expect("cargo -vV writes UTF-8");
    report
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .expect("cargo -vV reports a host triple")
        .to_owned()
}

fn engine_manifest() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")
}

fn root_manifest() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the engine sits two directories below the workspace root")
        .join("Cargo.toml")
}
