//! Embed the built reading client into the binary under the `embed-assets`
//! feature.
//!
//! `include_bytes!` needs a literal path per file, so this script walks the
//! built `client/dist` tree and generates a manifest of `(url-path, bytes)`
//! pairs the crate includes at compile time. A default build carries no web
//! assets and needs no built client, so the walk runs only when the feature
//! selects it — keeping `cargo build` and `cargo test` green on a fresh
//! checkout where `client/dist` does not yet exist.

use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    // Cargo sets `CARGO_FEATURE_<NAME>` for each enabled feature. Without the
    // embed feature there is nothing to generate: the crate gates its
    // `include!` of the manifest on the same feature, so the file is never read.
    if env::var_os("CARGO_FEATURE_EMBED_ASSETS").is_none() {
        return;
    }

    let manifest_dir =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
    let dist = manifest_dir.join("client").join("dist");
    // Rebuild when the built tree changes; the per-file lines below add each
    // concrete file so an in-place edit is caught as well as an add or removal.
    println!("cargo:rerun-if-changed={}", dist.display());

    let mut assets = Vec::new();
    if dist.is_dir() {
        collect(&dist, &dist, &mut assets);
    }
    assets.sort();
    assert!(
        !assets.is_empty(),
        "the `embed-assets` feature is enabled but no built client was found at {}; \
         build it first with `npm --prefix crates/slipbox-web/client ci` and \
         `npm --prefix crates/slipbox-web/client run build`",
        dist.display()
    );

    let mut manifest = String::from("pub static ASSETS: &[(&str, &[u8])] = &[\n");
    for (url_path, file_path) in &assets {
        writeln!(
            manifest,
            "    ({url_path:?}, include_bytes!({file_path:?})),"
        )
        .expect("writing to a String cannot fail");
    }
    manifest.push_str("];\n");

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
    fs::write(out_dir.join("embedded_assets.rs"), manifest)
        .expect("failed to write the embedded-asset manifest");
}

/// Collect every file under `dir` as a `(url-path, absolute-file-path)` pair,
/// where the url-path is the file's location relative to `root`, rooted at `/`
/// with forward slashes — the exact path a browser requests it by.
fn collect(root: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
    let entries = fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", dir.display()));
    for entry in entries {
        let path = entry.expect("failed to read a directory entry").path();
        if path.is_dir() {
            collect(root, &path, out);
        } else {
            let relative = path
                .strip_prefix(root)
                .expect("walked path is under the root");
            let url_path = format!("/{}", relative.to_string_lossy().replace('\\', "/"));
            println!("cargo:rerun-if-changed={}", path.display());
            out.push((url_path, path.to_string_lossy().into_owned()));
        }
    }
}
