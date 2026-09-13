//! Runtime authority for external programs the index would otherwise execute.

use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

use crate::discovery;

/// External program the index would have to execute to read a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalProgram {
    /// `gpg`, for a `.gpg` envelope.
    Gpg,
    /// `age`, falling back to `rage`, for an `.age` envelope.
    Age,
}

impl ExternalProgram {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Gpg => "gpg",
            Self::Age => "age",
        }
    }
}

impl fmt::Display for ExternalProgram {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Refusal raised before an unauthorized external program can run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedCapability {
    path: PathBuf,
    program: ExternalProgram,
}

impl UnsupportedCapability {
    fn new(path: &Path, program: ExternalProgram) -> Self {
        Self {
            path: path.to_path_buf(),
            program,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn program(&self) -> ExternalProgram {
        self.program
    }
}

impl fmt::Display for UnsupportedCapability {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "reading {} needs the external program {}, which this platform does not support",
            self.path.display(),
            self.program
        )
    }
}

impl Error for UnsupportedCapability {}

/// Platform capabilities the current embedding authorizes.
///
/// [`PlatformPolicy::default`] is [`PlatformPolicy::headless`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlatformPolicy {
    external_programs: bool,
}

impl PlatformPolicy {
    /// Deny all external programs.
    pub const fn headless() -> Self {
        Self {
            external_programs: false,
        }
    }

    /// Authorize decryptor helpers compiled by `desktop-decryptors`.
    #[cfg(feature = "desktop-decryptors")]
    pub const fn desktop() -> Self {
        Self {
            external_programs: true,
        }
    }

    pub const fn allows(&self, program: ExternalProgram) -> bool {
        match program {
            ExternalProgram::Gpg | ExternalProgram::Age => self.external_programs,
        }
    }

    /// Return `None` for plain text, an authorized decryptor for an envelope,
    /// or an unsupported-capability refusal.
    pub fn authorize_source(
        &self,
        path: &Path,
    ) -> Result<Option<ExternalProgram>, UnsupportedCapability> {
        let Some(program) = source_program(path) else {
            return Ok(None);
        };
        if self.allows(program) {
            Ok(Some(program))
        } else {
            Err(UnsupportedCapability::new(path, program))
        }
    }
}

impl Default for PlatformPolicy {
    fn default() -> Self {
        Self::headless()
    }
}

fn source_program(path: &Path) -> Option<ExternalProgram> {
    match discovery::envelope_extension(path).as_deref() {
        Some("gpg") => Some(ExternalProgram::Gpg),
        Some("age") => Some(ExternalProgram::Age),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_is_headless() {
        assert_eq!(PlatformPolicy::default(), PlatformPolicy::headless());
        assert!(!PlatformPolicy::default().allows(ExternalProgram::Gpg));
        assert!(!PlatformPolicy::default().allows(ExternalProgram::Age));
    }

    #[test]
    fn headless_policy_reads_plain_sources_in_process() {
        let authorized = PlatformPolicy::headless()
            .authorize_source(Path::new("/notes/plain.org"))
            .expect("a plain source needs no external program");
        assert_eq!(authorized, None);
    }

    #[test]
    fn headless_policy_refuses_every_encrypted_envelope() {
        for (path, program) in [
            ("/notes/secret.org.gpg", ExternalProgram::Gpg),
            ("/notes/secret.org.age", ExternalProgram::Age),
        ] {
            let refusal = PlatformPolicy::headless()
                .authorize_source(Path::new(path))
                .expect_err("an encrypted envelope needs an unauthorized program");
            assert_eq!(refusal.program(), program);
            assert_eq!(refusal.path(), Path::new(path));
            assert!(
                refusal.to_string().contains(program.as_str()),
                "refusal names the capability: {refusal}"
            );
        }
    }

    #[test]
    fn enabled_features_never_authorize_a_headless_policy() {
        assert!(!PlatformPolicy::default().allows(ExternalProgram::Gpg));
        assert!(
            PlatformPolicy::default()
                .authorize_source(Path::new("/notes/secret.org.gpg"))
                .is_err()
        );
    }

    #[cfg(feature = "desktop-decryptors")]
    #[test]
    fn desktop_policy_authorizes_the_decryptors() {
        let policy = PlatformPolicy::desktop();
        assert_eq!(
            policy
                .authorize_source(Path::new("/notes/secret.org.gpg"))
                .expect("desktop authorizes gpg"),
            Some(ExternalProgram::Gpg)
        );
        assert_eq!(
            policy
                .authorize_source(Path::new("/notes/secret.org.age"))
                .expect("desktop authorizes age"),
            Some(ExternalProgram::Age)
        );
        assert_eq!(
            policy
                .authorize_source(Path::new("/notes/plain.org"))
                .expect("a plain source needs no external program"),
            None
        );
    }
}
