use std::error::Error;
use std::fs;
use std::path::PathBuf;

use slipbox_core::{SourceChange, SourceDisplayName, SourceRecord};
use slipbox_sources::{
    CatalogRevision, MAX_CATALOG_SOURCES, MalformedCatalog, SourceCatalog, SourceCatalogError,
    SourceCatalogStore, SourceCatalogStoreError, StoredCatalog,
};
use tempfile::{TempDir, tempdir};

mod support;

use support::{fixture_id, private_github_source, public_source};

/// A valid one-source document. Each refusal below replaces exactly one field
/// of it, so the control and the case differ only in the rule under test.
const PUBLIC_DOCUMENT: &str = r#"{
  "compatibility": {
    "version": 1
  },
  "revision": 1,
  "active_source": "01010101010101010101010101010101",
  "sources": [
    {
      "id": "01010101010101010101010101010101",
      "display_name": "Field notes",
      "provider": "generic_https",
      "visibility": "public",
      "remote": "https://git.example.org/owner/notes.git",
      "branch": "main",
      "notes_folder": "notes"
    }
  ]
}
"#;

const PRIVATE_DOCUMENT: &str = r#"{
  "compatibility": {
    "version": 1
  },
  "revision": 1,
  "active_source": "02020202020202020202020202020202",
  "sources": [
    {
      "id": "02020202020202020202020202020202",
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

fn notes_source(seed: u8) -> SourceRecord {
    public_source(
        seed,
        "Field notes",
        "https://git.example.org/owner/notes.git",
        "notes",
    )
}

struct Storage {
    _directory: TempDir,
    path: PathBuf,
}

impl Storage {
    fn holding(document: &str) -> Self {
        let directory = tempdir().expect("a temporary private storage directory");
        let path = directory.path().join("source-catalog.json");
        if !document.is_empty() {
            fs::write(&path, document).expect("a synthetic document is writable");
        }
        Self {
            _directory: directory,
            path,
        }
    }

    fn store(&self) -> SourceCatalogStore {
        SourceCatalogStore::at(&self.path)
    }

    fn document(&self) -> String {
        fs::read_to_string(&self.path).expect("the document is readable")
    }

    fn load(&self) -> Result<Option<StoredCatalog>, SourceCatalogStoreError> {
        self.store().load()
    }
}

fn every_error_form(error: &SourceCatalogStoreError) -> Vec<String> {
    let mut forms = vec![error.to_string(), format!("{error:?}")];
    let mut cause = error.source();
    while let Some(current) = cause {
        forms.push(current.to_string());
        forms.push(format!("{current:?}"));
        cause = current.source();
    }
    forms
}

#[test]
fn the_document_fixtures_carry_the_fixture_identities() {
    assert!(PUBLIC_DOCUMENT.contains(&fixture_id(1).to_string()));
    assert!(PRIVATE_DOCUMENT.contains(&fixture_id(2).to_string()));

    let public = Storage::holding(PUBLIC_DOCUMENT)
        .load()
        .expect("the public control is readable")
        .expect("the public control exists");
    assert_eq!(public.catalog().active_id(), Some(&fixture_id(1)));
    assert_eq!(public.revision().get(), 1);

    let private = Storage::holding(PRIVATE_DOCUMENT)
        .load()
        .expect("the private control is readable")
        .expect("the private control exists");
    let record = private
        .catalog()
        .active()
        .expect("the private control selects its source");
    assert!(record.is_private());
    assert_eq!(
        record.credential().map(|handle| handle.as_str()),
        Some("slipbox.source.vault-handle-1")
    );
    assert!(record.notes_folder().is_root());
}

#[test]
fn one_identity_is_configured_once() {
    let mut catalog = SourceCatalog::new();
    catalog.add(notes_source(1)).expect("the source is new");

    let error = catalog
        .add(public_source(
            1,
            "A second label for one identity",
            "https://git.example.org/other/notes.git",
            "",
        ))
        .expect_err("a duplicate identity is refused");
    assert_eq!(error, SourceCatalogError::DuplicateSource(fixture_id(1)));
    assert_eq!(catalog.len(), 1);
    assert_eq!(
        catalog
            .get(&fixture_id(1))
            .expect("the first record is configured")
            .display_name()
            .as_str(),
        "Field notes",
        "the refused addition replaced the configured record"
    );
}

#[test]
fn a_selection_names_a_configured_source() {
    let mut catalog = SourceCatalog::new();
    let unknown = fixture_id(9);
    assert_eq!(
        catalog
            .set_active(&unknown)
            .expect_err("nothing is configured"),
        SourceCatalogError::UnknownSource(unknown.clone())
    );

    catalog.add(notes_source(1)).expect("the source is new");
    assert_eq!(
        catalog
            .set_active(&unknown)
            .expect_err("the source is absent"),
        SourceCatalogError::UnknownSource(unknown.clone())
    );
    assert_eq!(
        catalog.remove(&unknown).expect_err("the source is absent"),
        SourceCatalogError::UnknownSource(unknown.clone())
    );
    assert_eq!(
        catalog
            .apply(
                &unknown,
                SourceChange::Relabel(SourceDisplayName::parse("A label").expect("a label"))
            )
            .expect_err("the source is absent"),
        SourceCatalogError::UnknownSource(unknown)
    );
    assert_eq!(catalog.active_id(), Some(&fixture_id(1)));

    catalog.clear_active();
    assert_eq!(catalog.active_id(), None);
    assert_eq!(catalog.len(), 1, "clearing a selection removed a source");
}

#[test]
fn assembling_a_catalog_refuses_a_selection_it_does_not_hold() {
    assert_eq!(
        SourceCatalog::from_parts(vec![notes_source(1)], Some(fixture_id(2)))
            .expect_err("the selection is absent"),
        SourceCatalogError::UnknownActiveSource(fixture_id(2))
    );
    assert_eq!(
        SourceCatalog::from_parts(vec![notes_source(1), notes_source(1)], None)
            .expect_err("the identity repeats"),
        SourceCatalogError::DuplicateSource(fixture_id(1))
    );
    assert!(SourceCatalog::from_parts(Vec::new(), None).is_ok());
}

#[test]
fn a_catalog_holds_a_bounded_number_of_sources() {
    let limit = u8::try_from(MAX_CATALOG_SOURCES).expect("the bound fits a fixture seed");
    let sources: Vec<SourceRecord> = (0..=limit).map(notes_source).collect();
    assert_eq!(
        SourceCatalog::from_parts(sources.clone(), None).expect_err("the bound is exceeded"),
        SourceCatalogError::TooManySources {
            limit: MAX_CATALOG_SOURCES
        }
    );

    let mut catalog = SourceCatalog::from_parts(sources[..MAX_CATALOG_SOURCES].to_vec(), None)
        .expect("a full catalog is valid");
    assert_eq!(
        catalog
            .add(sources[MAX_CATALOG_SOURCES].clone())
            .expect_err("a full catalog accepts no more"),
        SourceCatalogError::TooManySources {
            limit: MAX_CATALOG_SOURCES
        }
    );
    assert_eq!(catalog.len(), MAX_CATALOG_SOURCES);
}

#[test]
fn a_stored_record_is_refused_by_the_rules_a_mutation_passes() {
    let cases: &[(&str, &str, &str)] = &[
        (
            "an identity that is not device-local",
            "\"id\": \"01010101010101010101010101010101\"",
            "\"id\": \"not-a-source-identity\"",
        ),
        (
            "a display name that is blank",
            "\"display_name\": \"Field notes\"",
            "\"display_name\": \"   \"",
        ),
        (
            "a provider this build does not carry",
            "\"provider\": \"generic_https\"",
            "\"provider\": \"gitlab\"",
        ),
        (
            "a visibility this build does not carry",
            "\"visibility\": \"public\"",
            "\"visibility\": \"internal\"",
        ),
        (
            "a public record holding an account",
            "\"visibility\": \"public\"",
            "\"visibility\": \"public\",\n      \"account\": \"U_kgDOfixture-account\"",
        ),
        (
            "a remote on an unsupported transport",
            "\"remote\": \"https://git.example.org/owner/notes.git\"",
            "\"remote\": \"git@git.example.org:owner/notes.git\"",
        ),
        (
            "a remote carrying userinfo",
            "\"remote\": \"https://git.example.org/owner/notes.git\"",
            "\"remote\": \"https://reader:s3cret@git.example.org/owner/notes.git\"",
        ),
        (
            "a remote carrying a credential in its query",
            "\"remote\": \"https://git.example.org/owner/notes.git\"",
            "\"remote\": \"https://git.example.org/owner/notes.git?private_token=s3cret\"",
        ),
        (
            "a ref Git refuses",
            "\"branch\": \"main\"",
            "\"branch\": \"release/../main\"",
        ),
        (
            "a folder that leaves the repository",
            "\"notes_folder\": \"notes\"",
            "\"notes_folder\": \"../elsewhere\"",
        ),
        (
            "a folder that is absolute",
            "\"notes_folder\": \"notes\"",
            "\"notes_folder\": \"/etc\"",
        ),
    ];

    for (description, from, to) in cases {
        let document = PUBLIC_DOCUMENT.replace(from, to);
        assert_ne!(
            document, PUBLIC_DOCUMENT,
            "{description} was not built from the control document"
        );
        let storage = Storage::holding(&document);

        let error = storage
            .load()
            .expect_err(&format!("{description} is refused"));
        assert!(
            matches!(error, SourceCatalogStoreError::Malformed { .. }),
            "{description} reported {error}"
        );
        assert!(
            !error.to_string().contains("s3cret"),
            "{description} repeated a refused credential: {error}"
        );
        assert_eq!(
            storage.document(),
            document,
            "{description} changed the document"
        );
    }
}

#[test]
fn a_private_record_without_its_authorization_is_refused() {
    let cases: &[(&str, &str)] = &[
        (
            "a private record without an account",
            ",\n      \"account\": \"U_kgDOfixture-account\"",
        ),
        (
            "a private record without a resolved repository",
            ",\n      \"provider_repository_id\": \"R_kgDOfixture-private\"",
        ),
        (
            "a private record without a credential handle",
            ",\n      \"credential\": \"slipbox.source.vault-handle-1\"",
        ),
    ];

    for (description, field) in cases {
        let document = PRIVATE_DOCUMENT.replace(field, "");
        assert_ne!(
            document, PRIVATE_DOCUMENT,
            "{description} was not built from the control document"
        );
        let error = Storage::holding(&document)
            .load()
            .expect_err(&format!("{description} is refused"));
        assert!(
            matches!(error, SourceCatalogStoreError::Malformed { .. }),
            "{description} reported {error}"
        );
    }

    let document = PRIVATE_DOCUMENT.replace(
        "\"provider\": \"github\"",
        "\"provider\": \"generic_https\"",
    );
    let error = Storage::holding(&document)
        .load()
        .expect_err("a deferred private provider is refused");
    assert!(
        matches!(error, SourceCatalogStoreError::Malformed { .. }),
        "a deferred private provider reported {error}"
    );
}

#[test]
fn a_stored_credential_is_never_a_token() {
    let token = "ghp_0123456789abcdefghijklmnopqrstuvwx";
    let document = PRIVATE_DOCUMENT.replace("slipbox.source.vault-handle-1", token);
    let storage = Storage::holding(&document);

    let error = storage.load().expect_err("a token spelling is refused");
    assert!(
        matches!(error, SourceCatalogStoreError::Malformed { .. }),
        "a token spelling reported {error}"
    );
    for form in [error.to_string(), format!("{error:?}")] {
        assert!(
            !form.contains(token),
            "the refusal repeated the token: {form}"
        );
        assert!(
            !form.contains("ghp_"),
            "the refusal repeated a token prefix: {form}"
        );
    }

    let mut catalog = SourceCatalog::new();
    catalog
        .add(private_github_source(
            2,
            "Private slip-box",
            "https://github.com/owner/slipbox.git",
            "R_kgDOfixture-private",
            "U_kgDOfixture-account",
            "slipbox.source.vault-handle-1",
        ))
        .expect("the source is new");
    let debug = format!(
        "{:?}",
        catalog
            .get(&fixture_id(2))
            .expect("the source is configured")
    );
    assert!(
        debug.contains("slipbox.source.vault-handle-1"),
        "a record's debug form hides the handle a fetch resolves it by: {debug}"
    );
    assert!(
        !debug.contains("ghp_"),
        "a record's debug form carries a token: {debug}"
    );
}

#[test]
fn a_refused_document_is_never_repeated_in_a_diagnostic() {
    // One marker is shaped like a provider token and one is arbitrary, so no
    // refusal can pass by scrubbing a known prefix.
    for marker in [
        "github_pat_SYNTHETIC_NOT_A_CREDENTIAL",
        "zvv-arbitrary-marker-8842",
    ] {
        let quoted = format!("\"{marker}\"");
        let cases: [(&str, String); 8] = [
            (
                "an unknown provider",
                PUBLIC_DOCUMENT.replace("\"generic_https\"", &quoted),
            ),
            (
                "an unknown visibility",
                PUBLIC_DOCUMENT.replace("\"public\"", &quoted),
            ),
            (
                "a version that is not a number",
                PUBLIC_DOCUMENT.replace("\"version\": 1", &format!("\"version\": {quoted}")),
            ),
            (
                "a revision that is not a number",
                PUBLIC_DOCUMENT.replace("\"revision\": 1", &format!("\"revision\": {quoted}")),
            ),
            (
                "a source field of the wrong type",
                PUBLIC_DOCUMENT.replace("\"main\"", &format!("[{quoted}]")),
            ),
            (
                "a missing required field",
                PUBLIC_DOCUMENT
                    .replace("\"Field notes\"", &quoted)
                    .replace("      \"branch\": \"main\",\n", ""),
            ),
            (
                "a document that is not JSON",
                format!("{marker}\n{PUBLIC_DOCUMENT}"),
            ),
            ("a document that ends early", {
                let whole = PUBLIC_DOCUMENT.replace("\"Field notes\"", &quoted);
                whole.split_at(whole.len() - 40).0.to_owned()
            }),
        ];

        for (description, document) in cases {
            assert!(
                document.contains(marker),
                "{description} was not built with the marker in it"
            );
            let storage = Storage::holding(&document);
            let refusals = [
                storage
                    .load()
                    .expect_err(&format!("{description} is refused")),
                storage
                    .store()
                    .save(CatalogRevision::ABSENT, &SourceCatalog::new())
                    .expect_err(&format!("{description} refuses a save")),
            ];
            for refusal in &refusals {
                assert!(
                    matches!(refusal, SourceCatalogStoreError::Malformed { .. }),
                    "{description} reported {refusal:?}"
                );
                for form in every_error_form(refusal) {
                    assert!(
                        !form.contains(marker),
                        "{description} repeated the refused document: {form}"
                    );
                }
            }
            assert_eq!(
                storage.document(),
                document,
                "{description} changed the document"
            );
        }
    }

    let syntax = Storage::holding(&format!("{{ not json\n{PUBLIC_DOCUMENT}"))
        .load()
        .expect_err("invalid JSON is refused");
    assert!(
        matches!(
            syntax,
            SourceCatalogStoreError::Malformed {
                cause: MalformedCatalog::Syntax { line: 1, .. },
                ..
            }
        ),
        "invalid JSON reported {syntax:?}"
    );

    let control = Storage::holding(PUBLIC_DOCUMENT);
    assert_eq!(
        control
            .load()
            .expect("the control is readable")
            .expect("the control exists")
            .revision()
            .get(),
        1
    );
    assert_eq!(control.document(), PUBLIC_DOCUMENT);
}

#[test]
fn a_document_that_is_not_text_is_refused_without_repeating_it() {
    let marker = "github_pat_SYNTHETIC_NOT_A_CREDENTIAL";
    let storage = Storage::holding("");
    let mut bytes = format!("{{\"credential\": \"{marker}\"").into_bytes();
    bytes.push(0xff);
    fs::write(&storage.path, &bytes).expect("a synthetic document is writable");

    let error = storage
        .load()
        .expect_err("bytes that are not text are refused");
    assert!(
        matches!(
            error,
            SourceCatalogStoreError::Malformed {
                cause: MalformedCatalog::NotUtf8,
                ..
            }
        ),
        "a document that is not text reported {error:?}"
    );
    for form in every_error_form(&error) {
        assert!(
            !form.contains(marker),
            "the refusal repeated the bytes: {form}"
        );
    }
    assert_eq!(
        fs::read(&storage.path).expect("the document is readable"),
        bytes
    );
}

#[test]
fn a_stored_record_must_be_reached_through_its_own_provider() {
    for (description, remote) in [
        (
            "an unrelated host",
            "https://unrelated.example.org/private.git",
        ),
        (
            "a host the provider's name is a prefix of",
            "https://github.com.attacker.example.org/owner/slipbox.git",
        ),
        (
            "a host with the provider's name as a suffix",
            "https://notgithub.com/owner/slipbox.git",
        ),
        (
            "a subdomain of the provider",
            "https://api.github.com/owner/slipbox.git",
        ),
        (
            "a non-default port",
            "https://github.com:8443/owner/slipbox.git",
        ),
        (
            "userinfo before the provider's host",
            "https://reader:s3cret@github.com/owner/slipbox.git",
        ),
        (
            "a path that names no owner",
            "https://github.com/slipbox.git",
        ),
        (
            "a path below a repository",
            "https://github.com/owner/slipbox/notes.git",
        ),
    ] {
        let document = PRIVATE_DOCUMENT.replace("https://github.com/owner/slipbox.git", remote);
        assert_ne!(
            document, PRIVATE_DOCUMENT,
            "{description} was not built from the control document"
        );
        let storage = Storage::holding(&document);
        let error = storage
            .load()
            .expect_err(&format!("{description} is refused"));
        assert!(
            matches!(error, SourceCatalogStoreError::Malformed { .. }),
            "{description} reported {error:?}"
        );
        for form in every_error_form(&error) {
            assert!(
                !form.contains("s3cret"),
                "{description} repeated a refused credential: {form}"
            );
        }
        assert_eq!(
            storage.document(),
            document,
            "{description} changed the document"
        );
    }

    assert!(Storage::holding(PRIVATE_DOCUMENT).load().is_ok());
    let public_github = PUBLIC_DOCUMENT
        .replace("\"generic_https\"", "\"github\"")
        .replace(
            "https://git.example.org/owner/notes.git",
            "https://github.com/owner/notes.git",
        );
    assert!(Storage::holding(&public_github).load().is_ok());
    let deep_generic = PUBLIC_DOCUMENT.replace(
        "https://git.example.org/owner/notes.git",
        "https://git.example.org/team/group/notes.git",
    );
    assert!(Storage::holding(&deep_generic).load().is_ok());
}

#[test]
fn a_stored_generic_source_carries_no_provider_repository_identity() {
    let marker = "zvv-marker-8842";
    let document = PUBLIC_DOCUMENT.replace(
        "\"visibility\": \"public\",",
        &format!("\"visibility\": \"public\",\n      \"provider_repository_id\": \"{marker}\","),
    );
    assert_ne!(
        document, PUBLIC_DOCUMENT,
        "the case was not built from the control document"
    );

    let storage = Storage::holding(&document);
    let refused_load = storage
        .load()
        .expect_err("a stored generic record with a provider identity is refused");
    assert!(
        matches!(refused_load, SourceCatalogStoreError::Malformed { .. }),
        "the load reported {refused_load:?}"
    );

    let mut catalog = SourceCatalog::new();
    catalog.add(notes_source(1)).expect("the source is new");
    let refused_save = storage
        .store()
        .save(CatalogRevision::ABSENT, &catalog)
        .expect_err("a write over that document is refused");
    assert!(
        matches!(refused_save, SourceCatalogStoreError::Malformed { .. }),
        "the write reported {refused_save:?}"
    );

    for error in [&refused_load, &refused_save] {
        for form in every_error_form(error) {
            assert!(
                !form.contains(marker),
                "a refusal repeated the identity it rejected: {form}"
            );
        }
    }
    assert_eq!(
        storage.document(),
        document,
        "a refusal changed the document"
    );

    let public_github = PUBLIC_DOCUMENT
        .replace("\"generic_https\"", "\"github\"")
        .replace(
            "https://git.example.org/owner/notes.git",
            "https://github.com/owner/notes.git",
        )
        .replace(
            "\"visibility\": \"public\",",
            "\"visibility\": \"public\",\n      \"provider_repository_id\": \"R_kgDOfixture\",",
        );
    assert!(Storage::holding(&public_github).load().is_ok());
    assert!(Storage::holding(PRIVATE_DOCUMENT).load().is_ok());
    let control = Storage::holding(PUBLIC_DOCUMENT)
        .load()
        .expect("the control document is readable")
        .expect("the control document exists");
    assert!(
        control
            .catalog()
            .active()
            .expect("the control selects its source")
            .provider_repository_id()
            .is_none()
    );
}

#[test]
fn a_stored_branch_names_a_branch_and_not_the_checkout() {
    for reserved in ["HEAD", "@"] {
        let document = PUBLIC_DOCUMENT.replace(
            "\"branch\": \"main\"",
            &format!("\"branch\": \"{reserved}\""),
        );
        let error = Storage::holding(&document)
            .load()
            .expect_err(&format!("{reserved} is refused as a branch"));
        assert!(
            matches!(error, SourceCatalogStoreError::Malformed { .. }),
            "{reserved} reported {error:?}"
        );
    }

    for accepted in [
        "head",
        "Head",
        "HEADS",
        "my/HEAD",
        "FETCH_HEAD",
        "relëase/0.19",
    ] {
        let document = PUBLIC_DOCUMENT.replace(
            "\"branch\": \"main\"",
            &format!("\"branch\": \"{accepted}\""),
        );
        let stored = Storage::holding(&document)
            .load()
            .unwrap_or_else(|error| panic!("{accepted} is a branch name: {error}"))
            .expect("the document exists");
        assert_eq!(
            stored
                .catalog()
                .active()
                .expect("the document selects its source")
                .branch()
                .as_str(),
            accepted,
            "a stored branch name was rewritten"
        );
    }
}

#[test]
fn a_save_states_the_revision_it_read() {
    let storage = Storage::holding("");
    let store = storage.store();
    let mut catalog = SourceCatalog::new();
    catalog.add(notes_source(1)).expect("the source is new");

    let first = store
        .save(CatalogRevision::ABSENT, &catalog)
        .expect("a first write succeeds");
    match store
        .save(CatalogRevision::ABSENT, &catalog)
        .expect_err("a first write over an existing document is refused")
    {
        SourceCatalogStoreError::StaleWrite { expected, found } => {
            assert_eq!(expected, CatalogRevision::ABSENT);
            assert_eq!(found, first);
        }
        other => panic!("a repeated first write reported {other}"),
    }
    assert_eq!(
        store
            .load()
            .expect("the document is readable")
            .expect("the document exists")
            .revision(),
        first
    );
}
