use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonServeConfig {
    pub root: PathBuf,
    pub db: PathBuf,
    pub workflow_dirs: Vec<PathBuf>,
    pub file_extensions: Vec<String>,
    pub exclude_regexps: Vec<String>,
    pub read_only: bool,
}

impl DaemonServeConfig {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>, db: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            db: db.into(),
            workflow_dirs: Vec::new(),
            file_extensions: Vec::new(),
            exclude_regexps: Vec::new(),
            read_only: false,
        }
    }

    /// Spawn the daemon with `--read-only`, refusing every mutating method at
    /// dispatch.
    #[must_use]
    pub fn read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    pub(crate) fn command_args(&self) -> Vec<OsString> {
        let mut args = vec![
            OsString::from("serve"),
            OsString::from("--root"),
            self.root.as_os_str().to_owned(),
            OsString::from("--db"),
            self.db.as_os_str().to_owned(),
        ];
        if self.read_only {
            args.push(OsString::from("--read-only"));
        }
        for workflow_dir in &self.workflow_dirs {
            args.push(OsString::from("--workflow-dir"));
            args.push(workflow_dir.as_os_str().to_owned());
        }
        for extension in &self.file_extensions {
            args.push(OsString::from("--file-extension"));
            args.push(OsString::from(extension));
        }
        for regexp in &self.exclude_regexps {
            args.push(OsString::from("--exclude-regexp"));
            args.push(OsString::from(regexp));
        }
        args
    }
}

#[cfg(test)]
mod tests {
    use super::DaemonServeConfig;

    #[test]
    fn new_defaults_to_a_writable_session() {
        let config = DaemonServeConfig::new("/notes", "/index.sqlite");

        assert!(!config.read_only);
        assert!(!config.command_args().iter().any(|arg| arg == "--read-only"));
    }

    #[test]
    fn read_only_builder_emits_the_flag_after_the_db_pair() {
        let config = DaemonServeConfig::new("/notes", "/index.sqlite").read_only(true);

        assert!(config.read_only);
        assert_eq!(
            config.command_args(),
            [
                "serve",
                "--root",
                "/notes",
                "--db",
                "/index.sqlite",
                "--read-only"
            ]
        );
    }

    #[test]
    fn read_only_precedes_repeatable_discovery_flags() {
        let mut config = DaemonServeConfig::new("/notes", "/index.sqlite").read_only(true);
        config.workflow_dirs.push("/flows".into());
        config.file_extensions.push("org".to_owned());
        config.exclude_regexps.push("archive/".to_owned());

        let args = config.command_args();
        let flag = args
            .iter()
            .position(|arg| arg == "--read-only")
            .expect("flag present");
        let first_repeatable = args
            .iter()
            .position(|arg| arg == "--workflow-dir" || arg == "--file-extension")
            .expect("repeatable flag present");

        assert!(flag < first_repeatable);
    }

    #[test]
    fn read_only_builder_can_be_cleared() {
        let config = DaemonServeConfig::new("/notes", "/index.sqlite")
            .read_only(true)
            .read_only(false);

        assert!(!config.read_only);
        assert!(!config.command_args().iter().any(|arg| arg == "--read-only"));
    }
}
