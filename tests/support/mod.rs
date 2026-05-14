// Each integration test crate includes this support module independently and uses
// a different subset of helpers, so shared helpers can look dead per crate.
#![allow(dead_code)]

use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use anyhow::Result;
use serde::de::DeserializeOwned;
use serde_json::Value;

pub fn slipbox_binary() -> &'static str {
    env!("CARGO_BIN_EXE_slipbox")
}

pub fn scoped_server_args(root: impl AsRef<Path>, db: impl AsRef<Path>) -> Vec<String> {
    vec![
        "--root".to_owned(),
        root.as_ref().display().to_string(),
        "--db".to_owned(),
        db.as_ref().display().to_string(),
        "--server-program".to_owned(),
        slipbox_binary().to_owned(),
    ]
}

pub fn scoped_server_json_args(root: impl AsRef<Path>, db: impl AsRef<Path>) -> Vec<String> {
    let mut args = scoped_server_args(root, db);
    args.push("--json".to_owned());
    args
}

pub fn scoped_server_args_with_file_extension(
    root: impl AsRef<Path>,
    db: impl AsRef<Path>,
    extension: &str,
) -> Vec<String> {
    let mut args = scoped_server_args(root, db);
    args.extend(["--file-extension".to_owned(), extension.to_owned()]);
    args
}

pub fn run_slipbox(args: &[String]) -> Result<Output> {
    Ok(Command::new(slipbox_binary()).args(args).output()?)
}

pub fn run_slipbox_with_stdin(args: &[String], stdin: &[u8]) -> Result<Output> {
    let mut child = Command::new(slipbox_binary())
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    child
        .stdin
        .as_mut()
        .expect("child stdin should be piped")
        .write_all(stdin)?;
    Ok(child.wait_with_output()?)
}

pub fn json_command(
    command: &str,
    root: impl AsRef<Path>,
    db: impl AsRef<Path>,
    extra: &[&str],
) -> Result<Output> {
    json_command_path(&[command], root, db, extra)
}

pub fn json_command_path(
    command_path: &[&str],
    root: impl AsRef<Path>,
    db: impl AsRef<Path>,
    extra: &[&str],
) -> Result<Output> {
    let mut args = command_path
        .iter()
        .map(|word| (*word).to_owned())
        .collect::<Vec<_>>();
    args.extend(scoped_server_json_args(root, db));
    args.extend(extra.iter().map(|value| (*value).to_owned()));
    run_slipbox(&args)
}

pub fn json_command_path_with_bad_server(
    command_path: &[&str],
    root: impl AsRef<Path>,
    db: impl AsRef<Path>,
    extra: &[&str],
) -> Result<Output> {
    let mut args = command_path
        .iter()
        .map(|word| (*word).to_owned())
        .collect::<Vec<_>>();
    args.extend([
        "--root".to_owned(),
        root.as_ref().display().to_string(),
        "--db".to_owned(),
        db.as_ref().display().to_string(),
        "--server-program".to_owned(),
        "/definitely/not/a/real/slipbox-binary".to_owned(),
        "--json".to_owned(),
    ]);
    args.extend(extra.iter().map(|value| (*value).to_owned()));
    run_slipbox(&args)
}

pub fn parse_stdout<T>(output: &Output) -> Result<T>
where
    T: DeserializeOwned,
{
    Ok(serde_json::from_slice(&output.stdout)?)
}

pub fn parse_stderr<T>(output: &Output) -> Result<T>
where
    T: DeserializeOwned,
{
    Ok(serde_json::from_slice(&output.stderr)?)
}

pub fn run_json<T>(args: &[String]) -> Result<T>
where
    T: DeserializeOwned,
{
    let output = run_slipbox(args)?;
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    parse_stdout(&output)
}

pub fn assert_success_json(output: Output) -> Result<Value> {
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    parse_stdout(&output)
}

pub fn sorted_keys(value: &Value) -> Vec<String> {
    let object = value.as_object().expect("expected JSON object");
    let mut keys: Vec<String> = object.keys().cloned().collect();
    keys.sort();
    keys
}

pub fn assert_exact_object_keys(value: &Value, expected: &[&str]) {
    let mut expected_keys: Vec<String> = expected.iter().map(|key| (*key).to_owned()).collect();
    expected_keys.sort();
    assert_eq!(sorted_keys(value), expected_keys);
}

pub fn assert_error_failure(output: &Output, needle: &str) {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    let parsed: Value = parse_stderr(output).expect("stderr should be structured JSON");
    assert_exact_object_keys(&parsed, &["error"]);
    let message = parsed["error"]["message"]
        .as_str()
        .expect("error message should be a string");
    assert!(message.contains(needle), "{message}");
}

pub fn assert_node_record_keys(value: &Value) {
    assert_exact_object_keys(
        value,
        &[
            "node_key",
            "explicit_id",
            "file_path",
            "title",
            "outline_path",
            "aliases",
            "tags",
            "refs",
            "todo_keyword",
            "scheduled_for",
            "deadline_for",
            "closed_at",
            "level",
            "line",
            "kind",
            "file_mtime_ns",
            "backlink_count",
            "forward_link_count",
        ],
    );
}

pub fn assert_anchor_record_keys(value: &Value) {
    assert_node_record_keys(value);
}

pub fn assert_file_record_keys(value: &Value) {
    assert_exact_object_keys(value, &["file_path", "title", "mtime_ns", "node_count"]);
}

pub fn assert_occurrence_record_keys(value: &Value) {
    assert_exact_object_keys(
        value,
        &[
            "file_path",
            "row",
            "col",
            "preview",
            "matched_text",
            "owning_anchor",
        ],
    );
    assert_node_record_keys(&value["owning_anchor"]);
}
