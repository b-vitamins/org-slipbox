//! Bounded HTTPS clone and fetch for repository sources.
//!
//! The transport keeps a bare object store. It does not materialize repository
//! files, run repository programs, consult credential helpers, or expose push.

use std::fmt;
use std::fs;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use gix::credentials::helper::{Action, NextAction};
use gix::credentials::protocol::Outcome as CredentialOutcome;
use gix::remote::{Direction, fetch};
use gix::sec::identity::Account;
use slipbox_core::{GitBranch, RemoteUrl};
use thiserror::Error;
use zeroize::Zeroize;

/// Git history retained by one ordinary refresh. Older objects already in a
/// store remain available; the network request never expands without a bound.
pub const FETCH_DEPTH: u32 = 256;

/// The longest repository path accepted at this boundary.
pub const MAX_REPOSITORY_PATH_BYTES: usize = 4_096;

const REMOTE_NAME: &str = "origin";

type BlockingTransport = Box<dyn gix::protocol::transport::client::blocking_io::Transport + Send>;
type BlockingConnection<'remote, 'auth, 'repo> =
    gix::remote::Connection<'remote, 'auth, 'repo, BlockingTransport>;
type ConnectionConfigurationError = Box<dyn std::error::Error + Send + Sync>;

/// One validated source and its bare object-store destination.
#[derive(Debug, Clone)]
pub struct FetchRequest {
    remote: RemoteUrl,
    branch: GitBranch,
    repository: PathBuf,
}

impl FetchRequest {
    pub fn new(
        remote: RemoteUrl,
        branch: GitBranch,
        repository: PathBuf,
    ) -> Result<Self, GitError> {
        validate_repository_path(&repository)?;
        Ok(Self {
            remote,
            branch,
            repository,
        })
    }

    #[must_use]
    pub fn remote(&self) -> &RemoteUrl {
        &self.remote
    }

    #[must_use]
    pub fn branch(&self) -> &GitBranch {
        &self.branch
    }

    #[must_use]
    pub fn repository(&self) -> &Path {
        &self.repository
    }
}

/// An access token held only for one transport call.
pub struct AccessToken(Vec<u8>);

impl AccessToken {
    pub fn new(mut bytes: Vec<u8>) -> Result<Self, GitError> {
        if bytes.is_empty() || bytes.len() > 4_096 || !bytes.iter().all(u8::is_ascii_graphic) {
            bytes.zeroize();
            return Err(GitError::CredentialRefused);
        }
        Ok(Self(bytes))
    }

    fn copy_string(&self) -> String {
        String::from_utf8(self.0.clone()).expect("an admitted access token is ASCII")
    }
}

impl fmt::Debug for AccessToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("AccessToken(redacted)")
    }
}

impl Drop for AccessToken {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

/// Coarse transport stages. Callers can expose these without handling Git's
/// internal progress vocabulary or credential-adjacent diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressStage {
    Preparing,
    Connecting,
    Receiving,
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GitProgress {
    pub stage: ProgressStage,
    pub objects: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FetchDisposition {
    Cloned,
    Updated,
    Unchanged,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchOutcome {
    pub disposition: FetchDisposition,
    pub revision: String,
    pub received_objects: u64,
}

/// Closed failures: no upstream message, URL, path, header or credential is
/// retained in a value that can reach application diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum GitError {
    #[error("the repository destination is not admitted")]
    DestinationRefused,
    #[error("the credential is not admitted")]
    CredentialRefused,
    #[error("the operation was cancelled")]
    Cancelled,
    #[error("the repository destination could not be prepared")]
    StorageFailed,
    #[error("the repository object store is not usable")]
    RepositoryInvalid,
    #[error("the requested branch is unavailable")]
    BranchUnavailable,
    #[error("the remote transport failed")]
    TransportFailed,
}

/// Clone a new bare object store or fetch the tracked branch into an existing
/// one. The supplied credential is never persisted.
pub fn synchronize(
    request: &FetchRequest,
    credential: Option<AccessToken>,
    cancelled: &AtomicBool,
    progress: impl FnMut(GitProgress),
) -> Result<FetchOutcome, GitError> {
    if credential.is_some()
        && (request.remote().host() != "github.com" || request.remote().port().is_some())
    {
        return Err(GitError::CredentialRefused);
    }
    synchronize_url(
        request,
        request.remote().as_str(),
        false,
        credential,
        cancelled,
        progress,
    )
}

fn synchronize_url(
    request: &FetchRequest,
    remote_url: &str,
    allow_http: bool,
    credential: Option<AccessToken>,
    cancelled: &AtomicBool,
    mut progress: impl FnMut(GitProgress),
) -> Result<FetchOutcome, GitError> {
    check_cancelled(cancelled)?;
    progress(GitProgress {
        stage: ProgressStage::Preparing,
        objects: 0,
    });

    let existing = repository_presence(request.repository())?;
    let parent = request
        .repository()
        .parent()
        .ok_or(GitError::DestinationRefused)?;
    fs::create_dir_all(parent).map_err(|_| GitError::StorageFailed)?;
    check_cancelled(cancelled)?;

    progress(GitProgress {
        stage: ProgressStage::Connecting,
        objects: 0,
    });
    let (repository, received_objects, changed) = if existing {
        fetch_existing(request, remote_url, allow_http, credential, cancelled)?
    } else {
        clone_new(request, remote_url, allow_http, credential, cancelled)?
    };
    check_cancelled(cancelled)?;

    progress(GitProgress {
        stage: ProgressStage::Receiving,
        objects: received_objects,
    });
    let revision = tracked_revision(&repository, request.branch())?;
    let disposition = if !existing {
        FetchDisposition::Cloned
    } else if changed {
        FetchDisposition::Updated
    } else {
        FetchDisposition::Unchanged
    };
    progress(GitProgress {
        stage: ProgressStage::Complete,
        objects: received_objects,
    });
    Ok(FetchOutcome {
        disposition,
        revision,
        received_objects,
    })
}

fn clone_new(
    request: &FetchRequest,
    remote_url: &str,
    allow_http: bool,
    credential: Option<AccessToken>,
    cancelled: &AtomicBool,
) -> Result<(gix::Repository, u64, bool), GitError> {
    let mut clone = gix::clone::PrepareFetch::new(
        remote_url,
        request.repository(),
        gix::create::Kind::Bare,
        gix::create::Options::default(),
        isolated_options(allow_http),
    )
    .map_err(|_| GitError::StorageFailed)?
    .with_remote_name(REMOTE_NAME)
    .map_err(|_| GitError::RepositoryInvalid)?
    .with_ref_name(Some(request.branch().as_str()))
    .map_err(|_| GitError::BranchUnavailable)?
    .with_shallow(fetch::Shallow::DepthAtRemote(fetch_depth()))
    .with_in_memory_config_overrides(transport_config(allow_http))
    .configure_remote(|remote| Ok(remote.with_fetch_tags(fetch::Tags::None)))
    .configure_connection(credentials(credential));

    let (repository, outcome) = clone
        .fetch_only(gix::progress::Discard, cancelled)
        .map_err(|_| cancelled_or_transport(cancelled))?;
    let objects = received_objects(&outcome);
    Ok((repository, objects, true))
}

fn fetch_existing(
    request: &FetchRequest,
    remote_url: &str,
    allow_http: bool,
    credential: Option<AccessToken>,
    cancelled: &AtomicBool,
) -> Result<(gix::Repository, u64, bool), GitError> {
    let repository = gix::open_opts(request.repository(), isolated_options(allow_http))
        .map_err(|_| GitError::RepositoryInvalid)?;
    let before = tracked_revision(&repository, request.branch())?;
    let refspec = tracked_refspec(request.branch());
    let remote = repository
        .remote_at_without_url_rewrite(remote_url)
        .map_err(|_| GitError::RepositoryInvalid)?;
    let remote = remote
        .with_refspecs([refspec.as_str()], Direction::Fetch)
        .map_err(|_| GitError::RepositoryInvalid)?
        .with_fetch_tags(fetch::Tags::None);
    let mut connection = remote
        .connect(Direction::Fetch)
        .map_err(|_| cancelled_or_transport(cancelled))?;
    credentials(credential)(&mut connection).map_err(|_| GitError::TransportFailed)?;
    let prepared = connection
        .prepare_fetch(gix::progress::Discard, Default::default())
        .map_err(|_| cancelled_or_transport(cancelled))?;
    let outcome = prepared
        .with_shallow(fetch::Shallow::DepthAtRemote(fetch_depth()))
        .receive(gix::progress::Discard, cancelled)
        .map_err(|_| cancelled_or_transport(cancelled))?;
    let objects = received_objects(&outcome);
    let changed = tracked_revision(&repository, request.branch())? != before;
    Ok((repository, objects, changed))
}

fn isolated_options(allow_http: bool) -> gix::open::Options {
    gix::open::Options::isolated().config_overrides(transport_config(allow_http))
}

fn transport_config(allow_http: bool) -> Vec<&'static str> {
    vec![
        "http.followRedirects=false",
        "http.lowSpeedLimit=1",
        "http.lowSpeedTime=30",
        "gitoxide.http.connectTimeout=20",
        "protocol.allow=never",
        "protocol.https.allow=always",
        if allow_http {
            "protocol.http.allow=always"
        } else {
            "protocol.http.allow=never"
        },
        "fetch.writeCommitGraph=false",
        "core.logAllRefUpdates=false",
    ]
}

#[cfg(test)]
mod tests;

#[allow(
    clippy::result_large_err,
    reason = "gix fixes the credential callback's result type"
)]
fn credentials(
    credential: Option<AccessToken>,
) -> impl FnMut(&mut BlockingConnection<'_, '_, '_>) -> Result<(), ConnectionConfigurationError> {
    move |connection| {
        let token = credential.as_ref().map(AccessToken::copy_string);
        connection.set_credentials(move |action| match action {
            Action::Get(context) => Ok(token.as_ref().map(|token| CredentialOutcome {
                identity: Account {
                    username: "x-access-token".to_owned(),
                    password: token.clone(),
                    oauth_refresh_token: None,
                },
                next: NextAction::from(context),
            })),
            Action::Store(_) | Action::Erase(_) => Ok(None),
        });
        Ok(())
    }
}

fn tracked_refspec(branch: &GitBranch) -> String {
    format!(
        "+refs/heads/{0}:refs/remotes/{REMOTE_NAME}/{0}",
        branch.as_str()
    )
}

fn tracked_revision(repository: &gix::Repository, branch: &GitBranch) -> Result<String, GitError> {
    let name = format!("refs/remotes/{REMOTE_NAME}/{}", branch.as_str());
    let mut reference = repository
        .find_reference(&name)
        .map_err(|_| GitError::BranchUnavailable)?;
    let id = reference
        .peel_to_id()
        .map_err(|_| GitError::BranchUnavailable)?;
    Ok(id.detach().to_string())
}

fn received_objects(outcome: &fetch::Outcome) -> u64 {
    match &outcome.status {
        fetch::Status::Change {
            write_pack_bundle, ..
        } => u64::from(write_pack_bundle.index.num_objects),
        fetch::Status::NoPackReceived { .. } => 0,
    }
}

fn repository_presence(path: &Path) -> Result<bool, GitError> {
    if !path.exists() {
        return Ok(false);
    }
    if !path.is_dir() {
        return Err(GitError::DestinationRefused);
    }
    let mut entries = fs::read_dir(path).map_err(|_| GitError::StorageFailed)?;
    match entries.next() {
        None => Ok(false),
        Some(Ok(_)) => Ok(true),
        Some(Err(_)) => Err(GitError::StorageFailed),
    }
}

fn validate_repository_path(path: &Path) -> Result<(), GitError> {
    let bytes = path.as_os_str().as_encoded_bytes();
    if !path.is_absolute()
        || bytes.is_empty()
        || bytes.len() > MAX_REPOSITORY_PATH_BYTES
        || path.parent().is_none()
    {
        return Err(GitError::DestinationRefused);
    }
    Ok(())
}

fn fetch_depth() -> NonZeroU32 {
    NonZeroU32::new(FETCH_DEPTH).expect("the declared fetch depth is positive")
}

fn check_cancelled(cancelled: &AtomicBool) -> Result<(), GitError> {
    if cancelled.load(Ordering::Relaxed) {
        Err(GitError::Cancelled)
    } else {
        Ok(())
    }
}

fn cancelled_or_transport(cancelled: &AtomicBool) -> GitError {
    if cancelled.load(Ordering::Relaxed) {
        GitError::Cancelled
    } else {
        GitError::TransportFailed
    }
}
