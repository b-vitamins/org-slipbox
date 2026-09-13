use std::fs;
use std::path::PathBuf;

use slipbox_core::{
    CredentialRef, GitBranch, NotesFolder, ProviderAccountId, ProviderRepositoryId, RemoteUrl,
    RepositoryEvidence, RetainedSourceState, SourceChange, SourceChangeError, SourceDisplayName,
    SourceProvider, SourceScopedKey,
};
use slipbox_sources::{
    CatalogRevision, SOURCE_CATALOG_VERSION, SourceCatalog, SourceCatalogError, SourceCatalogStore,
    SourceCatalogStoreError,
};
use tempfile::{TempDir, tempdir};

mod support;

use support::{fixture_id, private_github_source, public_github_source, public_source};

const PUBLIC_SEED: u8 = 0x0a;
const PRIVATE_SEED: u8 = 0x1b;

const NOTE_KEY: &str = "heading:notes.org:1";

/// The exact document this build writes. A change to it is a format change, not
/// a new golden string.
const GOLDEN_DOCUMENT: &str = r#"{
  "compatibility": {
    "version": 1
  },
  "revision": 1,
  "active_source": "1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b",
  "sources": [
    {
      "id": "0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a",
      "display_name": "Public notes",
      "provider": "generic_https",
      "visibility": "public",
      "remote": "https://git.example.org/owner/notes.git",
      "branch": "main",
      "notes_folder": "notes"
    },
    {
      "id": "1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b",
      "display_name": "Private slip-box",
      "provider": "github",
      "visibility": "private",
      "provider_repository_id": "R_kgDOfixture-private",
      "account": "U_kgDOfixture-account",
      "remote": "https://github.com/owner/slipbox.git",
      "branch": "main",
      "notes_folder": "",
      "credential": "slipbox.source.vault-handle-1"
    }
  ]
}
"#;

struct Storage {
    _directory: TempDir,
    path: PathBuf,
}

impl Storage {
    fn open() -> Self {
        let directory = tempdir().expect("a temporary private storage directory");
        let path = directory.path().join("source-catalog.json");
        Self {
            _directory: directory,
            path,
        }
    }

    fn store(&self) -> SourceCatalogStore {
        SourceCatalogStore::at(&self.path)
    }

    fn document(&self) -> String {
        fs::read_to_string(&self.path).expect("the catalog document is readable")
    }

    fn write_document(&self, contents: &str) {
        fs::write(&self.path, contents).expect("a synthetic document is writable");
    }
}

fn two_source_catalog() -> SourceCatalog {
    let mut catalog = SourceCatalog::new();
    catalog
        .add(public_source(
            PUBLIC_SEED,
            "Public notes",
            "https://git.example.org/owner/notes.git",
            "notes",
        ))
        .expect("the first source is new");
    catalog
        .add(private_github_source(
            PRIVATE_SEED,
            "Private slip-box",
            "https://github.com/owner/slipbox.git",
            "R_kgDOfixture-private",
            "U_kgDOfixture-account",
            "slipbox.source.vault-handle-1",
        ))
        .expect("the second source is new");
    catalog
        .set_active(&fixture_id(PRIVATE_SEED))
        .expect("the second source is configured");
    catalog
}

#[test]
fn a_written_catalog_matches_its_golden_document_and_reloads_unchanged() {
    let storage = Storage::open();
    let store = storage.store();
    let catalog = two_source_catalog();

    let revision = store
        .save(CatalogRevision::ABSENT, &catalog)
        .expect("a first write succeeds");
    assert_eq!(revision.get(), 1);
    assert_eq!(storage.document(), GOLDEN_DOCUMENT);
    assert_eq!(SOURCE_CATALOG_VERSION, 1);

    let stored = store
        .load()
        .expect("the document is readable")
        .expect("the document exists");
    assert_eq!(stored.revision(), revision);
    assert_eq!(stored.catalog(), &catalog);
    assert_eq!(
        stored.catalog().active_id(),
        Some(&fixture_id(PRIVATE_SEED)),
        "the reloaded catalog lost its explicit selection"
    );
    assert_eq!(
        stored
            .catalog()
            .sources()
            .iter()
            .map(|record| record.id().to_string())
            .collect::<Vec<_>>(),
        vec![
            fixture_id(PUBLIC_SEED).to_string(),
            fixture_id(PRIVATE_SEED).to_string()
        ]
    );
}

#[test]
fn a_stored_document_carries_a_credential_handle_and_no_token() {
    let storage = Storage::open();
    storage
        .store()
        .save(CatalogRevision::ABSENT, &two_source_catalog())
        .expect("a first write succeeds");

    let document = storage.document();
    assert!(
        document.contains("slipbox.source.vault-handle-1"),
        "the handle is what a later fetch resolves a credential by"
    );
    for token in ["ghp_", "github_pat_", "password", "@github.com"] {
        assert!(
            !document.contains(token),
            "the document carries {token}: {document}"
        );
    }
}

#[test]
fn an_empty_catalog_is_the_first_state_and_selects_nothing() {
    let storage = Storage::open();
    let store = storage.store();
    assert!(
        store
            .load()
            .expect("an absent document is readable")
            .is_none(),
        "an absent document is the empty state, not an error"
    );

    let revision = store
        .save(CatalogRevision::ABSENT, &SourceCatalog::new())
        .expect("an empty catalog is writable");
    let stored = store
        .load()
        .expect("the document is readable")
        .expect("the document exists");
    assert_eq!(stored.revision(), revision);
    assert!(stored.catalog().is_empty());
    assert_eq!(stored.catalog().active_id(), None);
    assert_eq!(stored.catalog().active(), None);
}

#[test]
fn adding_updating_switching_and_removing_survives_every_reopen() {
    let storage = Storage::open();
    let store = storage.store();

    let mut catalog = SourceCatalog::new();
    catalog
        .add(public_source(
            PUBLIC_SEED,
            "Public notes",
            "https://git.example.org/owner/notes.git",
            "notes",
        ))
        .expect("the first source is new");
    assert_eq!(
        catalog.active_id(),
        Some(&fixture_id(PUBLIC_SEED)),
        "the first source added is the first selection"
    );
    let mut revision = store
        .save(CatalogRevision::ABSENT, &catalog)
        .expect("a first write succeeds");

    catalog
        .add(private_github_source(
            PRIVATE_SEED,
            "Private slip-box",
            "https://github.com/owner/slipbox.git",
            "R_kgDOfixture-private",
            "U_kgDOfixture-account",
            "slipbox.source.vault-handle-1",
        ))
        .expect("the second source is new");
    assert_eq!(
        catalog.active_id(),
        Some(&fixture_id(PUBLIC_SEED)),
        "a later addition displaced the selection"
    );
    revision = store
        .save(revision, &catalog)
        .expect("a second write succeeds");

    let retained = catalog
        .apply(
            &fixture_id(PRIVATE_SEED),
            SourceChange::TrackNotesFolder(
                NotesFolder::parse("slipbox/notes").expect("a valid folder"),
            ),
        )
        .expect("a folder change is accepted");
    assert_eq!(retained, RetainedSourceState::IdentityAndReadingState);
    revision = store
        .save(revision, &catalog)
        .expect("an update is writable");

    let reopened = store
        .load()
        .expect("the document is readable")
        .expect("the document exists")
        .into_catalog();
    assert_eq!(reopened, catalog);
    assert_eq!(
        reopened
            .get(&fixture_id(PRIVATE_SEED))
            .expect("the updated source is configured")
            .notes_folder()
            .as_str(),
        "slipbox/notes"
    );
    assert_eq!(reopened.active_id(), Some(&fixture_id(PUBLIC_SEED)));

    catalog
        .set_active(&fixture_id(PRIVATE_SEED))
        .expect("switching to a configured source is accepted");
    revision = store
        .save(revision, &catalog)
        .expect("a switch is writable");
    assert_eq!(
        store
            .load()
            .expect("the document is readable")
            .expect("the document exists")
            .catalog()
            .active_id(),
        Some(&fixture_id(PRIVATE_SEED))
    );

    let removed = catalog
        .remove(&fixture_id(PRIVATE_SEED))
        .expect("the active source is configured");
    assert_eq!(removed.id(), &fixture_id(PRIVATE_SEED));
    assert_eq!(
        catalog.active_id(),
        None,
        "removing the active source promoted a neighbour by iteration order"
    );
    store
        .save(revision, &catalog)
        .expect("a removal is writable");

    let reopened = store
        .load()
        .expect("the document is readable")
        .expect("the document exists");
    assert_eq!(reopened.revision().get(), 5);
    assert_eq!(reopened.catalog().len(), 1);
    assert_eq!(reopened.catalog().active_id(), None);
    assert_eq!(
        reopened
            .catalog()
            .get(&fixture_id(PUBLIC_SEED))
            .expect("the remaining source kept its identity")
            .branch(),
        &GitBranch::parse("main").expect("a valid branch")
    );
}

#[test]
fn a_configured_identity_is_never_pointed_at_another_repository() {
    let storage = Storage::open();
    let store = storage.store();
    let revision = store
        .save(CatalogRevision::ABSENT, &two_source_catalog())
        .expect("a first write succeeds");
    let stored = store
        .load()
        .expect("the document is readable")
        .expect("the document exists");
    let original = stored.catalog().clone();
    let mut catalog = stored.into_catalog();

    let private = fixture_id(PRIVATE_SEED);
    let repository = ProviderRepositoryId::parse("R_kgDOfixture-private").expect("an identity");
    let account = ProviderAccountId::parse("U_kgDOfixture-account").expect("an account");
    let elsewhere = RemoteUrl::parse("https://github.com/other/slipbox.git").expect("a remote");
    let configured = RemoteUrl::parse("https://github.com/owner/slipbox.git").expect("a remote");

    let refusals = [
        (
            "another repository at the proposed remote",
            SourceChange::Relocate {
                remote: elsewhere.clone(),
                evidence: RepositoryEvidence::authorized(
                    SourceProvider::GitHub,
                    ProviderRepositoryId::parse("R_kgDOanotherRepository").expect("an identity"),
                    elsewhere.clone(),
                    account.clone(),
                ),
            },
            SourceChangeError::DifferentRepository,
        ),
        (
            "evidence obtained for a different remote",
            SourceChange::Relocate {
                remote: elsewhere.clone(),
                evidence: RepositoryEvidence::authorized(
                    SourceProvider::GitHub,
                    repository.clone(),
                    configured.clone(),
                    account.clone(),
                ),
            },
            SourceChangeError::UnobservedRemote,
        ),
        (
            "evidence obtained under another account",
            SourceChange::Relocate {
                remote: elsewhere.clone(),
                evidence: RepositoryEvidence::authorized(
                    SourceProvider::GitHub,
                    repository.clone(),
                    elsewhere.clone(),
                    ProviderAccountId::parse("U_kgDOanotherAccount").expect("an account"),
                ),
            },
            SourceChangeError::DifferentAuthorization,
        ),
        (
            "evidence from another provider",
            SourceChange::Relocate {
                remote: elsewhere.clone(),
                evidence: RepositoryEvidence::public(
                    SourceProvider::GenericHttps,
                    repository.clone(),
                    elsewhere.clone(),
                ),
            },
            SourceChangeError::DifferentProvider,
        ),
    ];

    for (description, change, expected) in refusals {
        let error = catalog
            .apply(&private, change)
            .expect_err(&format!("{description} is refused"));
        assert_eq!(
            error,
            SourceCatalogError::RefusedChange {
                id: private.clone(),
                cause: expected,
            },
            "{description} reported the wrong refusal"
        );
    }

    let public = fixture_id(PUBLIC_SEED);
    let moved = RemoteUrl::parse("https://git.example.org/other/notes.git").expect("a remote");
    assert_eq!(
        catalog
            .apply(
                &public,
                SourceChange::Relocate {
                    remote: moved.clone(),
                    evidence: RepositoryEvidence::public(
                        SourceProvider::GenericHttps,
                        repository.clone(),
                        moved,
                    ),
                }
            )
            .expect_err("an unprovable move is refused"),
        SourceCatalogError::RefusedChange {
            id: public,
            cause: SourceChangeError::UnprovableRepository,
        }
    );

    assert_eq!(catalog, original, "a refused update changed the catalog");
    assert_eq!(
        store
            .save(revision, &catalog)
            .expect("an unchanged catalog is writable")
            .get(),
        revision.get() + 1
    );
    let reopened = store
        .load()
        .expect("the document is readable")
        .expect("the document exists");
    assert_eq!(reopened.catalog(), &original);
    assert_eq!(
        reopened
            .catalog()
            .get(&private)
            .expect("the source is configured")
            .remote(),
        &configured,
        "a refused update reached the stored document"
    );

    let mut catalog = reopened.into_catalog();
    catalog
        .add(public_source(
            0x2c,
            "Another repository",
            "https://git.example.org/other/notes.git",
            "notes",
        ))
        .expect("a new repository is a new source");
    assert_ne!(
        SourceScopedKey::note(&fixture_id(0x2c), NOTE_KEY).expect("a node key is a usable key"),
        SourceScopedKey::note(&fixture_id(PUBLIC_SEED), NOTE_KEY)
            .expect("a node key is a usable key")
    );
}

#[test]
fn a_validated_update_keeps_its_identity_and_says_what_it_retains() {
    let storage = Storage::open();
    let store = storage.store();
    let mut revision = store
        .save(CatalogRevision::ABSENT, &two_source_catalog())
        .expect("a first write succeeds");
    let private = fixture_id(PRIVATE_SEED);
    let minted = SourceScopedKey::note(&private, NOTE_KEY).expect("a node key is a usable key");
    let repository = ProviderRepositoryId::parse("R_kgDOfixture-private").expect("an identity");
    let account = ProviderAccountId::parse("U_kgDOfixture-account").expect("an account");
    let reauthorized = ProviderAccountId::parse("U_kgDOfixture-second").expect("an account");
    let renamed = RemoteUrl::parse("https://github.com/owner/renamed.git").expect("a remote");

    let updates = [
        (
            "a relabel",
            SourceChange::Relabel(
                SourceDisplayName::parse("Renamed on the device").expect("a label"),
            ),
            RetainedSourceState::Everything,
        ),
        (
            "a branch change",
            SourceChange::TrackBranch(GitBranch::parse("release/0.19").expect("a branch")),
            RetainedSourceState::IdentityAndReadingState,
        ),
        (
            "a folder change",
            SourceChange::TrackNotesFolder(NotesFolder::parse("slipbox/notes").expect("a folder")),
            RetainedSourceState::IdentityAndReadingState,
        ),
        (
            "a proven rename",
            SourceChange::Relocate {
                remote: renamed.clone(),
                evidence: RepositoryEvidence::authorized(
                    SourceProvider::GitHub,
                    repository.clone(),
                    renamed.clone(),
                    account.clone(),
                ),
            },
            RetainedSourceState::Everything,
        ),
        (
            "a credential rotation on the same account",
            SourceChange::Reauthorize {
                account: account.clone(),
                credential: CredentialRef::parse("slipbox.source.vault-handle-2")
                    .expect("a handle"),
            },
            RetainedSourceState::Everything,
        ),
        (
            "a reauthorization under another account",
            SourceChange::Reauthorize {
                account: reauthorized.clone(),
                credential: CredentialRef::parse("slipbox.source.vault-handle-3")
                    .expect("a handle"),
            },
            RetainedSourceState::IdentityOnly,
        ),
    ];

    for (description, change, expected) in updates {
        let mut catalog = store
            .load()
            .expect("the document is readable")
            .expect("the document exists")
            .into_catalog();
        let retained = catalog
            .apply(&private, change)
            .unwrap_or_else(|error| panic!("{description} is accepted: {error}"));
        assert_eq!(
            retained, expected,
            "{description} reported the wrong retained state"
        );
        revision = store
            .save(revision, &catalog)
            .unwrap_or_else(|error| panic!("{description} is writable: {error}"));

        let reopened = store
            .load()
            .expect("the document is readable")
            .expect("the document exists");
        assert_eq!(reopened.revision(), revision);
        assert_eq!(
            reopened.catalog(),
            &catalog,
            "{description} did not survive the reload"
        );
        let record = reopened
            .catalog()
            .get(&private)
            .unwrap_or_else(|| panic!("{description} lost the source identity"));
        assert_eq!(
            SourceScopedKey::note(record.id(), NOTE_KEY).expect("a node key is a usable key"),
            minted,
            "{description} re-keyed the source"
        );
        assert_eq!(reopened.catalog().active_id(), Some(&private));
        assert_eq!(reopened.catalog().len(), 2);
    }

    let updated = store
        .load()
        .expect("the document is readable")
        .expect("the document exists")
        .into_catalog();
    let record = updated.get(&private).expect("the source is configured");
    assert_eq!(record.display_name().as_str(), "Renamed on the device");
    assert_eq!(record.branch().as_str(), "release/0.19");
    assert_eq!(record.notes_folder().as_str(), "slipbox/notes");
    assert_eq!(record.remote(), &renamed);
    assert_eq!(record.account(), Some(&reauthorized));
    assert_eq!(
        record.credential().map(CredentialRef::as_str),
        Some("slipbox.source.vault-handle-3")
    );
    let document = storage.document();
    for token in ["ghp_", "github_pat_", "@github.com"] {
        assert!(
            !document.contains(token),
            "the updated document carries {token}: {document}"
        );
    }
}

#[test]
fn an_unrelated_authority_never_inherits_a_configured_generic_identity() {
    let storage = Storage::open();
    let store = storage.store();
    let revision = store
        .save(CatalogRevision::ABSENT, &two_source_catalog())
        .expect("a first write succeeds");
    let committed = storage.document();
    let public = fixture_id(PUBLIC_SEED);
    let private = fixture_id(PRIVATE_SEED);
    let configured = RemoteUrl::parse("https://git.example.org/owner/notes.git").expect("a remote");
    let reading =
        SourceScopedKey::reading_reference(&public, NOTE_KEY).expect("a node key is a usable key");

    for proposed in [
        "https://forge-b.example/team/unrelated.git",
        "https://git.example.org:8443/owner/unrelated.git",
    ] {
        let stored = store
            .load()
            .expect("the document is readable")
            .expect("the document exists");
        let original = stored.catalog().clone();
        let mut catalog = stored.into_catalog();
        let remote = RemoteUrl::parse(proposed).expect("a remote");

        assert_eq!(
            catalog
                .apply(
                    &public,
                    SourceChange::Relocate {
                        remote: remote.clone(),
                        evidence: RepositoryEvidence::public(
                            SourceProvider::GenericHttps,
                            ProviderRepositoryId::parse("17").expect("an identity"),
                            remote,
                        ),
                    }
                )
                .expect_err(&format!("{proposed} is refused")),
            SourceCatalogError::RefusedChange {
                id: public.clone(),
                cause: SourceChangeError::UnprovableRepository,
            },
            "{proposed} reported the wrong refusal"
        );

        assert_eq!(catalog, original, "{proposed} changed the catalog");
        let record = catalog.get(&public).expect("the source is configured");
        assert_eq!(record.remote(), &configured);
        assert_eq!(record.provider_repository_id(), None);
        assert_eq!(
            SourceScopedKey::reading_reference(record.id(), NOTE_KEY)
                .expect("a node key is a usable key"),
            reading,
            "{proposed} re-keyed the source"
        );
        assert_eq!(catalog.sources()[0].id(), &public);
        assert_eq!(catalog.sources()[1].id(), &private);
        assert_eq!(catalog.active_id(), Some(&private));
        assert_eq!(
            storage.document(),
            committed,
            "{proposed} reached the document"
        );

        let reopened = store
            .load()
            .expect("the document is readable")
            .expect("the document exists");
        assert_eq!(reopened.revision(), revision);
        assert_eq!(reopened.catalog(), &original);
    }

    let unrelated = fixture_id(0x3d);
    let mut catalog = store
        .load()
        .expect("the document is readable")
        .expect("the document exists")
        .into_catalog();
    catalog
        .add(public_source(
            0x3d,
            "Unrelated notes",
            "https://forge-b.example/team/unrelated.git",
            "notes",
        ))
        .expect("another repository is another source");
    assert_ne!(unrelated, public);
    assert_ne!(
        SourceScopedKey::reading_reference(&unrelated, NOTE_KEY)
            .expect("a node key is a usable key"),
        reading
    );

    let next = store
        .save(revision, &catalog)
        .expect("the added source is writable");
    let reopened = store
        .load()
        .expect("the document is readable")
        .expect("the document exists");
    assert_eq!(reopened.revision(), next);
    assert_eq!(reopened.catalog().len(), 3);
    let added = reopened
        .catalog()
        .get(&unrelated)
        .expect("the added source is configured");
    assert_eq!(
        added.remote().as_str(),
        "https://forge-b.example/team/unrelated.git"
    );
    assert_eq!(added.provider_repository_id(), None);
    assert_eq!(
        reopened
            .catalog()
            .get(&public)
            .expect("the first source is configured")
            .remote(),
        &configured
    );
    assert_eq!(reopened.catalog().active_id(), Some(&private));
}

#[test]
fn a_generic_source_keeps_its_identity_through_its_ordinary_updates() {
    let storage = Storage::open();
    let store = storage.store();
    let mut revision = store
        .save(CatalogRevision::ABSENT, &two_source_catalog())
        .expect("a first write succeeds");
    let public = fixture_id(PUBLIC_SEED);
    let minted = SourceScopedKey::note(&public, NOTE_KEY).expect("a node key is a usable key");

    let updates = [
        (
            "a relabel",
            SourceChange::Relabel(SourceDisplayName::parse("Team notes").expect("a label")),
            RetainedSourceState::Everything,
        ),
        (
            "a branch change",
            SourceChange::TrackBranch(GitBranch::parse("release/0.19").expect("a branch")),
            RetainedSourceState::IdentityAndReadingState,
        ),
        (
            "a folder change",
            SourceChange::TrackNotesFolder(NotesFolder::parse("slipbox/notes").expect("a folder")),
            RetainedSourceState::IdentityAndReadingState,
        ),
    ];

    for (description, change, expected) in updates {
        let mut catalog = store
            .load()
            .expect("the document is readable")
            .expect("the document exists")
            .into_catalog();
        let retained = catalog
            .apply(&public, change)
            .unwrap_or_else(|error| panic!("{description} is accepted: {error}"));
        assert_eq!(
            retained, expected,
            "{description} reported the wrong retained state"
        );
        revision = store
            .save(revision, &catalog)
            .unwrap_or_else(|error| panic!("{description} is writable: {error}"));

        let reopened = store
            .load()
            .expect("the document is readable")
            .expect("the document exists");
        assert_eq!(reopened.revision(), revision);
        assert_eq!(
            reopened.catalog(),
            &catalog,
            "{description} did not survive the reload"
        );
        let record = reopened
            .catalog()
            .get(&public)
            .unwrap_or_else(|| panic!("{description} lost the source identity"));
        assert_eq!(
            SourceScopedKey::note(record.id(), NOTE_KEY).expect("a node key is a usable key"),
            minted,
            "{description} re-keyed the source"
        );
        assert_eq!(record.provider_repository_id(), None);
        assert_eq!(
            reopened.catalog().active_id(),
            Some(&fixture_id(PRIVATE_SEED))
        );
    }

    let record = store
        .load()
        .expect("the document is readable")
        .expect("the document exists")
        .into_catalog()
        .get(&public)
        .cloned()
        .expect("the source is configured");
    assert_eq!(record.display_name().as_str(), "Team notes");
    assert_eq!(record.branch().as_str(), "release/0.19");
    assert_eq!(record.notes_folder().as_str(), "slipbox/notes");
    assert_eq!(
        record.remote(),
        &RemoteUrl::parse("https://git.example.org/owner/notes.git").expect("a remote")
    );
}

#[test]
fn a_public_provider_rename_is_proven_by_the_repository_it_names() {
    let storage = Storage::open();
    let store = storage.store();
    let public = fixture_id(0x4e);
    let repository = ProviderRepositoryId::parse("R_kgDOfixture-public").expect("an identity");
    let mut catalog = SourceCatalog::new();
    catalog
        .add(public_github_source(
            0x4e,
            "Public slip-box",
            "https://github.com/owner/notes.git",
            "R_kgDOfixture-public",
        ))
        .expect("the source is new");
    let minted = SourceScopedKey::note(&public, NOTE_KEY).expect("a node key is a usable key");
    let revision = store
        .save(CatalogRevision::ABSENT, &catalog)
        .expect("a first write succeeds");
    let elsewhere = RemoteUrl::parse("https://github.com/owner/other.git").expect("a remote");

    assert_eq!(
        catalog
            .apply(
                &public,
                SourceChange::Relocate {
                    remote: elsewhere.clone(),
                    evidence: RepositoryEvidence::public(
                        SourceProvider::GitHub,
                        ProviderRepositoryId::parse("R_kgDOanotherRepository")
                            .expect("an identity"),
                        elsewhere,
                    ),
                }
            )
            .expect_err("another repository at the proposed remote is refused"),
        SourceCatalogError::RefusedChange {
            id: public.clone(),
            cause: SourceChangeError::DifferentRepository,
        }
    );

    let renamed = RemoteUrl::parse("https://github.com/owner/renamed.git").expect("a remote");
    assert_eq!(
        catalog
            .apply(
                &public,
                SourceChange::Relocate {
                    remote: renamed.clone(),
                    evidence: RepositoryEvidence::public(
                        SourceProvider::GitHub,
                        repository.clone(),
                        renamed.clone(),
                    ),
                }
            )
            .expect("an observed rename of the same repository is accepted"),
        RetainedSourceState::Everything
    );
    let committed = store
        .save(revision, &catalog)
        .expect("the renamed source is writable");

    let reopened = store
        .load()
        .expect("the document is readable")
        .expect("the document exists");
    assert_eq!(reopened.revision(), committed);
    let record = reopened
        .catalog()
        .get(&public)
        .expect("the source kept its identity");
    assert_eq!(record.remote(), &renamed);
    assert_eq!(record.provider_repository_id(), Some(&repository));
    assert_eq!(record.account(), None);
    assert_eq!(record.credential(), None);
    assert_eq!(
        SourceScopedKey::note(record.id(), NOTE_KEY).expect("a node key is a usable key"),
        minted,
        "a rename re-keyed the source"
    );
    assert_eq!(reopened.catalog().active_id(), Some(&public));
}

#[test]
fn an_unsupported_version_is_refused_without_touching_the_document() {
    let storage = Storage::open();
    let store = storage.store();
    let document = GOLDEN_DOCUMENT.replace("\"version\": 1", "\"version\": 2");
    storage.write_document(&document);

    match store.load().expect_err("a future version is refused") {
        SourceCatalogStoreError::UnsupportedVersion {
            found, supported, ..
        } => {
            assert_eq!(found, 2);
            assert_eq!(supported, SOURCE_CATALOG_VERSION);
        }
        other => panic!("an unsupported version reported {other}"),
    }

    let error = store
        .save(CatalogRevision::ABSENT, &SourceCatalog::new())
        .expect_err("a write over an unreadable document is refused");
    assert!(
        matches!(error, SourceCatalogStoreError::UnsupportedVersion { .. }),
        "a future version was overwritten from an assumed empty state: {error}"
    );
    assert_eq!(storage.document(), document);
}

#[test]
fn a_malformed_document_is_an_error_rather_than_an_empty_catalog() {
    let duplicated = GOLDEN_DOCUMENT.replace(
        "\"1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b\",\n      \"display_name\": \"Private slip-box\"",
        "\"0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a\",\n      \"display_name\": \"Private slip-box\"",
    );
    assert_ne!(
        duplicated, GOLDEN_DOCUMENT,
        "the duplicate-identity document was not built"
    );

    let cases: &[(&str, &str)] = &[
        ("an empty file", ""),
        ("a truncated document", &GOLDEN_DOCUMENT[..80]),
        ("text that is not JSON", "not a catalog at all\n"),
        (
            "a document without a version",
            &GOLDEN_DOCUMENT.replace("\"compatibility\"", "\"compatibility_of\""),
        ),
        (
            "a document without a revision",
            &GOLDEN_DOCUMENT.replace("\"revision\": 1", "\"revision\": 0"),
        ),
        (
            "a source with a credential it may not hold",
            &GOLDEN_DOCUMENT.replace(
                "\"visibility\": \"public\"",
                "\"visibility\": \"public\",\n      \"credential\": \"slipbox.source.vault-handle-9\"",
            ),
        ),
        (
            "a source with an unparsable remote",
            &GOLDEN_DOCUMENT.replace(
                "https://git.example.org/owner/notes.git",
                "https://reader:token@git.example.org/owner/notes.git",
            ),
        ),
        ("two sources with one identity", &duplicated),
        (
            "an active selection naming no configured source",
            &GOLDEN_DOCUMENT.replace(
                "\"active_source\": \"1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b\"",
                "\"active_source\": \"2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c2c\"",
            ),
        ),
    ];

    for (description, document) in cases {
        let storage = Storage::open();
        let store = storage.store();
        storage.write_document(document);

        let error = store
            .load()
            .expect_err(&format!("{description} is refused"));
        assert!(
            matches!(
                error,
                SourceCatalogStoreError::Malformed { .. } | SourceCatalogStoreError::Invalid { .. }
            ),
            "{description} reported {error}"
        );
        assert!(
            !error.to_string().contains("token"),
            "{description} repeated a refused credential: {error}"
        );
        assert!(
            !format!("{error:?}").contains("token"),
            "{description} repeated a refused credential in its debug form: {error:?}"
        );

        let refused = store
            .save(CatalogRevision::ABSENT, &SourceCatalog::new())
            .expect_err(&format!("a write over {description} is refused"));
        assert!(
            !matches!(refused, SourceCatalogStoreError::StaleWrite { .. }),
            "{description} was treated as a readable revision: {refused}"
        );
        assert_eq!(
            &storage.document(),
            document,
            "{description} was replaced by a catalog derived from an assumed empty state"
        );
    }
}

#[test]
fn a_document_that_is_not_text_is_malformed_rather_than_unreadable() {
    let storage = Storage::open();
    let store = storage.store();
    fs::write(&storage.path, [0xff, 0xfe, 0x00, 0x7b]).expect("a corrupt document is writable");

    let error = store.load().expect_err("a non-text document is refused");
    assert!(
        matches!(error, SourceCatalogStoreError::Malformed { .. }),
        "a non-text document reported {error}"
    );
    let refused = store
        .save(CatalogRevision::ABSENT, &SourceCatalog::new())
        .expect_err("a write over a non-text document is refused");
    assert!(
        matches!(refused, SourceCatalogStoreError::Malformed { .. }),
        "a non-text document was treated as an empty state: {refused}"
    );
    assert_eq!(
        fs::read(&storage.path).expect("the document is readable"),
        [0xff, 0xfe, 0x00, 0x7b]
    );
}

#[test]
fn a_document_over_the_size_limit_is_refused_rather_than_parsed() {
    let storage = Storage::open();
    let store = storage.store();
    let padding = " ".repeat(300 * 1024);
    storage.write_document(&format!("{GOLDEN_DOCUMENT}{padding}"));

    let error = store.load().expect_err("an oversized document is refused");
    match error {
        SourceCatalogStoreError::TooLarge { limit, found, .. } => {
            assert!(
                found > limit,
                "an oversized document reported {found} bytes"
            );
        }
        other => panic!("an oversized document reported {other}"),
    }
}

#[test]
fn a_stale_save_is_refused_and_the_committed_catalog_survives() {
    let storage = Storage::open();
    let owner = storage.store();
    let mut catalog = two_source_catalog();
    let revision = owner
        .save(CatalogRevision::ABSENT, &catalog)
        .expect("a first write succeeds");

    // A second writer commits between the first writer's load and its save.
    let intruder = storage.store();
    let mut committed = catalog.clone();
    committed
        .remove(&fixture_id(PUBLIC_SEED))
        .expect("the source is configured");
    let committed_revision = intruder
        .save(revision, &committed)
        .expect("the second writer holds the current revision");

    catalog
        .set_active(&fixture_id(PUBLIC_SEED))
        .expect("the source is configured");
    match owner
        .save(revision, &catalog)
        .expect_err("a stale write is refused")
    {
        SourceCatalogStoreError::StaleWrite { expected, found } => {
            assert_eq!(expected, revision);
            assert_eq!(found, committed_revision);
        }
        other => panic!("a stale write reported {other}"),
    }
    assert_eq!(
        owner
            .load()
            .expect("the document is readable")
            .expect("the document exists")
            .catalog(),
        &committed,
        "the refused write still replaced the committed catalog"
    );

    let stored = owner
        .load()
        .expect("the document is readable")
        .expect("the document exists");
    let mut merged = stored.catalog().clone();
    merged
        .add(public_source(
            PUBLIC_SEED,
            "Public notes",
            "https://git.example.org/owner/notes.git",
            "notes",
        ))
        .expect("the source is absent after the other writer removed it");
    let revision = owner
        .save(stored.revision(), &merged)
        .expect("a write on the current revision succeeds");
    assert_eq!(revision.get(), committed_revision.get() + 1);
}
