//! What the Android package links, exports and loads, as Cargo resolves it.
//!
//! Feature selection is asserted for the Android target, not the host, because
//! the packaged library is what has no system SQLite to fall back to.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

const ANDROID_TARGET: &str = "aarch64-linux-android";

const FORBIDDEN: &[(&str, &str)] = &[
    ("clap", "command-line parsing"),
    ("slipbox-daemon-client", "daemon process coordination"),
    ("slipbox-web", "reading server and client bundle"),
    ("slipbox", "root package adapters"),
];

const EXPECTED_WORKSPACE_CLOSURE: &[&str] = &[
    "slipbox-android",
    "slipbox-core",
    "slipbox-engine",
    "slipbox-index",
    "slipbox-rpc",
    "slipbox-store",
    "slipbox-write",
];

const SEAM_SOURCE: &str =
    "app/src/main/kotlin/io/github/b_vitamins/slipbox/engine/SlipboxNativeEngine.kt";

const VERIFIER: &str = "tools/verify-native-packaging.sh";

/// The closed set of native methods the library exports, as Kotlin names them.
///
/// It is spelled out here so that four declarations of it -- the Kotlin seam,
/// the linker version script, the Rust definitions and the packaging verifier --
/// cannot agree on a fifth set between them.
const JNI_METHODS: &[&str] = &[
    "nativeAdapterContract",
    "nativeCloseSession",
    "nativeMaintainSession",
    "nativeOpenMaintenanceSession",
    "nativeOpenReadSession",
    "nativeReadSession",
    "nativeRunFixtureProbe",
];

#[test]
fn the_android_package_consumes_the_engine_and_no_desktop_adapter() {
    let graph = ResolvedGraph::rooted_at(&crate_manifest());
    assert_eq!(graph.name(graph.root()), "slipbox-android");

    assert_eq!(
        graph.reachable_path_names(),
        owned(EXPECTED_WORKSPACE_CLOSURE)
    );
    for (package, role) in FORBIDDEN {
        assert!(
            !graph.reachable_names().contains(*package),
            "{package} ({role}) is reachable from the Android package"
        );
    }

    // The desktop host is a positive control for the same traversal.
    let root = ResolvedGraph::rooted_at(&workspace_manifest()).reachable_names();
    for (package, role) in FORBIDDEN {
        assert!(
            root.contains(*package),
            "{package} ({role}) is unreachable even from the root package"
        );
    }
}

#[test]
fn bundled_sqlite_is_selected_through_the_whole_android_path() {
    let graph = ResolvedGraph::rooted_at(&crate_manifest());

    for package in ["slipbox-engine", "slipbox-store"] {
        let selected = graph.selected_features(package);
        assert!(
            selected.contains("bundled-sqlite"),
            "{package} does not select bundled SQLite: {selected:?}"
        );
        assert!(
            !selected.contains("system-sqlite"),
            "{package} selects a host SQLite the Android package cannot link: {selected:?}"
        );
    }
    for package in ["rusqlite", "libsqlite3-sys"] {
        let selected = graph.selected_features(package);
        assert!(
            selected.contains("bundled"),
            "{package} does not select the bundled amalgamation: {selected:?}"
        );
    }

    // The crate declares no SQLite choice of its own, so no embedding can
    // deselect the amalgamation.
    let package = graph.package(graph.root());
    assert_eq!(
        package["features"]
            .as_object()
            .expect("a package declares a feature table")
            .len(),
        0,
        "the Android package declares a selectable feature: {}",
        package["features"]
    );

    let engine = declared_dependency(package, "slipbox-engine");
    assert_eq!(engine["uses_default_features"], false);
    assert_eq!(requested_features(engine), owned(&["bundled-sqlite"]));
}

#[test]
fn the_android_package_builds_one_shared_library_and_no_process() {
    let graph = ResolvedGraph::rooted_at(&crate_manifest());
    let targets = graph.package(graph.root())["targets"]
        .as_array()
        .expect("a package declares targets");

    // Cargo names a library target after its package with dashes replaced, which
    // is the name the loader resolves as `libslipbox_android.so`.
    let libraries: Vec<&Value> = targets
        .iter()
        .filter(|target| target["name"] == "slipbox_android")
        .collect();
    assert_eq!(
        libraries.len(),
        1,
        "the package declares no single library target named slipbox_android"
    );
    assert_eq!(kinds(libraries[0]), owned(&["cdylib", "rlib"]));

    let executables: Vec<&Value> = targets
        .iter()
        .filter(|target| kinds(target).contains("bin"))
        .collect();
    assert!(
        executables.is_empty(),
        "the package builds an executable no application can start: {executables:?}"
    );
}

#[test]
fn the_exported_symbols_are_exactly_the_ones_kotlin_declares() {
    let seam = read(&android_directory().join(SEAM_SOURCE));
    let package = kotlin_package(&seam);

    let declared: BTreeSet<String> = declarations(&seam, "external fun ")
        .map(|declaration| method_name(&declaration))
        .collect();
    assert_eq!(declared, owned(JNI_METHODS), "{SEAM_SOURCE}");

    let expected: BTreeSet<String> = JNI_METHODS
        .iter()
        .map(|method| mangled(&package, method))
        .collect();
    assert_eq!(exported_symbols(), expected, "jni-exports.map");

    let source = read(&crate_directory().join("src/jni_seam.rs"));
    let defined: BTreeSet<String> = declarations(&source, "pub unsafe extern \"system\" fn ")
        .map(|declaration| method_name(&declaration))
        .collect();
    assert_eq!(defined, expected, "src/jni_seam.rs");

    let verifier = read(&android_directory().join(VERIFIER));
    assert_eq!(verifier_symbols(&verifier), expected, "{VERIFIER}");
}

#[test]
fn the_loaded_library_name_follows_the_crate_name() {
    let seam = read(&android_directory().join(SEAM_SOURCE));

    let declared = declaration(&seam, "const val LIBRARY_NAME: String = ");
    assert_eq!(
        declared,
        format!("\"{}\"", "slipbox-android".replace('-', "_"))
    );
    assert!(
        seam.contains("System.loadLibrary(LIBRARY_NAME)"),
        "the seam loads a library it does not name"
    );
}

#[test]
fn the_cross_build_passes_link_arguments_cargo_cannot_split() {
    let build = read(&android_directory().join("app/build.gradle.kts"));

    // Cargo splits RUSTFLAGS and every CARGO_TARGET_<triple>_RUSTFLAGS value on
    // whitespace, so a checkout whose path holds any would drop the version
    // script and export every symbol the library defines.
    for (position, _) in build.match_indices("RUSTFLAGS") {
        assert!(
            build[..position].ends_with("CARGO_ENCODED_"),
            "the cross build passes rustflags through a whitespace-split variable"
        );
    }
    assert!(
        build.contains("\"CARGO_ENCODED_RUSTFLAGS\""),
        "the cross build passes no encoded rustflags"
    );
    assert!(
        build.contains(r#".joinToString("\u001F")"#),
        "the encoded rustflags are not separated by the unit separator"
    );
    assert!(
        build.contains(r#".flatMap { listOf("-C", "link-arg=$it") }"#),
        "an encoded element holds more than one argument"
    );
    assert!(
        build.contains(r#""-Wl,--version-script=${script.absolutePath}""#),
        "the version script path is not one encoded element"
    );
}

#[test]
fn no_native_library_is_committed_to_a_source_tree() {
    let mut committed = Vec::new();
    for tree in [crate_directory(), android_directory().join("app/src")] {
        collect_binaries(&tree, &mut committed);
    }

    assert_eq!(
        committed,
        Vec::<PathBuf>::new(),
        "a built artifact is in a source tree instead of a generated output directory"
    );
}

fn exported_symbols() -> BTreeSet<String> {
    read(&crate_directory().join("jni-exports.map"))
        .lines()
        .skip_while(|line| !line.trim_start().starts_with("global:"))
        .skip(1)
        .take_while(|line| !line.trim_start().starts_with("local:"))
        .filter_map(|line| line.trim().strip_suffix(';').map(str::to_owned))
        .collect()
}

/// The verifier's own inventory, read as the list it iterates over.
fn verifier_symbols(source: &str) -> BTreeSet<String> {
    source
        .lines()
        .skip_while(|line| !line.starts_with("JNI_SYMBOLS=\""))
        .skip(1)
        .take_while(|line| line.trim() != "\"")
        .map(|line| line.trim().to_owned())
        .filter(|line| !line.is_empty())
        .collect()
}

fn kotlin_package(source: &str) -> String {
    source
        .lines()
        .find_map(|line| line.strip_prefix("package "))
        .expect("a Kotlin source declares its package")
        .trim()
        .to_owned()
}

/// The JNI mangling of one method of the seam object: `_1` escapes an
/// underscore, and a dot becomes an underscore.
fn mangled(package: &str, method: &str) -> String {
    format!(
        "Java_{}",
        format!("{package}.SlipboxNativeEngine.{method}")
            .replace('_', "_1")
            .replace('.', "_")
    )
}

fn method_name(declaration: &str) -> String {
    declaration
        .split('(')
        .next()
        .expect("a declaration names what it declares")
        .to_owned()
}

fn declaration(source: &str, keyword: &str) -> String {
    let mut found = declarations(source, keyword);
    let first = found
        .next()
        .unwrap_or_else(|| panic!("the source declares no {keyword}"));
    assert_eq!(
        found.next(),
        None,
        "{keyword} is declared more than once, so no single declaration answers for it"
    );
    first
}

fn declarations<'a>(source: &'a str, keyword: &'a str) -> impl Iterator<Item = String> + 'a {
    source
        .lines()
        .filter_map(move |line| line.split_once(keyword))
        .map(|(_, rest)| rest.trim().to_owned())
}

fn collect_binaries(directory: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries {
        let path = entry.expect("a directory entry is readable").path();
        if path.is_dir() {
            collect_binaries(&path, found);
        } else if matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("so" | "a" | "apk")
        ) {
            found.push(path);
        }
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
            .args(["--filter-platform", ANDROID_TARGET])
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

        Self {
            root: metadata["resolve"]["root"]
                .as_str()
                .expect("a manifest query resolves a root package")
                .to_owned(),
            packages: by_id(&metadata["packages"]),
            nodes: by_id(&metadata["resolve"]["nodes"]),
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

    fn selected_features(&self, name: &str) -> BTreeSet<String> {
        let id = self
            .reachable()
            .into_iter()
            .find(|id| self.name(id) == name)
            .unwrap_or_else(|| panic!("{name} is in the resolved closure"));
        self.nodes[&id]["features"]
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
}

fn declared_dependency<'a>(package: &'a Value, name: &str) -> &'a Value {
    package["dependencies"]
        .as_array()
        .expect("a package lists its dependency declarations")
        .iter()
        .find(|dependency| dependency["name"] == *name)
        .unwrap_or_else(|| panic!("{name} is declared"))
}

/// Every test target also has crate type `bin`, so only `kind` separates a
/// library from an executable.
fn kinds(target: &Value) -> BTreeSet<String> {
    target["kind"]
        .as_array()
        .expect("a target states its kinds")
        .iter()
        .map(|kind| kind.as_str().expect("a target kind is a string").to_owned())
        .collect()
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

fn read(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

fn crate_directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn crate_manifest() -> PathBuf {
    crate_directory().join("Cargo.toml")
}

fn workspace_directory() -> PathBuf {
    crate_directory()
        .parent()
        .and_then(Path::parent)
        .expect("the crate sits two directories below the workspace root")
        .to_path_buf()
}

fn workspace_manifest() -> PathBuf {
    workspace_directory().join("Cargo.toml")
}

fn android_directory() -> PathBuf {
    workspace_directory().join("android")
}
