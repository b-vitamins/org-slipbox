// Each integration test crate includes this support module independently and uses
// a different subset of helpers, so shared helpers can look dead per crate.
#![allow(dead_code)]

use slipbox_core::{
    CredentialRef, GitBranch, NotesFolder, ProviderAccountId, ProviderRepositoryId, RemoteUrl,
    SOURCE_ID_ENTROPY_BYTES, SourceConfiguration, SourceDisplayName, SourceId, SourceProvider,
    SourceRecord, SourceVisibility,
};

pub fn fixture_id(seed: u8) -> SourceId {
    SourceId::mint([seed; SOURCE_ID_ENTROPY_BYTES])
}

pub fn public_source(seed: u8, name: &str, remote: &str, notes_folder: &str) -> SourceRecord {
    SourceRecord::new(SourceConfiguration {
        id: fixture_id(seed),
        display_name: SourceDisplayName::parse(name).expect("a fixture label is valid"),
        provider: SourceProvider::GenericHttps,
        visibility: SourceVisibility::Public,
        provider_repository_id: None,
        account: None,
        remote: RemoteUrl::parse(remote).expect("a fixture remote is valid"),
        branch: GitBranch::parse("main").expect("a fixture branch is valid"),
        notes_folder: NotesFolder::parse(notes_folder).expect("a fixture folder is valid"),
        credential: None,
    })
    .expect("a public fixture satisfies the record invariants")
}

pub fn public_github_source(seed: u8, name: &str, remote: &str, repository: &str) -> SourceRecord {
    SourceRecord::new(SourceConfiguration {
        id: fixture_id(seed),
        display_name: SourceDisplayName::parse(name).expect("a fixture label is valid"),
        provider: SourceProvider::GitHub,
        visibility: SourceVisibility::Public,
        provider_repository_id: Some(
            ProviderRepositoryId::parse(repository).expect("a fixture repository id is valid"),
        ),
        account: None,
        remote: RemoteUrl::parse(remote).expect("a fixture remote is valid"),
        branch: GitBranch::parse("main").expect("a fixture branch is valid"),
        notes_folder: NotesFolder::parse("notes").expect("a fixture folder is valid"),
        credential: None,
    })
    .expect("a public provider fixture satisfies the record invariants")
}

pub fn private_github_source(
    seed: u8,
    name: &str,
    remote: &str,
    repository: &str,
    account: &str,
    credential: &str,
) -> SourceRecord {
    SourceRecord::new(SourceConfiguration {
        id: fixture_id(seed),
        display_name: SourceDisplayName::parse(name).expect("a fixture label is valid"),
        provider: SourceProvider::GitHub,
        visibility: SourceVisibility::Private,
        provider_repository_id: Some(
            ProviderRepositoryId::parse(repository).expect("a fixture repository id is valid"),
        ),
        account: Some(ProviderAccountId::parse(account).expect("a fixture account id is valid")),
        remote: RemoteUrl::parse(remote).expect("a fixture remote is valid"),
        branch: GitBranch::parse("main").expect("a fixture branch is valid"),
        notes_folder: NotesFolder::root(),
        credential: Some(CredentialRef::parse(credential).expect("a fixture handle is valid")),
    })
    .expect("a private fixture satisfies the record invariants")
}
