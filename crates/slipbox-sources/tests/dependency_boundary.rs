//! The source crate's dependency boundary, as Cargo resolves it.
//!
//! Workspace feature unification may enlarge the host-filtered graph; absence
//! assertions apply to that superset.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

/// Desktop adapters, unreachable from source configuration in every build.
const FORBIDDEN: &[(&str, &str)] = &[
    ("slipbox", "root package adapters"),
    ("slipbox-web", "reading server and client bundle"),
    ("slipbox-daemon-client", "daemon process coordination"),
    ("clap", "command-line parsing"),
];

/// Derived-database crates. Source configuration is persisted beside them, not
/// by them, so they may enter only through this crate's own test build.
const TEST_ONLY: &[&str] = &[
    "slipbox-engine",
    "slipbox-index",
    "slipbox-rpc",
    "slipbox-store",
    "slipbox-write",
];

/// Whether the root package's dev-dependencies participate in the walk. No
/// other package's dev-dependencies ever do.
#[derive(Clone, Copy, PartialEq, Eq)]
enum DevEdges {
    Excluded,
    FromRoot,
}

#[test]
fn the_resolved_graph_is_rooted_at_this_crate() {
    let graph = ResolvedGraph::rooted_at(&sources_manifest());

    assert_eq!(graph.name(graph.root()), "slipbox-sources");
    let resolved_manifest = Path::new(
        graph.package(graph.root())["manifest_path"]
            .as_str()
            .expect("a package reports its manifest path"),
    );
    assert_eq!(
        fs::canonicalize(resolved_manifest).expect("the resolved manifest exists"),
        fs::canonicalize(sources_manifest()).expect("this manifest exists"),
        "the graph is rooted at another copy of this crate"
    );
}

#[test]
fn the_domain_types_come_from_core_and_nothing_else_in_the_workspace() {
    let graph = ResolvedGraph::rooted_at(&sources_manifest());

    assert_eq!(
        graph.path_names(graph.reachable(DevEdges::Excluded)),
        owned(&["slipbox-core", "slipbox-sources"]),
        "the library build reaches a workspace crate beyond core"
    );
    assert_eq!(
        graph.declared_dependencies("normal"),
        owned(&["serde", "serde_json", "slipbox-core", "thiserror"]),
        "the library's declared dependencies changed"
    );
}

#[test]
fn no_desktop_adapter_is_reachable_from_source_configuration() {
    let graph = ResolvedGraph::rooted_at(&sources_manifest());

    for edges in [DevEdges::Excluded, DevEdges::FromRoot] {
        let reachable = graph.names(graph.reachable(edges));
        for (package, role) in FORBIDDEN {
            assert!(
                !reachable.contains(*package),
                "{package} ({role}) is reachable from source configuration"
            );
        }
    }

    // The desktop host is a positive control for the same traversal.
    let root = ResolvedGraph::rooted_at(&root_manifest());
    let reachable = root.names(root.reachable(DevEdges::Excluded));
    for (package, role) in FORBIDDEN {
        assert!(
            reachable.contains(*package),
            "{package} ({role}) is unreachable even from the root package"
        );
    }
}

#[test]
fn the_canonical_engine_enters_only_through_the_test_build() {
    let graph = ResolvedGraph::rooted_at(&sources_manifest());

    let library = graph.names(graph.reachable(DevEdges::Excluded));
    let tests = graph.names(graph.reachable(DevEdges::FromRoot));
    for package in TEST_ONLY {
        assert!(
            !library.contains(*package),
            "{package} is reachable from the source library itself"
        );
    }
    for package in TEST_ONLY {
        assert!(
            tests.contains(*package),
            "{package} is unreachable even from the test build, so the absence above proves nothing"
        );
    }

    assert_eq!(
        graph.declared_dependencies("dev"),
        owned(&["slipbox-engine", "slipbox-rpc", "tempfile"]),
        "the test build pulls more than the canonical engine and a temporary-directory helper"
    );
}

#[test]
fn nothing_this_crate_depends_on_depends_on_it() {
    let graph = ResolvedGraph::rooted_at(&sources_manifest());

    for id in graph.reachable(DevEdges::FromRoot) {
        if id == graph.root() {
            continue;
        }
        assert!(
            !graph.edges(&id, DevEdges::FromRoot).contains(graph.root()),
            "{} depends on the source crate, which would close a cycle",
            graph.name(&id)
        );
    }
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

    fn declared_dependencies(&self, kind: &str) -> BTreeSet<String> {
        self.package(self.root())["dependencies"]
            .as_array()
            .expect("a package lists its dependency declarations")
            .iter()
            .filter(|dependency| match dependency["kind"].as_str() {
                None => kind == "normal",
                Some(declared) => declared == kind,
            })
            .map(|dependency| {
                dependency["name"]
                    .as_str()
                    .expect("a dependency is named")
                    .to_owned()
            })
            .collect()
    }

    fn edges(&self, id: &str, dev: DevEdges) -> BTreeSet<String> {
        let dev_follows = dev == DevEdges::FromRoot && id == self.root;
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
                        Some("dev") => dev_follows,
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

    fn reachable(&self, dev: DevEdges) -> BTreeSet<String> {
        let mut seen = BTreeSet::new();
        let mut queue = VecDeque::from([self.root.clone()]);
        while let Some(id) = queue.pop_front() {
            if !seen.insert(id.clone()) {
                continue;
            }
            queue.extend(self.edges(&id, dev));
        }
        seen
    }

    fn names(&self, ids: BTreeSet<String>) -> BTreeSet<String> {
        ids.iter().map(|id| self.name(id).to_owned()).collect()
    }

    fn path_names(&self, ids: BTreeSet<String>) -> BTreeSet<String> {
        ids.iter()
            .filter(|id| self.package(id)["source"].is_null())
            .map(|id| self.name(id).to_owned())
            .collect()
    }
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

fn sources_manifest() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")
}

fn root_manifest() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("this crate sits two directories below the workspace root")
        .join("Cargo.toml")
}
