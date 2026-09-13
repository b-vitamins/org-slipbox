//! Identity for a corpus carried by a Git repository.
//!
//! Device-local identities are minted from caller-supplied entropy, not URLs.

use std::error::Error;
use std::fmt;

use serde::{Deserialize, Serialize};

pub const SOURCE_ID_ENTROPY_BYTES: usize = 16;

const SOURCE_ID_CHARS: usize = SOURCE_ID_ENTROPY_BYTES * 2;
const MAX_DISPLAY_NAME_CHARS: usize = 100;
const MAX_PROVIDER_TEXT_CHARS: usize = 128;
const MAX_REMOTE_URL_CHARS: usize = 512;
const MAX_HOST_CHARS: usize = 253;
const MAX_HOST_LABEL_CHARS: usize = 63;
const MAX_BRANCH_CHARS: usize = 255;
const MAX_NOTES_FOLDER_CHARS: usize = 512;
const MAX_SCOPED_KEY_CHARS: usize = 1024;
const DEFAULT_HTTPS_PORT: u16 = 443;
const SCOPED_KEY_SEPARATOR: char = ':';
const GITHUB_AUTHORITY: &str = "github.com";

/// Prefixes a stored GitHub credential is spelled with. A credential reference
/// carrying one is a token rather than a handle for one.
const CREDENTIAL_TOKEN_PREFIXES: &[&str] = &["ghp_", "gho_", "ghu_", "ghs_", "ghr_", "github_pat_"];

/// A device-local source identity: 32 lowercase hexadecimal characters.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SourceId(String);

impl SourceId {
    /// Mint from entropy supplied by the platform's cryptographic random source.
    #[must_use]
    pub fn mint(entropy: [u8; SOURCE_ID_ENTROPY_BYTES]) -> Self {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        let mut value = String::with_capacity(SOURCE_ID_CHARS);
        for byte in entropy {
            value.push(char::from(DIGITS[usize::from(byte >> 4)]));
            value.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
        }
        Self(value)
    }

    pub fn parse(value: &str) -> Result<Self, SourceIdError> {
        let found = value.chars().count();
        if found != SOURCE_ID_CHARS {
            return Err(SourceIdError::Length { found });
        }
        if !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(SourceIdError::Charset);
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SourceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl TryFrom<String> for SourceId {
    type Error = SourceIdError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<SourceId> for String {
    fn from(value: SourceId) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceIdError {
    Length { found: usize },
    Charset,
}

impl fmt::Display for SourceIdError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length { found } => write!(
                formatter,
                "a source identity is {SOURCE_ID_CHARS} characters long, not {found}"
            ),
            Self::Charset => formatter
                .write_str("a source identity carries only lowercase hexadecimal characters"),
        }
    }
}

impl Error for SourceIdError {}

/// The provider a source is reached through. Serialized tokens are the names
/// [`SourceProvider::as_str`] reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceProvider {
    /// Any HTTPS Git host, reached without provider-owned identity.
    GenericHttps,
    #[serde(rename = "github")]
    GitHub,
}

impl SourceProvider {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GenericHttps => "generic_https",
            Self::GitHub => "github",
        }
    }

    /// Whether this provider supports authorized private access.
    #[must_use]
    pub const fn supports_private_access(self) -> bool {
        match self {
            Self::GenericHttps => false,
            Self::GitHub => true,
        }
    }

    /// Authority for provider-owned identities; generic HTTPS has none.
    #[must_use]
    pub const fn authority(self) -> Option<&'static str> {
        match self {
            Self::GenericHttps => None,
            Self::GitHub => Some(GITHUB_AUTHORITY),
        }
    }
}

impl fmt::Display for SourceProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceVisibility {
    Public,
    Private,
}

impl SourceVisibility {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Private => "private",
        }
    }
}

impl fmt::Display for SourceVisibility {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Which identity a source-bound key names. It takes part in equality, so a
/// note key and a file key spelled alike are two different keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceScope {
    Note,
    File,
    Asset,
    ReadingReference,
}

impl SourceScope {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Note => "note",
            Self::File => "file",
            Self::Asset => "asset",
            Self::ReadingReference => "reading_reference",
        }
    }

    fn from_token(value: &str) -> Option<Self> {
        match value {
            "note" => Some(Self::Note),
            "file" => Some(Self::File),
            "asset" => Some(Self::Asset),
            "reading_reference" => Some(Self::ReadingReference),
            _ => None,
        }
    }
}

impl fmt::Display for SourceScope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Why a text field was refused. No variant repeats the offending value, so an
/// error over credential-adjacent input carries nothing to leak.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceTextError {
    field: &'static str,
    reason: SourceTextReason,
}

impl SourceTextError {
    #[must_use]
    pub const fn field(&self) -> &'static str {
        self.field
    }

    #[must_use]
    pub const fn reason(&self) -> SourceTextReason {
        self.reason
    }
}

impl fmt::Display for SourceTextError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let field = self.field;
        match self.reason {
            SourceTextReason::Empty => write!(formatter, "{field} must not be empty"),
            SourceTextReason::TooLong { limit } => {
                write!(formatter, "{field} must be at most {limit} characters")
            }
            SourceTextReason::Charset => {
                write!(formatter, "{field} carries an unsupported character")
            }
            SourceTextReason::CredentialLike => write!(
                formatter,
                "{field} is spelled like a credential; a source stores only a handle for one"
            ),
        }
    }
}

impl Error for SourceTextError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceTextReason {
    Empty,
    TooLong { limit: usize },
    Charset,
    CredentialLike,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextCharset {
    /// Machine-supplied handles: printable ASCII without whitespace.
    AsciiGraphic,
    /// Human- and repository-supplied text: any script, no control character
    /// and no surrounding whitespace.
    PrintableUnicode,
}

fn parse_text(
    field: &'static str,
    value: &str,
    limit: usize,
    charset: TextCharset,
) -> Result<String, SourceTextError> {
    let refuse = |reason| SourceTextError { field, reason };
    if value.is_empty() {
        return Err(refuse(SourceTextReason::Empty));
    }
    if value.chars().count() > limit {
        return Err(refuse(SourceTextReason::TooLong { limit }));
    }
    let admitted = match charset {
        TextCharset::AsciiGraphic => value.chars().all(|character| character.is_ascii_graphic()),
        TextCharset::PrintableUnicode => {
            !value.chars().any(char::is_control)
                && value.trim() == value
                && !value
                    .chars()
                    .any(|character| character.is_whitespace() && character != ' ')
        }
    };
    if !admitted {
        return Err(refuse(SourceTextReason::Charset));
    }
    Ok(value.to_owned())
}

/// The label a source is shown under. It is never authority: two sources may
/// share a display name, and changing one leaves identity untouched.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SourceDisplayName(String);

impl SourceDisplayName {
    pub fn parse(value: &str) -> Result<Self, SourceTextError> {
        parse_text(
            "a source display name",
            value.trim(),
            MAX_DISPLAY_NAME_CHARS,
            TextCharset::PrintableUnicode,
        )
        .map(Self)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The provider's own identity for a repository, where the provider has one.
/// A generic HTTPS host has none, and one is never invented for it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ProviderRepositoryId(String);

impl ProviderRepositoryId {
    pub fn parse(value: &str) -> Result<Self, SourceTextError> {
        parse_text(
            "a provider repository identity",
            value,
            MAX_PROVIDER_TEXT_CHARS,
            TextCharset::AsciiGraphic,
        )
        .map(Self)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The account whose authorization a private source reads through.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ProviderAccountId(String);

impl ProviderAccountId {
    pub fn parse(value: &str) -> Result<Self, SourceTextError> {
        parse_text(
            "a provider account identity",
            value,
            MAX_PROVIDER_TEXT_CHARS,
            TextCharset::AsciiGraphic,
        )
        .map(Self)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// An opaque handle for a credential the platform vault holds. The credential
/// itself never reaches a source record; a value spelled like a GitHub token is
/// refused, which catches that spelling rather than every possible secret.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CredentialRef(String);

impl CredentialRef {
    pub fn parse(value: &str) -> Result<Self, SourceTextError> {
        let field = "a credential reference";
        let handle = parse_text(
            field,
            value,
            MAX_PROVIDER_TEXT_CHARS,
            TextCharset::AsciiGraphic,
        )?;
        let lowered = handle.to_ascii_lowercase();
        if CREDENTIAL_TOKEN_PREFIXES
            .iter()
            .any(|prefix| lowered.starts_with(prefix))
        {
            return Err(SourceTextError {
                field,
                reason: SourceTextReason::CredentialLike,
            });
        }
        Ok(Self(handle))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

macro_rules! validated_string_newtype {
    ($($type:ty : $error:ty),+ $(,)?) => {
        $(
            impl fmt::Display for $type {
                fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                    formatter.write_str(&self.0)
                }
            }

            impl TryFrom<String> for $type {
                type Error = $error;

                fn try_from(value: String) -> Result<Self, Self::Error> {
                    Self::parse(&value)
                }
            }

            impl From<$type> for String {
                fn from(value: $type) -> Self {
                    value.0
                }
            }
        )+
    };
}

validated_string_newtype!(
    SourceDisplayName: SourceTextError,
    ProviderRepositoryId: SourceTextError,
    ProviderAccountId: SourceTextError,
    CredentialRef: SourceTextError,
    RemoteUrl: RemoteUrlError,
    GitBranch: GitBranchError,
    NotesFolder: NotesFolderError,
);

/// A credential-free HTTPS fetch URL in normalized form.
///
/// Normalization lowercases the scheme and host and drops an explicit default
/// port. It leaves the repository path exactly as given, including case and any
/// `.git` suffix, so two spellings are never merged into one repository.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RemoteUrl(String);

impl RemoteUrl {
    pub fn parse(value: &str) -> Result<Self, RemoteUrlError> {
        let value = value.trim();
        if value.chars().count() > MAX_REMOTE_URL_CHARS {
            return Err(RemoteUrlError::TooLong {
                limit: MAX_REMOTE_URL_CHARS,
            });
        }
        let Some((scheme, rest)) = value.split_once("://") else {
            return Err(RemoteUrlError::UnsupportedScheme);
        };
        if !scheme.eq_ignore_ascii_case("https") {
            return Err(RemoteUrlError::UnsupportedScheme);
        }
        if rest.contains('?') || rest.contains('#') {
            return Err(RemoteUrlError::QueryOrFragment);
        }
        let Some((authority, path)) = rest.split_once('/') else {
            return Err(RemoteUrlError::MissingPath);
        };
        if authority.contains('@') {
            return Err(RemoteUrlError::CredentialInAuthority);
        }

        let (host, port) = parse_authority(authority)?;
        let path = normalize_remote_path(path)?;
        let port = match port {
            Some(DEFAULT_HTTPS_PORT) | None => String::new(),
            Some(port) => format!(":{port}"),
        };
        Ok(Self(format!("https://{host}{port}{path}")))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The lowercase host, without any port.
    #[must_use]
    pub fn host(&self) -> &str {
        let (authority, _) = self.parts();
        authority
            .split_once(':')
            .map_or(authority, |(host, _)| host)
    }

    /// The port the URL states, which normalization keeps only when it is not
    /// the default HTTPS port.
    #[must_use]
    pub fn port(&self) -> Option<u16> {
        let (authority, _) = self.parts();
        authority
            .split_once(':')
            .and_then(|(_, port)| port.parse().ok())
    }

    /// The repository path, with its leading slash and its original case.
    #[must_use]
    pub fn path(&self) -> &str {
        let (_, path) = self.parts();
        path
    }

    fn parts(&self) -> (&str, &str) {
        let rest = self.0.strip_prefix("https://").unwrap_or_default();
        match rest.find('/') {
            Some(boundary) => (&rest[..boundary], &rest[boundary..]),
            None => (rest, ""),
        }
    }
}

/// Bind provider-owned identities to the provider's remote authority.
fn validate_provider_remote(
    provider: SourceProvider,
    remote: &RemoteUrl,
) -> Result<(), SourceRecordError> {
    let Some(authority) = provider.authority() else {
        return Ok(());
    };
    if remote.host() != authority || remote.port().is_some() {
        return Err(SourceRecordError::UnsupportedProviderAuthority(provider));
    }
    let path = remote.path().strip_prefix('/').unwrap_or_default();
    let mut segments = path.split('/');
    let owner = segments.next().unwrap_or_default();
    let repository = segments.next().unwrap_or_default();
    let repository = repository.strip_suffix(".git").unwrap_or(repository);
    if segments.next().is_some() || owner.is_empty() || repository.is_empty() {
        return Err(SourceRecordError::UnsupportedProviderRepositoryPath(
            provider,
        ));
    }
    Ok(())
}

fn parse_authority(authority: &str) -> Result<(String, Option<u16>), RemoteUrlError> {
    // A bracketed IP-literal host is unsupported, and its brackets would
    // otherwise be read as a port separator.
    if authority.starts_with('[') || authority.contains(']') {
        return Err(RemoteUrlError::InvalidHost);
    }
    let (host, port) = match authority.split_once(':') {
        Some((host, port)) => (host, Some(port)),
        None => (authority, None),
    };
    let port = match port {
        None => None,
        Some(port) => Some(parse_port(port)?),
    };

    if host.is_empty() {
        return Err(RemoteUrlError::MissingHost);
    }
    let host = host.to_ascii_lowercase();
    if host.chars().count() > MAX_HOST_CHARS {
        return Err(RemoteUrlError::InvalidHost);
    }
    let admitted = host.split('.').all(|label| {
        !label.is_empty()
            && label.chars().count() <= MAX_HOST_LABEL_CHARS
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    });
    if !admitted {
        return Err(RemoteUrlError::InvalidHost);
    }
    Ok((host, port))
}

fn parse_port(port: &str) -> Result<u16, RemoteUrlError> {
    if port.is_empty() || port.contains(':') || (port.len() > 1 && port.starts_with('0')) {
        return Err(RemoteUrlError::InvalidPort);
    }
    match port.parse::<u16>() {
        Ok(0) | Err(_) => Err(RemoteUrlError::InvalidPort),
        Ok(port) => Ok(port),
    }
}

fn normalize_remote_path(path: &str) -> Result<String, RemoteUrlError> {
    let path = path.strip_suffix('/').unwrap_or(path);
    if path.is_empty() {
        return Err(RemoteUrlError::MissingPath);
    }
    let admitted = path.split('/').all(|segment| {
        !segment.is_empty()
            && segment != "."
            && segment != ".."
            && segment.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~')
            })
    });
    if !admitted {
        return Err(RemoteUrlError::InvalidPath);
    }
    Ok(format!("/{path}"))
}

/// Why a remote URL was refused. No variant carries the offending URL, so a
/// refused credential-bearing URL cannot reach a log through this error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteUrlError {
    UnsupportedScheme,
    CredentialInAuthority,
    QueryOrFragment,
    MissingHost,
    InvalidHost,
    InvalidPort,
    MissingPath,
    InvalidPath,
    TooLong { limit: usize },
}

impl fmt::Display for RemoteUrlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::UnsupportedScheme => {
                "a remote must be an https:// URL; no other transport is supported"
            }
            Self::CredentialInAuthority => "a remote URL must not carry userinfo",
            Self::QueryOrFragment => "a remote URL must not carry query or fragment data",
            Self::MissingHost => "a remote URL must name a host",
            Self::InvalidHost => "a remote URL host is not a supported domain name",
            Self::InvalidPort => "a remote URL port is not a supported port number",
            Self::MissingPath => "a remote URL must name a repository path",
            Self::InvalidPath => "a remote URL repository path carries an unsupported segment",
            Self::TooLong { limit } => {
                return write!(formatter, "a remote URL must be at most {limit} characters");
            }
        };
        formatter.write_str(message)
    }
}

impl Error for RemoteUrlError {}

/// The one branch a source follows, as `git check-ref-format` accepts it for a
/// branch name. Branch names are case-sensitive and are never folded.
///
/// `HEAD` and `@` are refused: Git reads them as the current checkout rather
/// than as a branch. Other pseudo-ref spellings, such as `FETCH_HEAD`, are
/// legal branch names and are accepted as Git accepts them.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct GitBranch(String);

impl GitBranch {
    pub fn parse(value: &str) -> Result<Self, GitBranchError> {
        let value = value.trim();
        if value.is_empty() {
            return Err(GitBranchError::Empty);
        }
        if value.chars().count() > MAX_BRANCH_CHARS {
            return Err(GitBranchError::TooLong {
                limit: MAX_BRANCH_CHARS,
            });
        }
        if value.chars().any(|character| {
            character.is_control()
                || matches!(
                    character,
                    ' ' | '~' | '^' | ':' | '?' | '*' | '[' | '\\' | '\u{7f}'
                )
        }) {
            return Err(GitBranchError::InvalidCharacter);
        }
        if value == "HEAD" || value == "@" {
            return Err(GitBranchError::ReservedName);
        }
        if value.contains("..") || value.contains("@{") {
            return Err(GitBranchError::ReservedSequence);
        }
        if value.starts_with('-')
            || value.starts_with('/')
            || value.ends_with('/')
            || value.ends_with('.')
            || value.contains("//")
        {
            return Err(GitBranchError::InvalidComponent);
        }
        if value.split('/').any(|component| {
            component.is_empty() || component.starts_with('.') || component.ends_with(".lock")
        }) {
            return Err(GitBranchError::InvalidComponent);
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitBranchError {
    Empty,
    TooLong { limit: usize },
    InvalidCharacter,
    InvalidComponent,
    ReservedSequence,
    ReservedName,
}

impl fmt::Display for GitBranchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::Empty => "a tracked branch must not be empty",
            Self::InvalidCharacter => "a tracked branch carries a character Git refuses in a ref",
            Self::InvalidComponent => "a tracked branch has a component Git refuses in a ref",
            Self::ReservedSequence => "a tracked branch carries a sequence Git reserves",
            Self::ReservedName => {
                "a tracked branch must name a branch, not the checkout Git reserves the name for"
            }
            Self::TooLong { limit } => {
                return write!(
                    formatter,
                    "a tracked branch must be at most {limit} characters"
                );
            }
        };
        formatter.write_str(message)
    }
}

impl Error for GitBranchError {}

/// The repository-relative folder a slip-box lives in, with the empty path
/// meaning the repository root.
///
/// The grammar refuses traversal, absolute and escaping paths here rather than
/// leaving it to a later importer. Whether a path inside the folder resolves
/// through a symlink out of it is a read-time check this type cannot make.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct NotesFolder(String);

impl NotesFolder {
    #[must_use]
    pub fn root() -> Self {
        Self(String::new())
    }

    pub fn parse(value: &str) -> Result<Self, NotesFolderError> {
        let value = value.trim();
        if value.is_empty() || value == "." {
            return Ok(Self::root());
        }
        if value.chars().count() > MAX_NOTES_FOLDER_CHARS {
            return Err(NotesFolderError::TooLong {
                limit: MAX_NOTES_FOLDER_CHARS,
            });
        }
        if value.starts_with('/') {
            return Err(NotesFolderError::Absolute);
        }
        if value
            .chars()
            .any(|character| character.is_control() || matches!(character, '\\' | ':'))
        {
            return Err(NotesFolderError::InvalidCharacter);
        }
        let value = value.strip_suffix('/').unwrap_or(value);
        for segment in value.split('/') {
            if segment.is_empty() {
                return Err(NotesFolderError::EmptySegment);
            }
            if segment == ".." {
                return Err(NotesFolderError::Traversal);
            }
            if segment == "." || segment == ".git" {
                return Err(NotesFolderError::ReservedSegment);
            }
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn is_root(&self) -> bool {
        self.0.is_empty()
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotesFolderError {
    Absolute,
    Traversal,
    EmptySegment,
    ReservedSegment,
    InvalidCharacter,
    TooLong { limit: usize },
}

impl fmt::Display for NotesFolderError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::Absolute => "a notes folder must be relative to the repository",
            Self::Traversal => "a notes folder must not leave the repository",
            Self::EmptySegment => "a notes folder must not have an empty path segment",
            Self::ReservedSegment => "a notes folder must not name a reserved path segment",
            Self::InvalidCharacter => "a notes folder carries an unsupported character",
            Self::TooLong { limit } => {
                return write!(
                    formatter,
                    "a notes folder must be at most {limit} characters"
                );
            }
        };
        formatter.write_str(message)
    }
}

impl Error for NotesFolderError {}

/// A key scoped to the source it belongs to.
///
/// The inner key is whatever the engine already reports for a note, file or
/// asset; it is wrapped rather than re-derived. Two sources holding one Org ID
/// or one file path therefore produce two distinct keys.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "SourceScopedKeyWire", into = "SourceScopedKeyWire")]
pub struct SourceScopedKey {
    source: SourceId,
    scope: SourceScope,
    key: String,
}

impl SourceScopedKey {
    pub fn new(
        source: SourceId,
        scope: SourceScope,
        key: &str,
    ) -> Result<Self, SourceScopedKeyError> {
        let key = parse_text(
            "a source-scoped key",
            key,
            MAX_SCOPED_KEY_CHARS,
            TextCharset::PrintableUnicode,
        )
        .map_err(SourceScopedKeyError::Key)?;
        Ok(Self { source, scope, key })
    }

    pub fn note(source: &SourceId, node_key: &str) -> Result<Self, SourceScopedKeyError> {
        Self::new(source.clone(), SourceScope::Note, node_key)
    }

    pub fn file(source: &SourceId, file_path: &str) -> Result<Self, SourceScopedKeyError> {
        Self::new(source.clone(), SourceScope::File, file_path)
    }

    pub fn asset(source: &SourceId, asset_path: &str) -> Result<Self, SourceScopedKeyError> {
        Self::new(source.clone(), SourceScope::Asset, asset_path)
    }

    /// A key device-owned reading state refers to a note by. It holds no note
    /// content, so it survives a rebuild or a re-clone of its source.
    pub fn reading_reference(
        source: &SourceId,
        node_key: &str,
    ) -> Result<Self, SourceScopedKeyError> {
        Self::new(source.clone(), SourceScope::ReadingReference, node_key)
    }

    #[must_use]
    pub fn source(&self) -> &SourceId {
        &self.source
    }

    #[must_use]
    pub fn scope(&self) -> SourceScope {
        self.scope
    }

    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// The single string a durable row is keyed by. The identity and scope
    /// fields cannot contain the separator, so the encoding stays unambiguous
    /// for a key that does.
    #[must_use]
    pub fn storage_key(&self) -> String {
        format!(
            "{}{SCOPED_KEY_SEPARATOR}{}{SCOPED_KEY_SEPARATOR}{}",
            self.source, self.scope, self.key
        )
    }

    pub fn parse_storage_key(value: &str) -> Result<Self, SourceScopedKeyError> {
        let Some((source, rest)) = value.split_once(SCOPED_KEY_SEPARATOR) else {
            return Err(SourceScopedKeyError::Malformed);
        };
        let Some((scope, key)) = rest.split_once(SCOPED_KEY_SEPARATOR) else {
            return Err(SourceScopedKeyError::Malformed);
        };
        let source = SourceId::parse(source).map_err(SourceScopedKeyError::Source)?;
        let Some(scope) = SourceScope::from_token(scope) else {
            return Err(SourceScopedKeyError::UnknownScope);
        };
        Self::new(source, scope, key)
    }
}

impl fmt::Display for SourceScopedKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.storage_key())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SourceScopedKeyWire {
    source: SourceId,
    scope: SourceScope,
    key: String,
}

impl TryFrom<SourceScopedKeyWire> for SourceScopedKey {
    type Error = SourceScopedKeyError;

    fn try_from(value: SourceScopedKeyWire) -> Result<Self, Self::Error> {
        Self::new(value.source, value.scope, &value.key)
    }
}

impl From<SourceScopedKey> for SourceScopedKeyWire {
    fn from(value: SourceScopedKey) -> Self {
        Self {
            source: value.source,
            scope: value.scope,
            key: value.key,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceScopedKeyError {
    Key(SourceTextError),
    Source(SourceIdError),
    Malformed,
    UnknownScope,
}

impl fmt::Display for SourceScopedKeyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Key(error) => error.fmt(formatter),
            Self::Source(error) => error.fmt(formatter),
            Self::Malformed => {
                formatter.write_str("a source-scoped storage key is <source>:<scope>:<key>")
            }
            Self::UnknownScope => {
                formatter.write_str("a source-scoped storage key names an unknown scope")
            }
        }
    }
}

impl Error for SourceScopedKeyError {}

/// A source configuration a caller proposes or a stored document carries.
///
/// [`SourceRecord::new`] is the only way to accept one, so a mutation and a
/// deserialized document pass through the same validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceConfiguration {
    pub id: SourceId,
    pub display_name: SourceDisplayName,
    pub provider: SourceProvider,
    pub visibility: SourceVisibility,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_repository_id: Option<ProviderRepositoryId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<ProviderAccountId>,
    pub remote: RemoteUrl,
    pub branch: GitBranch,
    pub notes_folder: NotesFolder,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential: Option<CredentialRef>,
}

/// A validated source record.
///
/// Private sources require provider repository/account IDs and a credential
/// handle. Public sources have no account or credential; generic HTTPS has no
/// provider repository ID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "SourceConfiguration", into = "SourceConfiguration")]
pub struct SourceRecord {
    id: SourceId,
    display_name: SourceDisplayName,
    provider: SourceProvider,
    visibility: SourceVisibility,
    provider_repository_id: Option<ProviderRepositoryId>,
    account: Option<ProviderAccountId>,
    remote: RemoteUrl,
    branch: GitBranch,
    notes_folder: NotesFolder,
    credential: Option<CredentialRef>,
}

impl SourceRecord {
    pub fn new(configuration: SourceConfiguration) -> Result<Self, SourceRecordError> {
        validate_provider_remote(configuration.provider, &configuration.remote)?;
        match configuration.visibility {
            SourceVisibility::Private => {
                if !configuration.provider.supports_private_access() {
                    return Err(SourceRecordError::UnsupportedPrivateProvider(
                        configuration.provider,
                    ));
                }
                if configuration.provider_repository_id.is_none() {
                    return Err(SourceRecordError::MissingRepositoryIdentity);
                }
                if configuration.account.is_none() {
                    return Err(SourceRecordError::MissingAccountIdentity);
                }
                if configuration.credential.is_none() {
                    return Err(SourceRecordError::MissingCredentialReference);
                }
            }
            SourceVisibility::Public => {
                if configuration.account.is_some() {
                    return Err(SourceRecordError::UnexpectedAccountIdentity);
                }
                if configuration.credential.is_some() {
                    return Err(SourceRecordError::UnexpectedCredentialReference);
                }
            }
        }
        if configuration.provider.authority().is_none()
            && configuration.provider_repository_id.is_some()
        {
            return Err(SourceRecordError::UnsupportedProviderRepositoryId(
                configuration.provider,
            ));
        }
        Ok(Self {
            id: configuration.id,
            display_name: configuration.display_name,
            provider: configuration.provider,
            visibility: configuration.visibility,
            provider_repository_id: configuration.provider_repository_id,
            account: configuration.account,
            remote: configuration.remote,
            branch: configuration.branch,
            notes_folder: configuration.notes_folder,
            credential: configuration.credential,
        })
    }

    #[must_use]
    pub fn id(&self) -> &SourceId {
        &self.id
    }

    #[must_use]
    pub fn display_name(&self) -> &SourceDisplayName {
        &self.display_name
    }

    #[must_use]
    pub fn provider(&self) -> SourceProvider {
        self.provider
    }

    #[must_use]
    pub fn visibility(&self) -> SourceVisibility {
        self.visibility
    }

    #[must_use]
    pub fn is_private(&self) -> bool {
        matches!(self.visibility, SourceVisibility::Private)
    }

    #[must_use]
    pub fn provider_repository_id(&self) -> Option<&ProviderRepositoryId> {
        self.provider_repository_id.as_ref()
    }

    #[must_use]
    pub fn account(&self) -> Option<&ProviderAccountId> {
        self.account.as_ref()
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
    pub fn notes_folder(&self) -> &NotesFolder {
        &self.notes_folder
    }

    #[must_use]
    pub fn credential(&self) -> Option<&CredentialRef> {
        self.credential.as_ref()
    }

    /// Apply a validated change and report which source state may be retained.
    pub fn apply(&self, change: SourceChange) -> Result<SourceTransition, SourceChangeError> {
        let mut record = self.clone();
        let retained = match change {
            SourceChange::Relabel(display_name) => {
                record.display_name = display_name;
                RetainedSourceState::Everything
            }
            SourceChange::TrackBranch(branch) => {
                let unchanged = branch == record.branch;
                record.branch = branch;
                if unchanged {
                    RetainedSourceState::Everything
                } else {
                    RetainedSourceState::IdentityAndReadingState
                }
            }
            SourceChange::TrackNotesFolder(notes_folder) => {
                let unchanged = notes_folder == record.notes_folder;
                record.notes_folder = notes_folder;
                if unchanged {
                    RetainedSourceState::Everything
                } else {
                    RetainedSourceState::IdentityAndReadingState
                }
            }
            SourceChange::Relocate { remote, evidence } => {
                let Some(repository) = record.provider_repository_id.as_ref() else {
                    return Err(SourceChangeError::UnprovableRepository);
                };
                if evidence.provider() != record.provider {
                    return Err(SourceChangeError::DifferentProvider);
                }
                if evidence.repository() != repository {
                    return Err(SourceChangeError::DifferentRepository);
                }
                if evidence.remote() != &remote {
                    return Err(SourceChangeError::UnobservedRemote);
                }
                if evidence.account() != record.account.as_ref() {
                    return Err(SourceChangeError::DifferentAuthorization);
                }
                validate_provider_remote(record.provider, &remote)
                    .map_err(SourceChangeError::UnsupportedRemote)?;
                record.remote = remote;
                RetainedSourceState::Everything
            }
            SourceChange::Reauthorize {
                account,
                credential,
            } => {
                if !record.is_private() {
                    return Err(SourceChangeError::NotAuthorized);
                }
                let same_account = record.account.as_ref() == Some(&account);
                record.account = Some(account);
                record.credential = Some(credential);
                if same_account {
                    RetainedSourceState::Everything
                } else {
                    RetainedSourceState::IdentityOnly
                }
            }
        };
        Ok(SourceTransition { record, retained })
    }
}

impl TryFrom<SourceConfiguration> for SourceRecord {
    type Error = SourceRecordError;

    fn try_from(value: SourceConfiguration) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<SourceRecord> for SourceConfiguration {
    fn from(value: SourceRecord) -> Self {
        Self {
            id: value.id,
            display_name: value.display_name,
            provider: value.provider,
            visibility: value.visibility,
            provider_repository_id: value.provider_repository_id,
            account: value.account,
            remote: value.remote,
            branch: value.branch,
            notes_folder: value.notes_folder,
            credential: value.credential,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceRecordError {
    UnsupportedPrivateProvider(SourceProvider),
    UnsupportedProviderAuthority(SourceProvider),
    UnsupportedProviderRepositoryPath(SourceProvider),
    UnsupportedProviderRepositoryId(SourceProvider),
    MissingRepositoryIdentity,
    MissingAccountIdentity,
    MissingCredentialReference,
    UnexpectedAccountIdentity,
    UnexpectedCredentialReference,
}

impl fmt::Display for SourceRecordError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::UnsupportedPrivateProvider(provider) => {
                return write!(
                    formatter,
                    "private access to {provider} is not supported; only a public repository can be read through it"
                );
            }
            Self::UnsupportedProviderAuthority(provider) => {
                return write!(
                    formatter,
                    "a {provider} source must be reached through that provider's own host on the default HTTPS port"
                );
            }
            Self::UnsupportedProviderRepositoryPath(provider) => {
                return write!(
                    formatter,
                    "a {provider} remote must name one owner and one repository"
                );
            }
            Self::UnsupportedProviderRepositoryId(provider) => {
                return write!(
                    formatter,
                    "{provider} issues no repository identity, so a source reached through it carries none"
                );
            }
            Self::MissingRepositoryIdentity => {
                "a private source needs the provider's resolved repository identity"
            }
            Self::MissingAccountIdentity => "a private source needs its authorizing account",
            Self::MissingCredentialReference => {
                "a private source needs a credential reference to fetch with"
            }
            Self::UnexpectedAccountIdentity => "a public source has no authorizing account",
            Self::UnexpectedCredentialReference => "a public source holds no credential reference",
        };
        formatter.write_str(message)
    }
}

impl Error for SourceRecordError {}

/// What a provider reports for the repository one remote URL names.
///
/// Bound to the observed remote and authorizing account. The sync layer supplies
/// the lookup result; this layer trusts the caller and cannot verify the lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryEvidence {
    provider: SourceProvider,
    repository: ProviderRepositoryId,
    remote: RemoteUrl,
    account: Option<ProviderAccountId>,
}

impl RepositoryEvidence {
    /// An observation of a public repository, made under no authorization.
    #[must_use]
    pub fn public(
        provider: SourceProvider,
        repository: ProviderRepositoryId,
        remote: RemoteUrl,
    ) -> Self {
        Self {
            provider,
            repository,
            remote,
            account: None,
        }
    }

    /// An observation made under one account's authorization, which is the only
    /// authorization it is evidence for.
    #[must_use]
    pub fn authorized(
        provider: SourceProvider,
        repository: ProviderRepositoryId,
        remote: RemoteUrl,
        account: ProviderAccountId,
    ) -> Self {
        Self {
            provider,
            repository,
            remote,
            account: Some(account),
        }
    }

    #[must_use]
    pub const fn provider(&self) -> SourceProvider {
        self.provider
    }

    #[must_use]
    pub const fn repository(&self) -> &ProviderRepositoryId {
        &self.repository
    }

    #[must_use]
    pub const fn remote(&self) -> &RemoteUrl {
        &self.remote
    }

    #[must_use]
    pub const fn account(&self) -> Option<&ProviderAccountId> {
        self.account.as_ref()
    }
}

/// A configuration change to a source that already exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceChange {
    /// Show the source under a different label.
    Relabel(SourceDisplayName),
    /// Follow a different branch of the same repository.
    TrackBranch(GitBranch),
    /// Read a different notes folder of the same repository.
    TrackNotesFolder(NotesFolder),
    /// Accept a new remote URL, on evidence obtained for that remote naming the
    /// repository and authorization this source already holds.
    Relocate {
        remote: RemoteUrl,
        evidence: RepositoryEvidence,
    },
    /// Replace the authorizing account and the credential handle its
    /// reauthorization produced.
    Reauthorize {
        account: ProviderAccountId,
        credential: CredentialRef,
    },
}

/// What an updated source keeps from the record it replaces.
///
/// This states what the change is entitled to; retaining or dropping a cached
/// generation and device-owned reading state is the sync and storage layers'
/// work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetainedSourceState {
    /// Local identity, any ready generation and cached assets, and reading state.
    Everything,
    /// Local identity and reading state. The tracked content moved, so a ready
    /// generation no longer answers for the source and must be rebuilt.
    IdentityAndReadingState,
    /// Local identity alone. A new authorizing account inherits neither the
    /// previous account's cache nor reading state scoped to it.
    IdentityOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceTransition {
    record: SourceRecord,
    retained: RetainedSourceState,
}

impl SourceTransition {
    #[must_use]
    pub fn record(&self) -> &SourceRecord {
        &self.record
    }

    #[must_use]
    pub fn into_record(self) -> SourceRecord {
        self.record
    }

    #[must_use]
    pub fn retained(&self) -> RetainedSourceState {
        self.retained
    }
}

/// Why a configuration change was refused. The source record stays unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceChangeError {
    DifferentRepository,
    DifferentProvider,
    UnobservedRemote,
    DifferentAuthorization,
    UnprovableRepository,
    UnsupportedRemote(SourceRecordError),
    NotAuthorized,
}

impl fmt::Display for SourceChangeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::DifferentRepository => {
                "the evidence names a different repository, which is a new source rather than an heir to this one"
            }
            Self::DifferentProvider => "the evidence names a different provider",
            Self::UnobservedRemote => {
                "the evidence was obtained for a different remote, so it says nothing about this one"
            }
            Self::DifferentAuthorization => {
                "the evidence was obtained under a different authorization than this source holds"
            }
            Self::UnprovableRepository => {
                "this source carries no provider repository identity, so a remote URL change cannot be proven to reach the same repository"
            }
            Self::UnsupportedRemote(error) => return error.fmt(formatter),
            Self::NotAuthorized => "a public source has no authorization to replace",
        };
        formatter.write_str(message)
    }
}

impl Error for SourceChangeError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn source_id(seed: u8) -> SourceId {
        SourceId::mint([seed; SOURCE_ID_ENTROPY_BYTES])
    }

    fn public_configuration(id: SourceId) -> SourceConfiguration {
        SourceConfiguration {
            id,
            display_name: SourceDisplayName::parse("Field notes").expect("a plain label"),
            provider: SourceProvider::GenericHttps,
            visibility: SourceVisibility::Public,
            provider_repository_id: None,
            account: None,
            remote: RemoteUrl::parse("https://git.example.org/notes.git").expect("a plain remote"),
            branch: GitBranch::parse("main").expect("a plain branch"),
            notes_folder: NotesFolder::parse("notes").expect("a plain folder"),
            credential: None,
        }
    }

    fn private_configuration(id: SourceId) -> SourceConfiguration {
        SourceConfiguration {
            id,
            display_name: SourceDisplayName::parse("Private slip-box").expect("a plain label"),
            provider: SourceProvider::GitHub,
            visibility: SourceVisibility::Private,
            provider_repository_id: Some(
                ProviderRepositoryId::parse("R_kgDOAbCdEf").expect("a provider identity"),
            ),
            account: Some(ProviderAccountId::parse("U_kgDOAbCdEf").expect("an account identity")),
            remote: RemoteUrl::parse("https://github.com/owner/slipbox.git").expect("a remote"),
            branch: GitBranch::parse("main").expect("a plain branch"),
            notes_folder: NotesFolder::root(),
            credential: Some(
                CredentialRef::parse("slipbox.source.vault-handle-1").expect("a handle"),
            ),
        }
    }

    #[test]
    fn a_minted_identity_is_hexadecimal_and_round_trips() {
        let minted = SourceId::mint([
            0x0f, 0xa0, 0x01, 0x10, 0xff, 0x00, 0x5c, 0x3d, 0, 1, 2, 3, 4, 5, 6, 7,
        ]);
        assert_eq!(minted.as_str(), "0fa00110ff005c3d0001020304050607");
        assert_eq!(SourceId::parse(minted.as_str()), Ok(minted));
    }

    #[test]
    fn a_stored_token_is_the_name_the_value_reports() {
        for provider in [SourceProvider::GenericHttps, SourceProvider::GitHub] {
            let encoded = serde_json::to_string(&provider).expect("a provider encodes");
            assert_eq!(encoded, format!("\"{}\"", provider.as_str()));
            assert_eq!(
                serde_json::from_str::<SourceProvider>(&encoded).expect("a provider decodes"),
                provider
            );
        }
        for visibility in [SourceVisibility::Public, SourceVisibility::Private] {
            let encoded = serde_json::to_string(&visibility).expect("a visibility encodes");
            assert_eq!(encoded, format!("\"{}\"", visibility.as_str()));
        }
        for scope in [
            SourceScope::Note,
            SourceScope::File,
            SourceScope::Asset,
            SourceScope::ReadingReference,
        ] {
            let encoded = serde_json::to_string(&scope).expect("a scope encodes");
            assert_eq!(encoded, format!("\"{}\"", scope.as_str()));
            assert_eq!(SourceScope::from_token(scope.as_str()), Some(scope));
        }
    }

    #[test]
    fn a_minted_identity_carries_nothing_from_the_repository() {
        let first = source_id(1);
        let second = source_id(2);
        let configuration = public_configuration(first.clone());
        assert!(
            !configuration
                .remote
                .as_str()
                .contains(configuration.id.as_str()),
            "the identity appears in the remote it was minted beside"
        );
        assert_ne!(first, second);
    }

    #[test]
    fn an_identity_refuses_a_wrong_length_or_charset() {
        assert_eq!(
            SourceId::parse("abc"),
            Err(SourceIdError::Length { found: 3 })
        );
        assert_eq!(
            SourceId::parse(&"a".repeat(SOURCE_ID_CHARS + 1)),
            Err(SourceIdError::Length {
                found: SOURCE_ID_CHARS + 1
            })
        );
        // An uppercase spelling of one identity would be a second spelling.
        assert_eq!(
            SourceId::parse("0FA00110FF005C3D0001020304050607"),
            Err(SourceIdError::Charset)
        );
        assert_eq!(
            SourceId::parse("0fa00110ff005c3d000102030405060g"),
            Err(SourceIdError::Charset)
        );
    }

    #[test]
    fn a_remote_url_normalizes_scheme_host_and_default_port() {
        for (input, expected) in [
            (
                "HTTPS://Git.Example.ORG/Owner/Notes.git",
                "https://git.example.org/Owner/Notes.git",
            ),
            (
                "https://git.example.org:443/owner/notes",
                "https://git.example.org/owner/notes",
            ),
            (
                "https://git.example.org:8443/owner/notes",
                "https://git.example.org:8443/owner/notes",
            ),
            (
                "  https://git.example.org/owner/notes/  ",
                "https://git.example.org/owner/notes",
            ),
        ] {
            assert_eq!(
                RemoteUrl::parse(input).map(|url| url.as_str().to_owned()),
                Ok(expected.to_owned()),
                "{input} did not normalize as expected"
            );
        }
    }

    #[test]
    fn a_remote_url_keeps_two_different_repositories_apart() {
        let lowercase = RemoteUrl::parse("https://git.example.org/owner/notes").expect("a remote");
        let mixed_case = RemoteUrl::parse("https://git.example.org/Owner/Notes").expect("a remote");
        let suffixed =
            RemoteUrl::parse("https://git.example.org/owner/notes.git").expect("a remote");
        let ported =
            RemoteUrl::parse("https://git.example.org:8443/owner/notes").expect("a remote");
        assert_ne!(lowercase, mixed_case, "a case-sensitive path was folded");
        assert_ne!(lowercase, suffixed, "a .git suffix was dropped");
        assert_ne!(lowercase, ported, "a non-default port was dropped");
    }

    #[test]
    fn a_remote_url_refuses_credentials_and_unsupported_transports() {
        for (input, expected) in [
            (
                "http://git.example.org/owner/notes",
                RemoteUrlError::UnsupportedScheme,
            ),
            (
                "git://git.example.org/owner/notes",
                RemoteUrlError::UnsupportedScheme,
            ),
            (
                "ssh://git@git.example.org/owner/notes",
                RemoteUrlError::UnsupportedScheme,
            ),
            (
                "git@github.com:owner/notes.git",
                RemoteUrlError::UnsupportedScheme,
            ),
            ("file:///srv/git/notes", RemoteUrlError::UnsupportedScheme),
            (
                "https://owner:secret@git.example.org/owner/notes",
                RemoteUrlError::CredentialInAuthority,
            ),
            (
                "https://@git.example.org/owner/notes",
                RemoteUrlError::CredentialInAuthority,
            ),
            (
                "https://git.example.org/owner/notes?access_token=secret",
                RemoteUrlError::QueryOrFragment,
            ),
            (
                "https://git.example.org/owner/notes#secret",
                RemoteUrlError::QueryOrFragment,
            ),
            ("https:///owner/notes", RemoteUrlError::MissingHost),
            ("https://git.example.org", RemoteUrlError::MissingPath),
            ("https://git.example.org/", RemoteUrlError::MissingPath),
            (
                "https://git.example.org/owner/../../etc/passwd",
                RemoteUrlError::InvalidPath,
            ),
            (
                "https://git.example.org/owner//notes",
                RemoteUrlError::InvalidPath,
            ),
            (
                "https://git.example.org/owner/notes%2Fescaped",
                RemoteUrlError::InvalidPath,
            ),
            (
                "https://git.example.org/owner/my notes",
                RemoteUrlError::InvalidPath,
            ),
            (
                "https://git.exämple.org/owner/notes",
                RemoteUrlError::InvalidHost,
            ),
            (
                "https://git..example.org/owner/notes",
                RemoteUrlError::InvalidHost,
            ),
            (
                "https://[2001:db8::1]/owner/notes",
                RemoteUrlError::InvalidHost,
            ),
            (
                "https://git.example.org:/owner/notes",
                RemoteUrlError::InvalidPort,
            ),
            (
                "https://git.example.org:0/owner/notes",
                RemoteUrlError::InvalidPort,
            ),
            (
                "https://git.example.org:0443/owner/notes",
                RemoteUrlError::InvalidPort,
            ),
            (
                "https://git.example.org:99999/owner/notes",
                RemoteUrlError::InvalidPort,
            ),
        ] {
            assert_eq!(
                RemoteUrl::parse(input),
                Err(expected),
                "{input} was accepted"
            );
        }
        assert_eq!(
            RemoteUrl::parse(&format!(
                "https://git.example.org/{}",
                "a".repeat(MAX_REMOTE_URL_CHARS)
            )),
            Err(RemoteUrlError::TooLong {
                limit: MAX_REMOTE_URL_CHARS
            })
        );
    }

    #[test]
    fn a_refused_remote_url_error_repeats_nothing_of_it() {
        let refusal = RemoteUrl::parse("https://owner:s3cr3t-token@git.example.org/owner/notes")
            .expect_err("userinfo is refused");
        let reported = refusal.to_string();
        for leaked in ["s3cr3t", "token", "owner", "git.example.org"] {
            assert!(
                !reported.contains(leaked),
                "the refusal repeats {leaked}: {reported}"
            );
        }
    }

    #[test]
    fn a_branch_follows_the_git_ref_grammar() {
        for accepted in [
            "main",
            "release/0.19",
            "feature/ünïcode",
            "v1.0",
            "a.b",
            "head",
            "Head",
            "HEADS",
            "HEAD-1",
            "my/HEAD",
            "FETCH_HEAD",
            "ORIG_HEAD",
            "üHEAD",
        ] {
            assert!(
                GitBranch::parse(accepted).is_ok(),
                "{accepted} is a valid branch name"
            );
        }
        for (input, expected) in [
            ("", GitBranchError::Empty),
            ("   ", GitBranchError::Empty),
            ("has space", GitBranchError::InvalidCharacter),
            ("has\ttab", GitBranchError::InvalidCharacter),
            ("caret^", GitBranchError::InvalidCharacter),
            ("tilde~1", GitBranchError::InvalidCharacter),
            ("colon:name", GitBranchError::InvalidCharacter),
            ("question?", GitBranchError::InvalidCharacter),
            ("star*", GitBranchError::InvalidCharacter),
            ("open[bracket", GitBranchError::InvalidCharacter),
            ("back\\slash", GitBranchError::InvalidCharacter),
            ("new\nline", GitBranchError::InvalidCharacter),
            ("double..dot", GitBranchError::ReservedSequence),
            ("at@{brace", GitBranchError::ReservedSequence),
            ("@", GitBranchError::ReservedName),
            ("HEAD", GitBranchError::ReservedName),
            ("  HEAD  ", GitBranchError::ReservedName),
            ("-leading", GitBranchError::InvalidComponent),
            ("/leading", GitBranchError::InvalidComponent),
            ("trailing/", GitBranchError::InvalidComponent),
            ("trailing.", GitBranchError::InvalidComponent),
            ("double//slash", GitBranchError::InvalidComponent),
            (".hidden", GitBranchError::InvalidComponent),
            ("release/.hidden", GitBranchError::InvalidComponent),
            ("release/held.lock", GitBranchError::InvalidComponent),
        ] {
            assert_eq!(
                GitBranch::parse(input),
                Err(expected),
                "{input:?} was accepted"
            );
        }
        assert_eq!(
            GitBranch::parse(&"b".repeat(MAX_BRANCH_CHARS + 1)),
            Err(GitBranchError::TooLong {
                limit: MAX_BRANCH_CHARS
            })
        );
    }

    #[test]
    fn a_branch_name_is_case_sensitive() {
        assert_ne!(
            GitBranch::parse("main").expect("a branch"),
            GitBranch::parse("Main").expect("a branch"),
        );
    }

    #[test]
    fn a_notes_folder_refuses_traversal_and_escaping_paths() {
        for (input, expected) in [
            ("/absolute", NotesFolderError::Absolute),
            ("..", NotesFolderError::Traversal),
            ("notes/../../etc", NotesFolderError::Traversal),
            ("notes//deep", NotesFolderError::EmptySegment),
            ("notes/./deep", NotesFolderError::ReservedSegment),
            (".git", NotesFolderError::ReservedSegment),
            ("notes/.git", NotesFolderError::ReservedSegment),
            ("notes\\deep", NotesFolderError::InvalidCharacter),
            ("C:/notes", NotesFolderError::InvalidCharacter),
            ("notes\u{0}deep", NotesFolderError::InvalidCharacter),
        ] {
            assert_eq!(
                NotesFolder::parse(input),
                Err(expected),
                "{input:?} was accepted"
            );
        }
        assert_eq!(
            NotesFolder::parse(&"f".repeat(MAX_NOTES_FOLDER_CHARS + 1)),
            Err(NotesFolderError::TooLong {
                limit: MAX_NOTES_FOLDER_CHARS
            })
        );
    }

    #[test]
    fn a_notes_folder_normalizes_the_root_and_keeps_case() {
        for root in ["", ".", "   "] {
            let parsed = NotesFolder::parse(root).expect("a root folder");
            assert!(parsed.is_root());
            assert_eq!(parsed.as_str(), "");
        }
        let trailing = NotesFolder::parse("notes/subject/").expect("a folder");
        assert_eq!(trailing.as_str(), "notes/subject");
        assert!(!trailing.is_root());
        assert_ne!(
            NotesFolder::parse("Notes").expect("a folder"),
            NotesFolder::parse("notes").expect("a folder"),
        );
        assert_eq!(
            NotesFolder::parse("notes/日本語")
                .expect("a folder")
                .as_str(),
            "notes/日本語"
        );
    }

    #[test]
    fn a_display_name_is_trimmed_but_never_authority() {
        assert_eq!(
            SourceDisplayName::parse("  Field notes  ")
                .expect("a label")
                .as_str(),
            "Field notes"
        );
        assert_eq!(
            SourceDisplayName::parse("Notes\nsecond line")
                .expect_err("a control character is refused")
                .reason(),
            SourceTextReason::Charset
        );
        assert_eq!(
            SourceDisplayName::parse("   ")
                .expect_err("blank is refused")
                .reason(),
            SourceTextReason::Empty
        );
        assert_eq!(
            SourceDisplayName::parse(&"n".repeat(MAX_DISPLAY_NAME_CHARS + 1))
                .expect_err("an oversized label is refused")
                .reason(),
            SourceTextReason::TooLong {
                limit: MAX_DISPLAY_NAME_CHARS
            }
        );
        assert!(SourceDisplayName::parse("日本語のノート").is_ok());
    }

    #[test]
    fn a_credential_reference_refuses_a_token_spelling() {
        for token in [
            "ghp_0123456789abcdefghijklmnopqrstuvwx",
            "GHP_0123456789abcdefghijklmnopqrstuvwx",
            "github_pat_11ABCDEFG0abcdefghijkl",
            "ghs_0123456789abcdefghijklmnopqrstuvwx",
        ] {
            assert_eq!(
                CredentialRef::parse(token)
                    .expect_err("a token spelling is refused")
                    .reason(),
                SourceTextReason::CredentialLike,
                "{token} was accepted as a handle"
            );
        }
        for handle in ["slipbox.source.1", "vault:handle/2", "a"] {
            assert!(
                CredentialRef::parse(handle).is_ok(),
                "{handle} is a plain handle"
            );
        }
        assert_eq!(
            CredentialRef::parse("has space")
                .expect_err("whitespace is refused")
                .reason(),
            SourceTextReason::Charset
        );
    }

    #[test]
    fn provider_identities_refuse_blank_and_unprintable_values() {
        let reason = |error: SourceTextError| error.reason();
        let refused = [
            ProviderRepositoryId::parse("").err().map(reason),
            ProviderRepositoryId::parse(" R_1").err().map(reason),
            ProviderAccountId::parse("").err().map(reason),
            ProviderAccountId::parse("U_1\u{7f}").err().map(reason),
        ];
        assert_eq!(
            refused,
            [
                Some(SourceTextReason::Empty),
                Some(SourceTextReason::Charset),
                Some(SourceTextReason::Empty),
                Some(SourceTextReason::Charset),
            ]
        );
        assert!(ProviderRepositoryId::parse("R_kgDOAbCdEf").is_ok());
        assert!(ProviderAccountId::parse("U_kgDOAbCdEf").is_ok());
    }

    #[test]
    fn a_public_record_holds_no_authorization_state() {
        let mut configuration = public_configuration(source_id(3));
        configuration.account = Some(ProviderAccountId::parse("U_1").expect("an account"));
        assert_eq!(
            SourceRecord::new(configuration.clone()),
            Err(SourceRecordError::UnexpectedAccountIdentity)
        );

        configuration.account = None;
        configuration.credential = Some(CredentialRef::parse("handle").expect("a handle"));
        assert_eq!(
            SourceRecord::new(configuration),
            Err(SourceRecordError::UnexpectedCredentialReference)
        );
    }

    #[test]
    fn a_public_generic_https_record_needs_no_provider_owned_identity() {
        let record =
            SourceRecord::new(public_configuration(source_id(4))).expect("a public source");
        assert_eq!(record.provider_repository_id(), None);
        assert_eq!(record.account(), None);
        assert_eq!(record.credential(), None);
        assert!(!record.is_private());
    }

    #[test]
    fn a_private_record_needs_the_resolved_repository_and_account() {
        let base = private_configuration(source_id(5));

        let mut without_repository = base.clone();
        without_repository.provider_repository_id = None;
        assert_eq!(
            SourceRecord::new(without_repository),
            Err(SourceRecordError::MissingRepositoryIdentity)
        );

        let mut without_account = base.clone();
        without_account.account = None;
        assert_eq!(
            SourceRecord::new(without_account),
            Err(SourceRecordError::MissingAccountIdentity)
        );

        let mut without_credential = base.clone();
        without_credential.credential = None;
        assert_eq!(
            SourceRecord::new(without_credential),
            Err(SourceRecordError::MissingCredentialReference)
        );

        assert!(SourceRecord::new(base).is_ok());
    }

    #[test]
    fn a_private_source_on_a_deferred_provider_is_refused() {
        let mut configuration = private_configuration(source_id(6));
        configuration.provider = SourceProvider::GenericHttps;
        assert_eq!(
            SourceRecord::new(configuration),
            Err(SourceRecordError::UnsupportedPrivateProvider(
                SourceProvider::GenericHttps
            ))
        );
        assert!(!SourceProvider::GenericHttps.supports_private_access());
        assert!(SourceProvider::GitHub.supports_private_access());
    }

    #[test]
    fn a_generic_source_carries_no_provider_owned_repository_identity() {
        let marker = "zvv-marker-8842";
        for remote in [
            "https://forge-a.example/team/notes.git",
            "https://forge-b.example/team/unrelated.git",
            "https://forge-a.example:8443/team/unrelated.git",
        ] {
            let mut configuration = public_configuration(source_id(0x30));
            configuration.remote = RemoteUrl::parse(remote).expect("a remote");
            configuration.provider_repository_id =
                Some(ProviderRepositoryId::parse(marker).expect("a provider identity"));
            assert_eq!(
                SourceRecord::new(configuration.clone()),
                Err(SourceRecordError::UnsupportedProviderRepositoryId(
                    SourceProvider::GenericHttps
                )),
                "{remote} kept a repository identity no provider issued"
            );

            let document = serde_json::to_string(&configuration).expect("a configuration encodes");
            let refused = serde_json::from_str::<SourceRecord>(&document)
                .expect_err("a stored record passes the same rule");
            assert!(
                !refused.to_string().contains(marker),
                "a refusal repeated the identity it rejected: {refused}"
            );
        }

        for remote in [
            "https://forge-a.example/team/notes.git",
            "https://forge-a.example/team/group/notes.git",
            "https://forge-a.example:8443/team/notes.git",
        ] {
            let mut configuration = public_configuration(source_id(0x31));
            configuration.remote = RemoteUrl::parse(remote).expect("a remote");
            let record = SourceRecord::new(configuration)
                .unwrap_or_else(|error| panic!("{remote} is a generic source: {error}"));
            assert_eq!(record.provider_repository_id(), None);
        }

        let resolved = ProviderRepositoryId::parse("R_kgDOfixture").expect("a provider identity");
        let mut github = public_configuration(source_id(0x32));
        github.provider = SourceProvider::GitHub;
        github.remote = RemoteUrl::parse("https://github.com/owner/notes.git").expect("a remote");
        assert_eq!(
            SourceRecord::new(github.clone())
                .expect("a public GitHub source before its repository is resolved")
                .provider_repository_id(),
            None
        );
        github.provider_repository_id = Some(resolved.clone());
        assert_eq!(
            SourceRecord::new(github)
                .expect("a public GitHub source")
                .provider_repository_id(),
            Some(&resolved)
        );
        assert!(SourceRecord::new(private_configuration(source_id(0x33))).is_ok());
    }

    #[test]
    fn an_unprovable_generic_source_cannot_move_to_another_authority() {
        let record = SourceRecord::new(public_configuration(source_id(0x34))).expect("a source");
        let configured = record.remote().clone();

        for proposed in [
            "https://forge-b.example/team/unrelated.git",
            "https://git.example.org:8443/team/unrelated.git",
        ] {
            let remote = RemoteUrl::parse(proposed).expect("a remote");
            assert_eq!(
                record.apply(SourceChange::Relocate {
                    remote: remote.clone(),
                    evidence: RepositoryEvidence::public(
                        SourceProvider::GenericHttps,
                        ProviderRepositoryId::parse("17").expect("an identity"),
                        remote,
                    ),
                }),
                Err(SourceChangeError::UnprovableRepository),
                "{proposed} inherited an identity from an unrelated authority"
            );
        }
        assert_eq!(record.remote(), &configured);
    }

    #[test]
    fn a_provider_record_is_refused_for_a_remote_that_provider_does_not_serve() {
        for (remote, expected) in [
            (
                "https://unrelated.example.org/owner/private.git",
                SourceRecordError::UnsupportedProviderAuthority(SourceProvider::GitHub),
            ),
            (
                "https://github.com.attacker.example.org/owner/private.git",
                SourceRecordError::UnsupportedProviderAuthority(SourceProvider::GitHub),
            ),
            (
                "https://notgithub.com/owner/private.git",
                SourceRecordError::UnsupportedProviderAuthority(SourceProvider::GitHub),
            ),
            (
                "https://api.github.com/owner/private.git",
                SourceRecordError::UnsupportedProviderAuthority(SourceProvider::GitHub),
            ),
            (
                "https://github.com:8443/owner/private.git",
                SourceRecordError::UnsupportedProviderAuthority(SourceProvider::GitHub),
            ),
            (
                "https://github.com/private.git",
                SourceRecordError::UnsupportedProviderRepositoryPath(SourceProvider::GitHub),
            ),
            (
                "https://github.com/owner/private/extra.git",
                SourceRecordError::UnsupportedProviderRepositoryPath(SourceProvider::GitHub),
            ),
        ] {
            let mut configuration = private_configuration(source_id(17));
            configuration.remote = RemoteUrl::parse(remote).expect("a well-formed remote");
            assert_eq!(
                SourceRecord::new(configuration),
                Err(expected),
                "{remote} was accepted for a GitHub source"
            );
        }

        assert_eq!(
            RemoteUrl::parse("https://owner:token@github.com/owner/private.git"),
            Err(RemoteUrlError::CredentialInAuthority)
        );

        for accepted in [
            "https://github.com/owner/slipbox.git",
            "https://github.com/owner/slipbox",
            "HTTPS://GitHub.COM:443/owner/slipbox.git",
        ] {
            let mut configuration = private_configuration(source_id(18));
            configuration.remote = RemoteUrl::parse(accepted).expect("a well-formed remote");
            assert!(
                SourceRecord::new(configuration).is_ok(),
                "{accepted} is a GitHub repository remote"
            );
        }

        let mut public_github = public_configuration(source_id(19));
        public_github.provider = SourceProvider::GitHub;
        assert_eq!(
            SourceRecord::new(public_github.clone()),
            Err(SourceRecordError::UnsupportedProviderAuthority(
                SourceProvider::GitHub
            )),
            "a public GitHub record escaped the provider's own host"
        );
        public_github.remote =
            RemoteUrl::parse("https://github.com/owner/notes.git").expect("a remote");
        assert!(SourceRecord::new(public_github).is_ok());

        let mut generic = public_configuration(source_id(20));
        generic.remote =
            RemoteUrl::parse("https://git.example.org/deep/nested/notes.git").expect("a remote");
        assert!(SourceRecord::new(generic).is_ok());
    }

    #[test]
    fn a_scoped_key_separates_sources_and_scopes() {
        let first = source_id(7);
        let second = source_id(8);
        // The engine's own key for one file position in two corpora.
        let node_key = "heading:notes.org:3";

        let in_first = SourceScopedKey::note(&first, node_key).expect("a scoped key");
        let in_second = SourceScopedKey::note(&second, node_key).expect("a scoped key");
        assert_ne!(in_first, in_second);
        assert_ne!(in_first.storage_key(), in_second.storage_key());
        assert_eq!(in_first.key(), node_key);

        let as_file = SourceScopedKey::file(&first, node_key).expect("a scoped key");
        let as_asset = SourceScopedKey::asset(&first, node_key).expect("a scoped key");
        let as_reading =
            SourceScopedKey::reading_reference(&first, node_key).expect("a scoped key");
        for other in [&as_file, &as_asset, &as_reading] {
            assert_ne!(&in_first, other, "one key spelling collapsed two scopes");
        }
        assert_eq!(as_file.scope(), SourceScope::File);
        assert_eq!(as_reading.source(), &first);
    }

    #[test]
    fn a_storage_key_round_trips_a_key_holding_the_separator() {
        let source = source_id(9);
        for key in [
            "heading:notes.org:3",
            "file:notes.org",
            "assets/diagram one.png",
            "日本語/ノート.org",
        ] {
            for scope in [
                SourceScope::Note,
                SourceScope::File,
                SourceScope::Asset,
                SourceScope::ReadingReference,
            ] {
                let scoped =
                    SourceScopedKey::new(source.clone(), scope, key).expect("a scoped key");
                assert_eq!(
                    SourceScopedKey::parse_storage_key(&scoped.storage_key()),
                    Ok(scoped)
                );
            }
        }
    }

    #[test]
    fn a_scoped_key_refuses_an_unusable_key_or_storage_spelling() {
        let source = source_id(10);
        assert_eq!(
            SourceScopedKey::note(&source, "").expect_err("an empty key is refused"),
            SourceScopedKeyError::Key(SourceTextError {
                field: "a source-scoped key",
                reason: SourceTextReason::Empty
            })
        );
        assert!(SourceScopedKey::note(&source, " leading").is_err());
        assert!(SourceScopedKey::note(&source, "new\nline").is_err());
        assert!(SourceScopedKey::note(&source, &"k".repeat(MAX_SCOPED_KEY_CHARS + 1)).is_err());

        assert_eq!(
            SourceScopedKey::parse_storage_key("nothing-separated"),
            Err(SourceScopedKeyError::Malformed)
        );
        assert_eq!(
            SourceScopedKey::parse_storage_key(&format!("{source}:note")),
            Err(SourceScopedKeyError::Malformed)
        );
        assert_eq!(
            SourceScopedKey::parse_storage_key(&format!("{source}:trail:key")),
            Err(SourceScopedKeyError::UnknownScope)
        );
        assert!(matches!(
            SourceScopedKey::parse_storage_key("not-an-id:note:key"),
            Err(SourceScopedKeyError::Source(_))
        ));
    }

    #[test]
    fn a_branch_or_folder_change_keeps_identity_and_reading_state() {
        let record = SourceRecord::new(public_configuration(source_id(11))).expect("a source");

        let moved = record
            .apply(SourceChange::TrackBranch(
                GitBranch::parse("release/0.19").expect("a branch"),
            ))
            .expect("a branch change is accepted");
        assert_eq!(
            moved.retained(),
            RetainedSourceState::IdentityAndReadingState
        );
        assert_eq!(moved.record().id(), record.id());
        assert_eq!(moved.record().branch().as_str(), "release/0.19");
        assert_eq!(moved.record().remote(), record.remote());

        let moved = record
            .apply(SourceChange::TrackNotesFolder(
                NotesFolder::parse("notes/subject").expect("a folder"),
            ))
            .expect("a folder change is accepted");
        assert_eq!(
            moved.retained(),
            RetainedSourceState::IdentityAndReadingState
        );
        assert_eq!(moved.record().notes_folder().as_str(), "notes/subject");

        let unchanged = record
            .apply(SourceChange::TrackBranch(record.branch().clone()))
            .expect("the same branch is accepted");
        assert_eq!(unchanged.retained(), RetainedSourceState::Everything);
        assert_eq!(unchanged.into_record(), record);
    }

    #[test]
    fn a_relabelled_source_keeps_everything() {
        let record = SourceRecord::new(public_configuration(source_id(12))).expect("a source");
        let relabelled = record
            .apply(SourceChange::Relabel(
                SourceDisplayName::parse("Renamed on the device").expect("a label"),
            ))
            .expect("a label change is accepted");
        assert_eq!(relabelled.retained(), RetainedSourceState::Everything);
        assert_eq!(relabelled.record().id(), record.id());
        assert_eq!(relabelled.record().remote(), record.remote());
    }

    #[test]
    fn a_proven_rename_keeps_the_source_and_an_unproven_one_does_not() {
        let record = SourceRecord::new(private_configuration(source_id(13))).expect("a source");
        let renamed = RemoteUrl::parse("https://github.com/owner/renamed.git").expect("a remote");
        let repository = record
            .provider_repository_id()
            .expect("a private source resolved its repository")
            .clone();
        let account = record.account().expect("an authorizing account").clone();

        let relocated = record
            .apply(SourceChange::Relocate {
                remote: renamed.clone(),
                evidence: RepositoryEvidence::authorized(
                    SourceProvider::GitHub,
                    repository.clone(),
                    renamed.clone(),
                    account.clone(),
                ),
            })
            .expect("evidence for the same repository is accepted");
        assert_eq!(relocated.retained(), RetainedSourceState::Everything);
        assert_eq!(relocated.record().id(), record.id());
        assert_eq!(relocated.record().remote(), &renamed);

        let public = SourceRecord::new(public_configuration(source_id(14))).expect("a source");
        assert_eq!(
            public.apply(SourceChange::Relocate {
                remote: renamed,
                evidence: RepositoryEvidence::public(
                    SourceProvider::GenericHttps,
                    ProviderRepositoryId::parse("R_1").expect("an identity"),
                    public.remote().clone(),
                ),
            }),
            Err(SourceChangeError::UnprovableRepository)
        );
    }

    #[test]
    fn evidence_holds_only_for_the_remote_and_authorization_it_names() {
        let record = SourceRecord::new(private_configuration(source_id(21))).expect("a source");
        let repository = record
            .provider_repository_id()
            .expect("a repository")
            .clone();
        let account = record.account().expect("an account").clone();
        let proposed = RemoteUrl::parse("https://github.com/owner/renamed.git").expect("a remote");
        let observed = RemoteUrl::parse("https://github.com/owner/watched.git").expect("a remote");

        for (evidence, expected) in [
            (
                RepositoryEvidence::authorized(
                    SourceProvider::GitHub,
                    repository.clone(),
                    observed,
                    account.clone(),
                ),
                SourceChangeError::UnobservedRemote,
            ),
            (
                RepositoryEvidence::public(
                    SourceProvider::GitHub,
                    repository.clone(),
                    proposed.clone(),
                ),
                SourceChangeError::DifferentAuthorization,
            ),
            (
                RepositoryEvidence::authorized(
                    SourceProvider::GitHub,
                    repository.clone(),
                    proposed.clone(),
                    ProviderAccountId::parse("U_kgDOanotherAccount").expect("an account"),
                ),
                SourceChangeError::DifferentAuthorization,
            ),
        ] {
            assert_eq!(
                record.apply(SourceChange::Relocate {
                    remote: proposed.clone(),
                    evidence,
                }),
                Err(expected)
            );
        }

        let elsewhere =
            RemoteUrl::parse("https://git.example.org/owner/renamed.git").expect("a remote");
        assert_eq!(
            record.apply(SourceChange::Relocate {
                remote: elsewhere.clone(),
                evidence: RepositoryEvidence::authorized(
                    SourceProvider::GitHub,
                    repository,
                    elsewhere,
                    account,
                ),
            }),
            Err(SourceChangeError::UnsupportedRemote(
                SourceRecordError::UnsupportedProviderAuthority(SourceProvider::GitHub)
            )),
            "a relocation moved a GitHub source off its provider's host"
        );
    }

    #[test]
    fn a_different_repository_reusing_a_url_inherits_nothing() {
        let record = SourceRecord::new(private_configuration(source_id(15))).expect("a source");

        assert_eq!(
            record.apply(SourceChange::Relocate {
                remote: record.remote().clone(),
                evidence: RepositoryEvidence::authorized(
                    SourceProvider::GitHub,
                    ProviderRepositoryId::parse("R_kgDOsomethingElse").expect("an identity"),
                    record.remote().clone(),
                    record.account().expect("an account").clone(),
                ),
            }),
            Err(SourceChangeError::DifferentRepository),
            "a changed provider repository identity was treated as the same repository"
        );

        assert_eq!(
            record.apply(SourceChange::Relocate {
                remote: RemoteUrl::parse("https://git.example.org/owner/slipbox.git")
                    .expect("a remote"),
                evidence: RepositoryEvidence::public(
                    SourceProvider::GenericHttps,
                    record
                        .provider_repository_id()
                        .expect("a resolved repository")
                        .clone(),
                    RemoteUrl::parse("https://git.example.org/owner/slipbox.git")
                        .expect("a remote"),
                ),
            }),
            Err(SourceChangeError::DifferentProvider)
        );
    }

    #[test]
    fn a_new_account_keeps_identity_without_the_previous_cache() {
        let record = SourceRecord::new(private_configuration(source_id(16))).expect("a source");
        let rotated = record
            .apply(SourceChange::Reauthorize {
                account: record.account().expect("an account").clone(),
                credential: CredentialRef::parse("slipbox.source.vault-handle-2")
                    .expect("a handle"),
            })
            .expect("rotating a credential is accepted");
        assert_eq!(rotated.retained(), RetainedSourceState::Everything);
        assert_eq!(
            rotated.record().credential().map(CredentialRef::as_str),
            Some("slipbox.source.vault-handle-2")
        );

        let reauthorized = record
            .apply(SourceChange::Reauthorize {
                account: ProviderAccountId::parse("U_kgDOsecondAccount").expect("an account"),
                credential: CredentialRef::parse("slipbox.source.vault-handle-3")
                    .expect("a handle"),
            })
            .expect("a second account is accepted with explicit reauthorization");
        assert_eq!(reauthorized.retained(), RetainedSourceState::IdentityOnly);
        assert_eq!(reauthorized.record().id(), record.id());

        let public = SourceRecord::new(public_configuration(source_id(17))).expect("a source");
        assert_eq!(
            public.apply(SourceChange::Reauthorize {
                account: ProviderAccountId::parse("U_1").expect("an account"),
                credential: CredentialRef::parse("handle").expect("a handle"),
            }),
            Err(SourceChangeError::NotAuthorized)
        );
    }
}
