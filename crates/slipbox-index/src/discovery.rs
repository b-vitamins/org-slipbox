use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use regex::Regex;
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct DiscoveryPolicy {
    file_extensions: Vec<String>,
    exclude_regexps: Vec<Regex>,
}

impl Default for DiscoveryPolicy {
    fn default() -> Self {
        Self::new([String::from("org")], std::iter::empty::<String>())
            .expect("default discovery policy should be valid")
    }
}

impl DiscoveryPolicy {
    pub fn new<I, J, S, T>(file_extensions: I, exclude_regexps: J) -> Result<Self>
    where
        I: IntoIterator<Item = S>,
        J: IntoIterator<Item = T>,
        S: AsRef<str>,
        T: AsRef<str>,
    {
        let file_extensions = normalize_extensions(file_extensions);
        let exclude_regexps = exclude_regexps
            .into_iter()
            .filter_map(|pattern| {
                let pattern = pattern.as_ref().trim();
                (!pattern.is_empty()).then_some(pattern.to_owned())
            })
            .map(|pattern| {
                Regex::new(&pattern)
                    .with_context(|| format!("invalid file exclude regexp {pattern:?}"))
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(Self {
            file_extensions,
            exclude_regexps,
        })
    }

    #[must_use]
    pub fn file_extensions(&self) -> &[String] {
        &self.file_extensions
    }

    #[must_use]
    pub fn matches_path(&self, root: &Path, path: &Path) -> bool {
        let Some(relative_path) = relative_path(root, path) else {
            return false;
        };
        let Some(extension) = base_extension(path) else {
            return false;
        };
        let included = self
            .file_extensions
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(&extension));
        included
            && !self
                .exclude_regexps
                .iter()
                .any(|pattern| pattern.is_match(&relative_path))
    }

    pub fn list_files(&self, root: &Path) -> Result<Vec<PathBuf>> {
        let mut paths = WalkDir::new(root)
            .follow_links(false)
            .sort_by_file_name()
            .into_iter()
            .filter_map(|entry| match entry {
                Ok(entry)
                    if entry.file_type().is_file() && self.matches_path(root, entry.path()) =>
                {
                    Some(Ok(entry.into_path()))
                }
                Ok(_) => None,
                Err(error) => Some(Err(error)),
            })
            .collect::<std::result::Result<Vec<_>, _>>()
            .context("failed while traversing slipbox files")?;
        paths.sort();
        Ok(paths)
    }
}

pub(crate) fn relative_path(root: &Path, path: &Path) -> Option<String> {
    path.strip_prefix(root)
        .ok()
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
}

pub(crate) fn base_extension(path: &Path) -> Option<String> {
    let extension = path.extension()?.to_str()?;
    if extension.eq_ignore_ascii_case("gpg") || extension.eq_ignore_ascii_case("age") {
        return path.file_stem().map(Path::new).and_then(base_extension);
    }
    Some(extension.to_ascii_lowercase())
}

pub(crate) fn envelope_extension(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
}

pub(crate) fn default_file_stem(path: &Path) -> Option<String> {
    if matches!(envelope_extension(path).as_deref(), Some("gpg" | "age")) {
        return path
            .file_stem()
            .map(PathBuf::from)
            .and_then(|stem| stem.file_stem().map(|value| value.to_owned()))
            .and_then(|stem| stem.to_str().map(ToOwned::to_owned));
    }
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(ToOwned::to_owned)
}

fn normalize_extensions<I, S>(extensions: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut normalized = Vec::new();
    for extension in extensions {
        let extension = extension.as_ref().trim().trim_start_matches('.');
        if extension.is_empty()
            || normalized
                .iter()
                .any(|candidate: &String| candidate.eq_ignore_ascii_case(extension))
        {
            continue;
        }
        normalized.push(extension.to_ascii_lowercase());
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::DiscoveryPolicy;
    use anyhow::Result;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn matches_configured_extensions_with_encrypted_suffixes() -> Result<()> {
        let workspace = tempdir()?;
        let root = workspace.path();
        let org = root.join("note.org");
        let gpg = root.join("secret.org.gpg");
        let age = root.join("locked.org.age");
        let markdown = root.join("readme.md");

        fs::write(&org, "")?;
        fs::write(&gpg, "")?;
        fs::write(&age, "")?;
        fs::write(&markdown, "")?;

        let policy = DiscoveryPolicy::new(["org"], std::iter::empty::<&str>())?;
        assert!(policy.matches_path(root, &org));
        assert!(policy.matches_path(root, &gpg));
        assert!(policy.matches_path(root, &age));
        assert!(!policy.matches_path(root, &markdown));

        Ok(())
    }

    #[test]
    fn excludes_paths_by_relative_regexp() -> Result<()> {
        let workspace = tempdir()?;
        let root = workspace.path();
        let note = root.join("notes").join("keep.org");
        let archive = root.join("archive").join("skip.org");

        fs::create_dir_all(note.parent().expect("note parent"))?;
        fs::create_dir_all(archive.parent().expect("archive parent"))?;
        fs::write(&note, "")?;
        fs::write(&archive, "")?;

        let policy = DiscoveryPolicy::new(["org"], ["^archive/"])?;
        let files = policy.list_files(root)?;

        assert_eq!(files, vec![note]);
        assert!(policy.matches_path(root, &root.join("notes").join("keep.org")));
        assert!(!policy.matches_path(root, &root.join("archive").join("skip.org")));

        Ok(())
    }
}
