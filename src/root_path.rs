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
        let root_relative = absolute.strip_prefix(root).map_err(|_| {
            anyhow!(
                "file path {} is not under {}",
                file_path.display(),
                root.display()
            )
        })?;
        normalize_relative_path(root_relative)?
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
}
