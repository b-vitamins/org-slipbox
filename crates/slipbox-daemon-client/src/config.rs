use std::ffi::OsString;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonServeConfig {
    pub root: PathBuf,
    pub db: PathBuf,
    pub workflow_dirs: Vec<PathBuf>,
    pub file_extensions: Vec<String>,
    pub exclude_regexps: Vec<String>,
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
        }
    }

    pub(crate) fn command_args(&self) -> Vec<OsString> {
        let mut args = vec![
            OsString::from("serve"),
            OsString::from("--root"),
            self.root.as_os_str().to_owned(),
            OsString::from("--db"),
            self.db.as_os_str().to_owned(),
        ];
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
