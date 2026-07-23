use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, anyhow};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRootPath {
    pub relative_path: String,
    pub absolute_path: PathBuf,
}

pub fn resolve_root_path(root: &Path, file_path: &Path) -> Result<ResolvedRootPath> {
    let root = root
        .canonicalize()
        .with_context(|| format!("failed to canonicalize root {}", root.display()))?;
    resolve_root_path_from_canonical_root(&root, file_path)
}

pub fn resolve_root_path_from_canonical_root(
    root: &Path,
    file_path: &Path,
) -> Result<ResolvedRootPath> {
    let relative = if file_path.is_absolute() {
        let absolute = normalize_absolute_path(file_path)?;
        let root_relative = root_relative_tail(root, &absolute).ok_or_else(|| {
            anyhow!(
                "file path {} is not under {}",
                file_path.display(),
                root.display()
            )
        })?;
        normalize_relative_path(&root_relative)?
    } else {
        normalize_relative_path(file_path)?
    };

    reject_symlink_components(root, &relative)?;
    let relative_path = relative.to_string_lossy().replace('\\', "/");
    Ok(ResolvedRootPath {
        absolute_path: root.join(relative),
        relative_path,
    })
}

/// Split an absolute path into the tail that follows `root`, or `None` when the
/// path lies elsewhere.
///
/// `root` is canonical, so a path spelled through a symlinked ancestor cannot be
/// compared to it lexically (`/var/folders/x/root` and
/// `/private/var/folders/x/root` are the same place); successively longer
/// ancestors are canonicalized until one resolves to `root`. The walk stops at
/// that first match, leaving the tail exactly as spelled for
/// `reject_symlink_components` to inspect.
fn root_relative_tail(root: &Path, absolute: &Path) -> Option<PathBuf> {
    if let Ok(tail) = absolute.strip_prefix(root) {
        return Some(tail.to_path_buf());
    }

    let components: Vec<Component> = absolute.components().collect();
    for boundary in 1..components.len() {
        let ancestor: PathBuf = components[..boundary].iter().collect();
        match ancestor.canonicalize() {
            Ok(resolved) if resolved == root => {
                return Some(components[boundary..].iter().collect());
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
    None
}

fn normalize_absolute_path(path: &Path) -> Result<PathBuf> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(anyhow!(
                    "file path {} must not contain parent-directory components",
                    path.display()
                ));
            }
            Component::Normal(part) => normalized.push(part),
            Component::RootDir | Component::Prefix(_) => normalized.push(component.as_os_str()),
        }
    }
    Ok(normalized)
}

fn normalize_relative_path(path: &Path) -> Result<PathBuf> {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => normalized.push(part),
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(anyhow!(
                    "file path {} must stay within the slipbox root",
                    path.display()
                ));
            }
        }
    }
    if normalized.as_os_str().is_empty() {
        return Err(anyhow!("file path must not be empty"));
    }
    Ok(normalized)
}

fn reject_symlink_components(root: &Path, relative: &Path) -> Result<()> {
    let mut absolute = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(part) = component else {
            continue;
        };
        absolute.push(part);
        match fs::symlink_metadata(&absolute) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(anyhow!(
                    "file path {} crosses symlink component {}",
                    root.join(relative).display(),
                    absolute.display()
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => break,
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("failed to inspect path component {}", absolute.display())
                });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use anyhow::Result;

    use super::resolve_root_path;

    #[test]
    fn resolver_accepts_new_paths_under_real_directories() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("notes"))?;

        let resolved = resolve_root_path(&root, "notes/new.org".as_ref())?;

        assert_eq!(resolved.relative_path, "notes/new.org");
        assert_eq!(
            resolved.absolute_path,
            root.canonicalize()?.join("notes/new.org")
        );
        Ok(())
    }

    #[test]
    fn resolver_accepts_absolute_paths_spelled_through_an_unresolved_root() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("notes"))?;
        fs::write(root.join("notes/a.org"), "#+title: A\n")?;

        let resolved = resolve_root_path(&root, &root.join("notes/a.org"))?;

        assert_eq!(resolved.relative_path, "notes/a.org");
        assert_eq!(
            resolved.absolute_path,
            root.canonicalize()?.join("notes/a.org")
        );
        Ok(())
    }

    #[test]
    fn resolver_accepts_absolute_paths_to_files_that_do_not_exist_yet() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let root = temp.path().join("root");
        fs::create_dir_all(root.join("notes"))?;

        let resolved = resolve_root_path(&root, &root.join("notes/new.org"))?;

        assert_eq!(resolved.relative_path, "notes/new.org");
        assert_eq!(
            resolved.absolute_path,
            root.canonicalize()?.join("notes/new.org")
        );
        Ok(())
    }

    #[test]
    fn resolver_rejects_absolute_paths_outside_the_root() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let root = temp.path().join("root");
        fs::create_dir_all(&root)?;
        let outside = temp.path().join("outside.org");
        fs::write(&outside, "#+title: Outside\n")?;

        let error = resolve_root_path(&root, &outside).unwrap_err();

        assert!(error.to_string().contains("is not under"));
        Ok(())
    }

    #[test]
    fn resolver_rejects_parent_components() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let root = temp.path().join("root");
        fs::create_dir_all(&root)?;

        let error = resolve_root_path(&root, "../outside.org".as_ref()).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("must stay within the slipbox root")
        );
        Ok(())
    }

    #[test]
    fn resolver_rejects_parent_components_in_absolute_paths() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let root = temp.path().join("root");
        fs::create_dir_all(&root)?;

        let error = resolve_root_path(&root, &root.join("../outside.org")).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("must not contain parent-directory components")
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn resolver_rejects_symlink_components() -> Result<()> {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir()?;
        let root = temp.path().join("root");
        let outside = temp.path().join("outside");
        fs::create_dir_all(&root)?;
        fs::create_dir_all(&outside)?;
        symlink(&outside, root.join("linked"))?;

        let error = resolve_root_path(&root, "linked/escape.org".as_ref()).unwrap_err();

        assert!(error.to_string().contains("crosses symlink component"));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn resolver_rejects_symlink_components_in_absolute_paths() -> Result<()> {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir()?;
        let root = temp.path().join("root");
        let outside = temp.path().join("outside");
        fs::create_dir_all(&root)?;
        fs::create_dir_all(&outside)?;
        symlink(&outside, root.join("linked"))?;

        let error = resolve_root_path(&root, &root.join("linked/escape.org")).unwrap_err();

        assert!(error.to_string().contains("crosses symlink component"));
        Ok(())
    }
}
