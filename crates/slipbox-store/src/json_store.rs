use std::fs;
use std::io::Write;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Serialize, de::DeserializeOwned};
use urlencoding::encode;

#[derive(Debug, Clone, Copy)]
pub(crate) struct JsonStoreSpec {
    pub(crate) directory_suffix: &'static str,
    pub(crate) layout_version: &'static str,
    pub(crate) file_extension: &'static str,
    pub(crate) item_name: &'static str,
    pub(crate) plural_name: &'static str,
    pub(crate) id_field_name: &'static str,
    pub(crate) default_database_file_name: &'static str,
}

pub(crate) struct JsonFileStore<T> {
    root: PathBuf,
    spec: JsonStoreSpec,
    _marker: PhantomData<T>,
}

impl<T> JsonFileStore<T>
where
    T: DeserializeOwned + Serialize,
{
    pub(crate) fn for_database_path(path: &Path, spec: JsonStoreSpec) -> Self {
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(spec.default_database_file_name);
        Self {
            root: path.with_file_name(format!("{file_name}{}", spec.directory_suffix)),
            spec,
            _marker: PhantomData,
        }
    }

    pub(crate) fn migrate(&self) -> Result<()> {
        fs::create_dir_all(self.version_dir()).with_context(|| {
            format!(
                "failed to create {} store {}",
                self.spec.item_name,
                self.version_dir().display()
            )
        })?;
        Ok(())
    }

    pub(crate) fn version_dir(&self) -> PathBuf {
        self.root.join(self.spec.layout_version)
    }

    fn item_path(&self, item_id: &str) -> PathBuf {
        self.version_dir()
            .join(format!("{}.{}", encode(item_id), self.spec.file_extension))
    }

    #[cfg(test)]
    pub(crate) fn path_for_id(&self, item_id: &str) -> PathBuf {
        self.item_path(item_id)
    }

    fn temporary_item_path(&self, item_id: &str) -> PathBuf {
        self.version_dir().join(format!(
            ".{}.tmp-{}-{}",
            encode(item_id),
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ))
    }

    fn validate_id(&self, item_id: &str) -> Result<()> {
        if item_id.trim().is_empty() {
            anyhow::bail!("{} must not be empty", self.spec.id_field_name);
        }
        if item_id.trim() != item_id {
            anyhow::bail!(
                "{} must not have leading or trailing whitespace",
                self.spec.id_field_name
            );
        }
        Ok(())
    }

    fn load_file_with<F, V>(&self, path: &Path, id_of: &F, validate: &V) -> Result<T>
    where
        F: Fn(&T) -> &str,
        V: Fn(&T) -> Option<String>,
    {
        let contents = fs::read_to_string(path).with_context(|| {
            format!("failed to read {} {}", self.spec.item_name, path.display())
        })?;
        let item = serde_json::from_str::<T>(&contents).with_context(|| {
            format!("failed to parse {} {}", self.spec.item_name, path.display())
        })?;
        if let Some(error) = validate(&item) {
            anyhow::bail!(
                "stored {} {} is invalid: {}",
                self.spec.item_name,
                path.display(),
                error
            );
        }
        let item_id = id_of(&item);
        let expected_path = self.item_path(item_id);
        if expected_path != path {
            anyhow::bail!(
                "stored {} {} does not match {} {}",
                self.spec.item_name,
                path.display(),
                self.spec.id_field_name,
                item_id
            );
        }
        Ok(item)
    }

    pub(crate) fn load_file<F, V>(&self, path: &Path, id_of: F, validate: V) -> Result<T>
    where
        F: Fn(&T) -> &str,
        V: Fn(&T) -> Option<String>,
    {
        self.load_file_with(path, &id_of, &validate)
    }

    pub(crate) fn list_paths(&self) -> Result<Vec<PathBuf>> {
        let mut paths = Vec::new();
        if !self.version_dir().exists() {
            return Ok(paths);
        }

        for entry in fs::read_dir(self.version_dir()).with_context(|| {
            format!(
                "failed to list {} in {}",
                self.spec.plural_name,
                self.version_dir().display()
            )
        })? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) == Some(self.spec.file_extension) {
                paths.push(path);
            }
        }

        paths.sort();
        Ok(paths)
    }

    fn write_temporary_file(&self, item_id: &str, item: &T) -> Result<PathBuf> {
        let temporary_path = self.temporary_item_path(item_id);
        let json = serde_json::to_vec_pretty(item)
            .with_context(|| format!("failed to serialize {}", self.spec.item_name))?;
        let mut file = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
        {
            Ok(file) => file,
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "failed to create temporary {} {}",
                        self.spec.item_name,
                        temporary_path.display()
                    )
                });
            }
        };
        if let Err(error) = file.write_all(&json).and_then(|_| file.sync_all()) {
            drop(file);
            let _ = fs::remove_file(&temporary_path);
            return Err(error).with_context(|| {
                format!(
                    "failed to write temporary {} {}",
                    self.spec.item_name,
                    temporary_path.display()
                )
            });
        }
        Ok(temporary_path)
    }

    pub(crate) fn save<V>(&self, item_id: &str, item: &T, validate: V) -> Result<()>
    where
        V: Fn(&T) -> Option<String>,
    {
        if let Some(error) = validate(item) {
            anyhow::bail!("{} is invalid: {error}", self.spec.item_name);
        }

        let path = self.item_path(item_id);
        let temporary_path = self.write_temporary_file(item_id, item)?;
        #[cfg(windows)]
        if path.exists() {
            fs::remove_file(&path).with_context(|| {
                format!(
                    "failed to replace existing {} {}",
                    self.spec.item_name,
                    path.display()
                )
            })?;
        }
        if let Err(error) = fs::rename(&temporary_path, &path) {
            let _ = fs::remove_file(&temporary_path);
            return Err(error).with_context(|| {
                format!(
                    "failed to finalize {} {}",
                    self.spec.item_name,
                    path.display()
                )
            });
        }
        Ok(())
    }

    pub(crate) fn save_if_absent<V>(&self, item_id: &str, item: &T, validate: V) -> Result<bool>
    where
        V: Fn(&T) -> Option<String>,
    {
        if let Some(error) = validate(item) {
            anyhow::bail!("{} is invalid: {error}", self.spec.item_name);
        }

        let path = self.item_path(item_id);
        let temporary_path = self.write_temporary_file(item_id, item)?;
        match fs::hard_link(&temporary_path, &path) {
            Ok(()) => {
                fs::remove_file(&temporary_path).with_context(|| {
                    format!(
                        "failed to clean up temporary {} {}",
                        self.spec.item_name,
                        temporary_path.display()
                    )
                })?;
                Ok(true)
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let _ = fs::remove_file(&temporary_path);
                Ok(false)
            }
            Err(error) => {
                let _ = fs::remove_file(&temporary_path);
                Err(error).with_context(|| {
                    format!(
                        "failed to finalize new {} {}",
                        self.spec.item_name,
                        path.display()
                    )
                })
            }
        }
    }

    pub(crate) fn load<F, V>(&self, item_id: &str, id_of: F, validate: V) -> Result<Option<T>>
    where
        F: Fn(&T) -> &str,
        V: Fn(&T) -> Option<String>,
    {
        self.validate_id(item_id)?;
        let path = self.item_path(item_id);
        if !path.exists() {
            return Ok(None);
        }
        self.load_file(path.as_path(), id_of, validate).map(Some)
    }

    pub(crate) fn list<F, V>(&self, id_of: F, validate: V) -> Result<Vec<T>>
    where
        F: Fn(&T) -> &str,
        V: Fn(&T) -> Option<String>,
    {
        self.list_paths()?
            .into_iter()
            .map(|path| self.load_file_with(&path, &id_of, &validate))
            .collect()
    }

    pub(crate) fn delete(&self, item_id: &str) -> Result<bool> {
        self.validate_id(item_id)?;
        let path = self.item_path(item_id);
        if !path.exists() {
            return Ok(false);
        }
        fs::remove_file(&path).with_context(|| {
            format!(
                "failed to delete {} {}",
                self.spec.item_name,
                path.display()
            )
        })?;
        Ok(true)
    }
}
