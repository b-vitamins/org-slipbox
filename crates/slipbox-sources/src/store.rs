use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use slipbox_core::{SourceId, SourceRecord};
use thiserror::Error;

use crate::{SourceCatalog, SourceCatalogError};

/// The one document version this build writes and reads. A document declaring
/// any other version is refused and left untouched.
pub const SOURCE_CATALOG_VERSION: u32 = 1;

/// Largest catalog document that is read or written. A larger file is refused
/// rather than parsed, so a corrupt or hostile document cannot exhaust memory.
pub const MAX_CATALOG_DOCUMENT_BYTES: u64 = 256 * 1024;

const TEMPORARY_FILE_ATTEMPTS: u32 = 32;

/// The revision of a stored catalog document.
///
/// It counts committed writes to one path and orders nothing else. Every save
/// states the revision it read, and the store refuses a save whose revision is
/// no longer current.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CatalogRevision(u64);

impl CatalogRevision {
    /// The revision a caller states when it believes no document exists yet.
    pub const ABSENT: Self = Self(0);

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// The next revision, or `None` once the count is exhausted. Wrapping or
    /// saturating here would let a save that states an old revision pass the
    /// staleness check.
    fn next(self) -> Option<Self> {
        self.0.checked_add(1).map(Self)
    }
}

impl fmt::Display for CatalogRevision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

/// A catalog as stored, with the revision a later save must state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredCatalog {
    revision: CatalogRevision,
    catalog: SourceCatalog,
}

impl StoredCatalog {
    #[must_use]
    pub fn revision(&self) -> CatalogRevision {
        self.revision
    }

    #[must_use]
    pub fn catalog(&self) -> &SourceCatalog {
        &self.catalog
    }

    #[must_use]
    pub fn into_catalog(self) -> SourceCatalog {
        self.catalog
    }
}

/// Durable storage for one catalog document at a path the caller owns.
///
/// The caller owns the private-storage path and its existing parent directory.
/// This document is not derived from Org files.
///
/// One writer must own each path. Revision checks detect committed competing
/// writes, but do not provide mutual exclusion.
#[derive(Debug, Clone)]
pub struct SourceCatalogStore {
    path: PathBuf,
    fault: Option<WriteStep>,
}

impl SourceCatalogStore {
    #[must_use]
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            fault: None,
        }
    }

    #[cfg(test)]
    fn failing_at(path: impl Into<PathBuf>, step: WriteStep) -> Self {
        Self {
            path: path.into(),
            fault: Some(step),
        }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Read the catalog, or `Ok(None)` if absent. Corrupt and unsupported
    /// documents are errors, never empty catalogs.
    pub fn load(&self) -> Result<Option<StoredCatalog>, SourceCatalogStoreError> {
        let Some(contents) = self.read_bounded()? else {
            return Ok(None);
        };

        let envelope = serde_json::from_str::<VersionEnvelope>(&contents)
            .map_err(|cause| self.malformed(&cause))?;
        if envelope.compatibility.version != SOURCE_CATALOG_VERSION {
            return Err(SourceCatalogStoreError::UnsupportedVersion {
                path: self.path.clone(),
                found: envelope.compatibility.version,
                supported: SOURCE_CATALOG_VERSION,
            });
        }

        let document = serde_json::from_str::<CatalogDocument>(&contents)
            .map_err(|cause| self.malformed(&cause))?;
        if document.revision == CatalogRevision::ABSENT {
            return Err(SourceCatalogStoreError::Malformed {
                path: self.path.clone(),
                cause: MalformedCatalog::Revision,
            });
        }
        let catalog = SourceCatalog::from_parts(document.sources, document.active_source).map_err(
            |cause| SourceCatalogStoreError::Invalid {
                path: self.path.clone(),
                cause,
            },
        )?;
        Ok(Some(StoredCatalog {
            revision: document.revision,
            catalog,
        }))
    }

    /// Write the catalog, returning the revision it was committed at.
    ///
    /// `expected` is the last-read revision, or [`CatalogRevision::ABSENT`].
    /// Stale, corrupt and unsupported documents are not overwritten.
    /// Pre-commit errors preserve the stored bytes and revision. Rename commits
    /// the write; [`SourceCatalogStoreError::NotDurable`] reports its new revision
    /// and requires reload rather than retrying the stale revision.
    pub fn save(
        &self,
        expected: CatalogRevision,
        catalog: &SourceCatalog,
    ) -> Result<CatalogRevision, SourceCatalogStoreError> {
        let found = self
            .load()?
            .map_or(CatalogRevision::ABSENT, |stored| stored.revision);
        if found != expected {
            return Err(SourceCatalogStoreError::StaleWrite { expected, found });
        }

        let Some(revision) = found.next() else {
            return Err(SourceCatalogStoreError::RevisionExhausted {
                path: self.path.clone(),
                revision: found,
            });
        };
        let document = CatalogDocument {
            compatibility: CatalogCompatibility {
                version: SOURCE_CATALOG_VERSION,
            },
            revision,
            active_source: catalog.active_id().cloned(),
            sources: catalog.sources().to_vec(),
        };
        let mut encoded =
            serde_json::to_string_pretty(&document).map_err(|_| SourceCatalogStoreError::Encode)?;
        encoded.push('\n');
        let found = encoded.len() as u64;
        if found > MAX_CATALOG_DOCUMENT_BYTES {
            return Err(SourceCatalogStoreError::TooLarge {
                path: self.path.clone(),
                limit: MAX_CATALOG_DOCUMENT_BYTES,
                found,
            });
        }

        if let Some(cause) = self.write_atomically(encoded.as_bytes())? {
            return Err(SourceCatalogStoreError::NotDurable {
                path: self.path.clone(),
                revision,
                cause,
            });
        }
        Ok(revision)
    }

    fn read_bounded(&self) -> Result<Option<String>, SourceCatalogStoreError> {
        let file = match File::open(&self.path) {
            Ok(file) => file,
            Err(cause) if cause.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(cause) => return Err(self.read_error(cause)),
        };
        let length = file
            .metadata()
            .map_err(|cause| self.read_error(cause))?
            .len();
        if length > MAX_CATALOG_DOCUMENT_BYTES {
            return Err(SourceCatalogStoreError::TooLarge {
                path: self.path.clone(),
                limit: MAX_CATALOG_DOCUMENT_BYTES,
                found: length,
            });
        }

        let mut contents = String::new();
        // Bytes that are not text are a corrupt document rather than a failure
        // to reach the file.
        let read = file
            .take(MAX_CATALOG_DOCUMENT_BYTES + 1)
            .read_to_string(&mut contents)
            .map_err(|cause| match cause.kind() {
                io::ErrorKind::InvalidData => SourceCatalogStoreError::Malformed {
                    path: self.path.clone(),
                    cause: MalformedCatalog::NotUtf8,
                },
                _ => self.read_error(cause),
            })? as u64;
        if read > MAX_CATALOG_DOCUMENT_BYTES {
            return Err(SourceCatalogStoreError::TooLarge {
                path: self.path.clone(),
                limit: MAX_CATALOG_DOCUMENT_BYTES,
                found: read,
            });
        }
        Ok(Some(contents))
    }

    /// Same-directory rename commits the replacement. `Ok(Some(cause))` means
    /// the new document is committed but directory sync failed.
    fn write_atomically(
        &self,
        contents: &[u8],
    ) -> Result<Option<io::Error>, SourceCatalogStoreError> {
        let directory = match self.path.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent,
            _ => Path::new("."),
        };
        let (temporary_path, mut file) = self.create_temporary_file(directory)?;

        let mut outcome = match self.injected(WriteStep::WriteTemporary) {
            Some(cause) => Err(cause),
            None => file.write_all(contents),
        };
        if outcome.is_ok() {
            outcome = match self.injected(WriteStep::SyncTemporary) {
                Some(cause) => Err(cause),
                None => file.sync_all(),
            };
        }
        drop(file);
        if outcome.is_ok() {
            outcome = match self.injected(WriteStep::Replace) {
                Some(cause) => Err(cause),
                None => fs::rename(&temporary_path, &self.path),
            };
        }
        if let Err(cause) = outcome {
            let _ = fs::remove_file(&temporary_path);
            return Err(self.write_error(cause));
        }

        #[cfg(unix)]
        {
            let synced = match self.injected(WriteStep::SyncDirectory) {
                Some(cause) => Err(cause),
                None => File::open(directory).and_then(|handle| handle.sync_all()),
            };
            if let Err(cause) = synced {
                return Ok(Some(cause));
            }
        }
        Ok(None)
    }

    fn injected(&self, step: WriteStep) -> Option<io::Error> {
        (self.fault == Some(step)).then(|| io::Error::other("an injected write failure"))
    }

    fn create_temporary_file(
        &self,
        directory: &Path,
    ) -> Result<(PathBuf, File), SourceCatalogStoreError> {
        if let Some(cause) = self.injected(WriteStep::CreateTemporary) {
            return Err(self.write_error(cause));
        }
        let file_name = self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("source-catalog.json");
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let mut last = None;
        for attempt in 0..TEMPORARY_FILE_ATTEMPTS {
            let candidate = directory.join(format!(
                ".{file_name}.tmp-{}-{stamp}-{attempt}",
                std::process::id()
            ));
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&candidate)
            {
                Ok(file) => return Ok((candidate, file)),
                Err(cause) if cause.kind() == io::ErrorKind::AlreadyExists => last = Some(cause),
                Err(cause) => return Err(self.write_error(cause)),
            }
        }
        Err(self.write_error(last.unwrap_or_else(|| {
            io::Error::new(io::ErrorKind::AlreadyExists, "no temporary name was free")
        })))
    }

    fn read_error(&self, cause: io::Error) -> SourceCatalogStoreError {
        SourceCatalogStoreError::Read {
            path: self.path.clone(),
            cause,
        }
    }

    fn write_error(&self, cause: io::Error) -> SourceCatalogStoreError {
        SourceCatalogStoreError::Write {
            path: self.path.clone(),
            cause,
        }
    }

    fn malformed(&self, cause: &serde_json::Error) -> SourceCatalogStoreError {
        SourceCatalogStoreError::Malformed {
            path: self.path.clone(),
            cause: MalformedCatalog::of(cause),
        }
    }
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WriteStep {
    CreateTemporary,
    WriteTemporary,
    SyncTemporary,
    Replace,
    SyncDirectory,
}

/// Why a stored document is not a catalog.
///
/// Categories and parser positions are retained, never rejected input or parser
/// messages that could leak secrets through formatting or the error chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum MalformedCatalog {
    #[error("the document is not UTF-8 text")]
    NotUtf8,
    #[error("the JSON at line {line} column {column} is invalid")]
    Syntax { line: usize, column: usize },
    #[error("the value at line {line} column {column} is not one this document admits")]
    Value { line: usize, column: usize },
    #[error("the document ends before the catalog does")]
    Truncated,
    #[error("a stored catalog revision starts at 1")]
    Revision,
}

impl MalformedCatalog {
    /// Classify a parse failure without retaining the parser's message, which
    /// quotes the input it refused.
    fn of(cause: &serde_json::Error) -> Self {
        let (line, column) = (cause.line(), cause.column());
        match cause.classify() {
            serde_json::error::Category::Eof => Self::Truncated,
            serde_json::error::Category::Data => Self::Value { line, column },
            // Reading from a string cannot fail, so a reported reader error is
            // treated as invalid syntax at the position reached.
            serde_json::error::Category::Syntax | serde_json::error::Category::Io => {
                Self::Syntax { line, column }
            }
        }
    }
}

#[derive(Debug, Error)]
pub enum SourceCatalogStoreError {
    #[error("failed to read the source catalog {}", path.display())]
    Read {
        path: PathBuf,
        #[source]
        cause: io::Error,
    },
    #[error("failed to write the source catalog {}", path.display())]
    Write {
        path: PathBuf,
        #[source]
        cause: io::Error,
    },
    /// Encoding a catalog of validated records has no failure that depends on
    /// their values, so no encoder text is retained.
    #[error("failed to encode the source catalog")]
    Encode,
    #[error(
        "the source catalog {} declares version {found}, and this build reads version {supported}",
        path.display()
    )]
    UnsupportedVersion {
        path: PathBuf,
        found: u32,
        supported: u32,
    },
    #[error("the source catalog {} is malformed: {cause}", path.display())]
    Malformed {
        path: PathBuf,
        cause: MalformedCatalog,
    },
    #[error(
        "the source catalog {} is {found} bytes, over the {limit} byte limit",
        path.display()
    )]
    TooLarge {
        path: PathBuf,
        limit: u64,
        found: u64,
    },
    #[error("the source catalog moved to revision {found} since revision {expected} was read")]
    StaleWrite {
        expected: CatalogRevision,
        found: CatalogRevision,
    },
    /// The revision count is exhausted. Refused before anything is written,
    /// because a revision that stopped advancing would accept a stale save.
    #[error("the source catalog {} has no revision after {revision}", path.display())]
    RevisionExhausted {
        path: PathBuf,
        revision: CatalogRevision,
    },
    /// The document was replaced but not confirmed durable. The catalog on the
    /// path is the new one at `revision`; a caller retries from there, never
    /// from the revision it read.
    #[error(
        "the source catalog {} was committed at revision {revision} without confirming it is durable",
        path.display()
    )]
    NotDurable {
        path: PathBuf,
        revision: CatalogRevision,
        #[source]
        cause: io::Error,
    },
    #[error("the source catalog {} is not a valid catalog: {cause}", path.display())]
    Invalid {
        path: PathBuf,
        #[source]
        cause: SourceCatalogError,
    },
}

/// The stored document. Field order is the serialized order, so the version is
/// readable before anything else in it.
#[derive(Debug, Serialize, Deserialize)]
struct CatalogDocument {
    compatibility: CatalogCompatibility,
    revision: CatalogRevision,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    active_source: Option<SourceId>,
    sources: Vec<SourceRecord>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CatalogCompatibility {
    version: u32,
}

/// The version alone, read before the typed document so an unsupported version
/// is refused as such rather than as a failure over fields it does not have.
#[derive(Debug, Deserialize)]
struct VersionEnvelope {
    compatibility: CatalogCompatibility,
}

#[cfg(test)]
mod tests {
    use slipbox_core::{
        GitBranch, NotesFolder, RemoteUrl, SOURCE_ID_ENTROPY_BYTES, SourceConfiguration,
        SourceDisplayName, SourceId, SourceProvider, SourceVisibility,
    };
    use tempfile::TempDir;

    use super::*;

    const FILE_NAME: &str = "source-catalog.json";

    fn record(seed: u8, remote: &str) -> SourceRecord {
        SourceRecord::new(SourceConfiguration {
            id: SourceId::mint([seed; SOURCE_ID_ENTROPY_BYTES]),
            display_name: SourceDisplayName::parse("Field notes").expect("a fixture label"),
            provider: SourceProvider::GenericHttps,
            visibility: SourceVisibility::Public,
            provider_repository_id: None,
            account: None,
            remote: RemoteUrl::parse(remote).expect("a fixture remote"),
            branch: GitBranch::parse("main").expect("a fixture branch"),
            notes_folder: NotesFolder::parse("notes").expect("a fixture folder"),
            credential: None,
        })
        .expect("a public fixture satisfies the record invariants")
    }

    fn catalog_of(records: impl IntoIterator<Item = SourceRecord>) -> SourceCatalog {
        let mut catalog = SourceCatalog::new();
        for record in records {
            catalog.add(record).expect("each fixture source is new");
        }
        catalog
    }

    fn one_source() -> SourceCatalog {
        catalog_of([record(0x41, "https://git.example.org/owner/first.git")])
    }

    fn two_sources() -> SourceCatalog {
        catalog_of([
            record(0x41, "https://git.example.org/owner/first.git"),
            record(0x42, "https://git.example.org/owner/second.git"),
        ])
    }

    struct Storage {
        _directory: TempDir,
        path: PathBuf,
    }

    impl Storage {
        fn empty() -> Self {
            let directory = tempfile::tempdir().expect("a temporary private storage directory");
            let path = directory.path().join(FILE_NAME);
            Self {
                _directory: directory,
                path,
            }
        }

        fn document(&self) -> Vec<u8> {
            fs::read(&self.path).expect("the stored document is readable")
        }

        fn entries(&self) -> Vec<String> {
            let mut names: Vec<String> = fs::read_dir(self.path.parent().expect("a parent"))
                .expect("the storage directory is readable")
                .map(|entry| {
                    entry
                        .expect("an entry is readable")
                        .file_name()
                        .to_string_lossy()
                        .into_owned()
                })
                .collect();
            names.sort();
            names
        }
    }

    #[test]
    fn a_failure_before_the_commit_leaves_the_document_and_revision_whole() {
        let storage = Storage::empty();
        let store = SourceCatalogStore::at(&storage.path);
        let committed = store
            .save(CatalogRevision::ABSENT, &one_source())
            .expect("a first write succeeds");
        let before = storage.document();
        let updated = two_sources();

        for step in [
            WriteStep::CreateTemporary,
            WriteStep::WriteTemporary,
            WriteStep::SyncTemporary,
            WriteStep::Replace,
        ] {
            let failing = SourceCatalogStore::failing_at(&storage.path, step);
            let error = failing
                .save(committed, &updated)
                .expect_err("the injected failure is reported");
            assert!(
                matches!(error, SourceCatalogStoreError::Write { .. }),
                "failing {step:?} reported {error:?}"
            );
            assert_eq!(
                storage.document(),
                before,
                "failing {step:?} changed the stored document"
            );
            assert_eq!(
                store
                    .load()
                    .expect("the document is readable")
                    .expect("the document exists")
                    .revision(),
                committed,
                "failing {step:?} advanced the revision"
            );
            assert_eq!(
                storage.entries(),
                vec![FILE_NAME.to_owned()],
                "failing {step:?} left a temporary file behind"
            );
        }

        assert_eq!(
            store
                .save(committed, &updated)
                .expect("the same save succeeds once nothing fails"),
            committed.next().expect("a next revision")
        );
        assert_ne!(storage.document(), before);
    }

    /// The directory sync is the only step after the commit, and it exists on
    /// Unix alone.
    #[cfg(unix)]
    #[test]
    fn a_commit_that_is_not_confirmed_durable_reports_the_revision_it_reached() {
        let storage = Storage::empty();
        let store = SourceCatalogStore::at(&storage.path);
        let read = store
            .save(CatalogRevision::ABSENT, &one_source())
            .expect("a first write succeeds");
        let updated = two_sources();

        let error = SourceCatalogStore::failing_at(&storage.path, WriteStep::SyncDirectory)
            .save(read, &updated)
            .expect_err("an unconfirmed commit is reported");
        let SourceCatalogStoreError::NotDurable { revision, .. } = error else {
            panic!("a post-commit failure was reported as {error:?}");
        };
        assert_eq!(revision, read.next().expect("a next revision"));

        let reopened = store
            .load()
            .expect("the document is readable")
            .expect("the document exists");
        assert_eq!(
            reopened.revision(),
            revision,
            "the reported commit is not the one on the path"
        );
        assert_eq!(reopened.catalog(), &updated);
        assert_eq!(storage.entries(), vec![FILE_NAME.to_owned()]);

        assert!(
            matches!(
                store.save(read, &updated),
                Err(SourceCatalogStoreError::StaleWrite { .. })
            ),
            "the revision the caller read was still accepted after the commit"
        );
        assert_eq!(
            store
                .save(revision, &updated)
                .expect("a retry from the committed revision succeeds"),
            revision.next().expect("a next revision")
        );
    }

    #[test]
    fn an_exhausted_revision_is_refused_before_anything_is_written() {
        let storage = Storage::empty();
        let store = SourceCatalogStore::at(&storage.path);
        let catalog = one_source();
        let document = serde_json::to_string_pretty(&serde_json::json!({
            "compatibility": {"version": SOURCE_CATALOG_VERSION},
            "revision": u64::MAX,
            "active_source": catalog.active_id(),
            "sources": catalog.sources(),
        }))
        .expect("a synthetic document encodes");
        fs::write(&storage.path, &document).expect("the storage directory is writable");
        let before = storage.document();

        let stored = store
            .load()
            .expect("the document is readable")
            .expect("the document exists");
        assert_eq!(stored.revision().get(), u64::MAX);

        for attempt in 0..2 {
            let error = store
                .save(stored.revision(), stored.catalog())
                .expect_err("an exhausted revision is refused");
            assert!(
                matches!(error, SourceCatalogStoreError::RevisionExhausted { .. }),
                "attempt {attempt} reported {error:?}"
            );
            assert_eq!(storage.document(), before);
        }
        assert_eq!(
            store
                .load()
                .expect("the document is readable")
                .expect("the document exists")
                .revision(),
            stored.revision()
        );
        assert_eq!(storage.entries(), vec![FILE_NAME.to_owned()]);
    }
}
