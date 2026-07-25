//! The static reading-client assets served beneath the JSON API.
//!
//! Assets resolve by exact key against an in-memory map and are never touched on
//! the filesystem, so a crafted path such as `/assets/../secret` simply misses.
//! Under the `embed-assets` feature the build script compiles the built
//! `client/dist` tree in; without it the set is empty and the server answers
//! every non-API path with a not-found.

use std::borrow::Cow;
use std::collections::HashMap;

/// The bytes to send for one static path, with their wire metadata.
pub struct StaticResponse<'a> {
    pub bytes: &'a [u8],
    pub content_type: &'static str,
    pub cache_control: &'static str,
}

/// The static assets of the reading client, keyed by the absolute URL path a
/// browser requests the file by: `/index.html`, `/assets/index-abc123.js`.
pub struct Assets {
    files: HashMap<String, Cow<'static, [u8]>>,
}

impl Assets {
    /// The client compiled into the binary under the `embed-assets` feature.
    #[cfg(feature = "embed-assets")]
    #[must_use]
    pub fn embedded() -> Self {
        mod generated {
            include!(concat!(env!("OUT_DIR"), "/embedded_assets.rs"));
        }
        Self {
            files: generated::ASSETS
                .iter()
                .map(|(path, bytes)| ((*path).to_owned(), Cow::Borrowed(*bytes)))
                .collect(),
        }
    }

    /// An empty set: this binary was built without the client.
    #[cfg(not(feature = "embed-assets"))]
    #[must_use]
    pub fn embedded() -> Self {
        Self {
            files: HashMap::new(),
        }
    }

    /// Build a set directly from `(path, bytes)` pairs.
    #[must_use]
    pub fn in_memory(
        files: impl IntoIterator<Item = (impl Into<String>, impl Into<Vec<u8>>)>,
    ) -> Self {
        Self {
            files: files
                .into_iter()
                .map(|(path, bytes)| (path.into(), Cow::Owned(bytes.into())))
                .collect(),
        }
    }

    /// True when no client is embedded.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Resolve a request path, with its query already removed, to the static
    /// response that answers it.
    ///
    /// A miss whose final segment names a file (`foo.js`) is a not-found, since
    /// the app shell must never stand in for a missing script or font the browser
    /// would then mis-decode; any other miss is a client route and falls back to
    /// the shell.
    #[must_use]
    pub fn resolve(&self, path: &str) -> Option<StaticResponse<'_>> {
        if self.files.is_empty() {
            return None;
        }
        let key = if path == "/" { "/index.html" } else { path };
        if let Some(bytes) = self.files.get(key) {
            return Some(StaticResponse {
                bytes,
                content_type: content_type_for(key),
                cache_control: cache_control_for(key),
            });
        }
        if names_a_file(key) {
            return None;
        }
        let index = self.files.get("/index.html")?;
        Some(StaticResponse {
            bytes: index,
            content_type: content_type_for("/index.html"),
            cache_control: cache_control_for("/index.html"),
        })
    }
}

/// True when a path's final segment carries an extension.
fn names_a_file(path: &str) -> bool {
    path.rsplit('/')
        .next()
        .is_some_and(|segment| segment.contains('.'))
}

/// The `Content-Type` for a path, chosen by extension.
fn content_type_for(path: &str) -> &'static str {
    match extension(path) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") | Some("map") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("woff2") => "font/woff2",
        Some("woff") => "font/woff",
        Some("ttf") => "font/ttf",
        _ => "application/octet-stream",
    }
}

/// The `Cache-Control` for a path. Files under `/assets/` carry a content hash in
/// their name, so a given URL's bytes never change; everything else is
/// revalidated so a rebuilt client is picked up.
fn cache_control_for(path: &str) -> &'static str {
    if path.starts_with("/assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    }
}

/// The extension of a path's final segment, if any.
fn extension(path: &str) -> Option<&str> {
    path.rsplit('/')
        .next()?
        .rsplit_once('.')
        .map(|(_, ext)| ext)
}

#[cfg(test)]
mod tests {
    use super::Assets;

    fn client() -> Assets {
        Assets::in_memory([
            ("/index.html", b"<!doctype html>shell".to_vec()),
            ("/assets/index-abc123.js", b"console.log(1)".to_vec()),
            ("/assets/index-def456.css", b".a{}".to_vec()),
        ])
    }

    #[test]
    fn serves_the_shell_at_the_root() {
        let client = client();
        let response = client.resolve("/").expect("root serves the shell");
        assert_eq!(response.bytes, b"<!doctype html>shell");
        assert_eq!(response.content_type, "text/html; charset=utf-8");
        assert_eq!(response.cache_control, "no-cache");
    }

    #[test]
    fn serves_a_hashed_asset_as_immutable() {
        let client = client();
        let response = client
            .resolve("/assets/index-abc123.js")
            .expect("the hashed script resolves");
        assert_eq!(response.bytes, b"console.log(1)");
        assert_eq!(response.content_type, "text/javascript; charset=utf-8");
        assert_eq!(
            response.cache_control,
            "public, max-age=31536000, immutable"
        );
    }

    #[test]
    fn falls_back_to_the_shell_for_a_client_route() {
        let client = client();
        let response = client
            .resolve("/some/reading/path")
            .expect("a client route falls back to the shell");
        assert_eq!(response.bytes, b"<!doctype html>shell");
        assert_eq!(response.content_type, "text/html; charset=utf-8");
    }

    #[test]
    fn a_missing_file_is_not_found_not_the_shell() {
        assert!(client().resolve("/assets/missing.js").is_none());
    }

    #[test]
    fn traversal_paths_simply_miss() {
        assert!(client().resolve("/assets/../secret.js").is_none());
    }

    #[test]
    fn an_empty_set_resolves_nothing() {
        let empty = Assets::in_memory(std::iter::empty::<(String, Vec<u8>)>());
        assert!(empty.is_empty());
        assert!(empty.resolve("/").is_none());
    }
}
