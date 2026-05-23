use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result, bail};

static TEMPORARY_PATH_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Default)]
pub struct FileRewriteTransaction {
    // All writes are staged before any visible mutation; removals run last.
    writes: Vec<FileRewrite>,
    removals: Vec<PathBuf>,
}

struct FileRewrite {
    path: PathBuf,
    content: String,
    temporary_path: Option<PathBuf>,
}

impl FileRewriteTransaction {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn write(&mut self, path: impl Into<PathBuf>, content: impl Into<String>) {
        self.writes.push(FileRewrite {
            path: path.into(),
            content: content.into(),
            temporary_path: None,
        });
    }

    pub fn remove(&mut self, path: impl Into<PathBuf>) {
        self.removals.push(path.into());
    }

    pub fn commit(mut self) -> Result<()> {
        self.validate()?;

        for write in &mut self.writes {
            write.stage()?;
        }

        for write in &mut self.writes {
            write.commit()?;
        }

        for path in &self.removals {
            fs::remove_file(path)
                .with_context(|| format!("failed to remove {}", path.display()))?;
        }

        Ok(())
    }

    fn validate(&self) -> Result<()> {
        let mut write_paths = HashSet::with_capacity(self.writes.len());
        for write in &self.writes {
            if !write_paths.insert(write.path.clone()) {
                bail!("duplicate write target {}", write.path.display());
            }
        }

        let mut removal_paths = HashSet::with_capacity(self.removals.len());
        for path in &self.removals {
            if !removal_paths.insert(path.clone()) {
                bail!("duplicate removal target {}", path.display());
            }
            if write_paths.contains(path) {
                bail!("cannot write and remove {}", path.display());
            }
        }

        Ok(())
    }
}

impl FileRewrite {
    fn stage(&mut self) -> Result<()> {
        let replacement_permissions = replacement_permissions(&self.path)?;

        if let Some(parent) = parent_dir(&self.path) {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create directory {}", parent.display()))?;
        }

        for _ in 0..16 {
            let temporary_path = temporary_path_for(&self.path)?;
            let mut file = match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary_path)
            {
                Ok(file) => file,
                Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("failed to stage {}", self.path.display()));
                }
            };

            let stage_result = (|| -> Result<()> {
                file.write_all(self.content.as_bytes())?;
                if let Some(permissions) = replacement_permissions.clone() {
                    fs::set_permissions(&temporary_path, permissions)?;
                }
                file.flush()?;
                Ok(())
            })();

            if let Err(error) = stage_result {
                drop(file);
                let _ = fs::remove_file(&temporary_path);
                return Err(error)
                    .with_context(|| format!("failed to stage {}", self.path.display()));
            }

            self.temporary_path = Some(temporary_path);
            return Ok(());
        }

        bail!(
            "failed to allocate temporary write path for {}",
            self.path.display()
        )
    }

    fn commit(&mut self) -> Result<()> {
        let temporary_path = self
            .temporary_path
            .as_ref()
            .context("write was not staged before commit")?;
        replace_file(temporary_path, &self.path)?;
        self.temporary_path = None;
        Ok(())
    }
}

impl Drop for FileRewrite {
    fn drop(&mut self) {
        if let Some(temporary_path) = &self.temporary_path {
            let _ = fs::remove_file(temporary_path);
        }
    }
}

fn temporary_path_for(path: &Path) -> Result<PathBuf> {
    let parent = parent_dir(path).unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .context("write target must name a file")?
        .to_string_lossy();
    let suffix = TEMPORARY_PATH_COUNTER.fetch_add(1, Ordering::Relaxed);
    Ok(parent.join(format!(
        ".{file_name}.org-slipbox-{}-{suffix}.tmp",
        std::process::id()
    )))
}

fn replacement_permissions(path: &Path) -> Result<Option<fs::Permissions>> {
    match fs::metadata(path) {
        Ok(metadata) => {
            let permissions = metadata.permissions();
            if permissions.readonly() {
                bail!("{} is read-only", path.display());
            }
            Ok(Some(permissions))
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("failed to inspect {}", path.display())),
    }
}

fn parent_dir(path: &Path) -> Option<&Path> {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
}

fn replace_file(temporary_path: &Path, target_path: &Path) -> Result<()> {
    #[cfg(windows)]
    if target_path.exists() {
        fs::remove_file(target_path)
            .with_context(|| format!("failed to replace {}", target_path.display()))?;
    }

    fs::rename(temporary_path, target_path)
        .with_context(|| format!("failed to replace {}", target_path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn writes_before_removing_sources() -> Result<()> {
        let workspace = tempdir()?;
        let source_path = workspace.path().join("source.org");
        let target_path = workspace.path().join("target.org");
        fs::write(&source_path, "source")?;

        let mut transaction = FileRewriteTransaction::new();
        transaction.write(&target_path, "target");
        transaction.remove(&source_path);
        transaction.commit()?;

        assert_eq!(fs::read_to_string(&target_path)?, "target");
        assert!(!source_path.exists());
        Ok(())
    }

    #[test]
    fn staging_failure_leaves_removals_untouched() -> Result<()> {
        let workspace = tempdir()?;
        let source_path = workspace.path().join("source.org");
        let blocked_parent = workspace.path().join("blocked");
        let target_path = blocked_parent.join("target.org");
        fs::write(&source_path, "source")?;
        fs::write(&blocked_parent, "not a directory")?;

        let mut transaction = FileRewriteTransaction::new();
        transaction.write(target_path, "target");
        transaction.remove(&source_path);

        assert!(transaction.commit().is_err());
        assert_eq!(fs::read_to_string(&source_path)?, "source");
        Ok(())
    }

    #[test]
    fn failed_validation_does_not_leave_temporary_files() -> Result<()> {
        let workspace = tempdir()?;
        let path = workspace.path().join("note.org");

        let mut transaction = FileRewriteTransaction::new();
        transaction.write(&path, "one");
        transaction.write(&path, "two");

        assert!(transaction.commit().is_err());
        let remaining_paths = fs::read_dir(workspace.path())?.count();
        assert_eq!(remaining_paths, 0);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn replacement_preserves_existing_permissions() -> Result<()> {
        use std::os::unix::fs::PermissionsExt;

        let workspace = tempdir()?;
        let path = workspace.path().join("note.org");
        fs::write(&path, "old")?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;

        let mut transaction = FileRewriteTransaction::new();
        transaction.write(&path, "new");
        transaction.commit()?;

        assert_eq!(fs::read_to_string(&path)?, "new");
        assert_eq!(fs::metadata(&path)?.permissions().mode() & 0o777, 0o600);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn read_only_replacement_is_rejected() -> Result<()> {
        use std::os::unix::fs::PermissionsExt;

        let workspace = tempdir()?;
        let path = workspace.path().join("note.org");
        fs::write(&path, "old")?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o444))?;

        let mut transaction = FileRewriteTransaction::new();
        transaction.write(&path, "new");

        assert!(transaction.commit().is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644))?;
        assert_eq!(fs::read_to_string(&path)?, "old");
        Ok(())
    }
}
