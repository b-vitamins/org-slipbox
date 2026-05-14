use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::Result;
use chrono::{Local, NaiveDate};
use serde::Deserialize;
use slipbox_core::{AgendaResult, AnchorRecord, NodeKind, NodeRecord, SearchNodesResult};
use tempfile::{TempDir, tempdir};

mod support;

use support::{run_slipbox, scoped_server_args_with_file_extension};

#[derive(Debug, Deserialize)]
struct ErrorPayload {
    error: ErrorMessage,
}

#[derive(Debug, Deserialize)]
struct ErrorMessage {
    message: String,
}

fn build_fixture() -> Result<(TempDir, PathBuf, PathBuf)> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    let db = workspace.path().join("slipbox.sqlite");
    Ok((workspace, root, db))
}

fn daily_command(root: &Path, db: &Path, subcommand: &str, extra_args: &[String]) -> Vec<String> {
    let mut args = vec!["daily".to_owned(), subcommand.to_owned()];
    args.extend(scoped_server_args_with_file_extension(root, db, "org"));
    args.extend_from_slice(extra_args);
    args
}

fn org_date(date: NaiveDate) -> String {
    format!("<{} {}>", date.format("%Y-%m-%d"), date.format("%a"))
}

#[test]
fn daily_ensure_defaults_to_today_as_json() -> Result<()> {
    let (_workspace, root, db) = build_fixture()?;
    let today = Local::now().date_naive();
    let args = daily_command(&root, &db, "ensure", &["--json".to_owned()]);

    let output = run_slipbox(&args)?;

    assert!(output.status.success(), "{output:?}");
    let node: NodeRecord = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        node.file_path,
        format!("daily/{}.org", today.format("%Y-%m-%d"))
    );
    assert_eq!(node.title, today.format("%Y-%m-%d").to_string());
    assert_eq!(node.kind, NodeKind::File);
    assert!(node.explicit_id.is_some());
    assert!(output.stderr.is_empty());

    Ok(())
}

#[test]
fn daily_ensure_show_and_search_fixed_date() -> Result<()> {
    let (_workspace, root, db) = build_fixture()?;
    let ensure_args = daily_command(
        &root,
        &db,
        "ensure",
        &[
            "--date".to_owned(),
            "2026-05-12".to_owned(),
            "--json".to_owned(),
        ],
    );
    let ensure_output = run_slipbox(&ensure_args)?;
    assert!(ensure_output.status.success(), "{ensure_output:?}");
    let ensured: NodeRecord = serde_json::from_slice(&ensure_output.stdout)?;
    assert_eq!(ensured.file_path, "daily/2026-05-12.org");
    assert_eq!(ensured.title, "2026-05-12");

    let show_args = daily_command(
        &root,
        &db,
        "show",
        &[
            "--date".to_owned(),
            "2026-05-12".to_owned(),
            "--json".to_owned(),
        ],
    );
    let show_output = run_slipbox(&show_args)?;
    assert!(show_output.status.success(), "{show_output:?}");
    let shown: NodeRecord = serde_json::from_slice(&show_output.stdout)?;
    assert_eq!(shown.node_key, ensured.node_key);
    assert!(show_output.stderr.is_empty());

    let mut search_args = vec![
        "node".to_owned(),
        "search".to_owned(),
        "2026-05-12".to_owned(),
    ];
    search_args.extend(scoped_server_args_with_file_extension(&root, &db, "org"));
    search_args.push("--json".to_owned());
    let search_output = run_slipbox(&search_args)?;
    assert!(search_output.status.success(), "{search_output:?}");
    let search: SearchNodesResult = serde_json::from_slice(&search_output.stdout)?;
    assert!(
        search
            .nodes
            .iter()
            .any(|node| node.node_key == "file:daily/2026-05-12.org")
    );

    Ok(())
}

#[test]
fn daily_append_creates_entry_that_is_immediately_indexed_and_on_agenda() -> Result<()> {
    let (_workspace, root, db) = build_fixture()?;
    let date = NaiveDate::from_ymd_opt(2026, 5, 12).expect("valid fixed date");
    let args = daily_command(
        &root,
        &db,
        "append",
        &[
            "--date".to_owned(),
            "2026-05-12".to_owned(),
            "--heading".to_owned(),
            format!("TODO Standup\nSCHEDULED: {}", org_date(date)),
            "--json".to_owned(),
        ],
    );

    let output = run_slipbox(&args)?;

    assert!(output.status.success(), "{output:?}");
    let appended: AnchorRecord = serde_json::from_slice(&output.stdout)?;
    assert_eq!(appended.file_path, "daily/2026-05-12.org");
    assert_eq!(appended.title, "Standup");
    assert_eq!(appended.todo_keyword.as_deref(), Some("TODO"));
    assert_eq!(
        appended.scheduled_for.as_deref(),
        Some("2026-05-12T00:00:00")
    );
    assert!(output.stderr.is_empty());

    let mut agenda_args = vec![
        "agenda".to_owned(),
        "date".to_owned(),
        "2026-05-12".to_owned(),
    ];
    agenda_args.extend(scoped_server_args_with_file_extension(&root, &db, "org"));
    agenda_args.push("--json".to_owned());
    let agenda_output = run_slipbox(&agenda_args)?;
    assert!(agenda_output.status.success(), "{agenda_output:?}");
    let agenda: AgendaResult = serde_json::from_slice(&agenda_output.stdout)?;
    assert!(agenda.nodes.iter().any(|node| node.title == "Standup"));

    Ok(())
}

#[test]
fn daily_custom_directory_formats_and_head_are_explicit_cli_config() -> Result<()> {
    let (_workspace, root, db) = build_fixture()?;
    let args = daily_command(
        &root,
        &db,
        "ensure",
        &[
            "--date".to_owned(),
            "2026-05-12".to_owned(),
            "--directory".to_owned(),
            "journal".to_owned(),
            "--file-format".to_owned(),
            "%Y/%m/%d.org".to_owned(),
            "--title-format".to_owned(),
            "Journal %Y-%m-%d".to_owned(),
            "--head".to_owned(),
            "#+title: Journal %Y-%m-%d\n#+filetags: :daily:\n".to_owned(),
            "--json".to_owned(),
        ],
    );

    let output = run_slipbox(&args)?;

    assert!(output.status.success(), "{output:?}");
    let node: NodeRecord = serde_json::from_slice(&output.stdout)?;
    assert_eq!(node.file_path, "journal/2026/05/12.org");
    assert_eq!(node.title, "Journal 2026-05-12");
    assert_eq!(node.tags, vec!["daily"]);
    let source = fs::read_to_string(root.join("journal/2026/05/12.org"))?;
    assert!(source.starts_with("#+title: Journal 2026-05-12\n#+filetags: :daily:\n"));
    assert!(source.contains(":ID: "));

    Ok(())
}

#[test]
fn daily_ensure_with_head_does_not_rewrite_existing_daily_note() -> Result<()> {
    let (_workspace, root, db) = build_fixture()?;
    let args = daily_command(
        &root,
        &db,
        "ensure",
        &[
            "--date".to_owned(),
            "2026-05-12".to_owned(),
            "--head".to_owned(),
            "#+title: Daily %Y-%m-%d\n".to_owned(),
            "--json".to_owned(),
        ],
    );

    let first_output = run_slipbox(&args)?;
    assert!(first_output.status.success(), "{first_output:?}");
    let first: NodeRecord = serde_json::from_slice(&first_output.stdout)?;
    assert_eq!(first.file_path, "daily/2026-05-12.org");
    let daily_path = root.join("daily/2026-05-12.org");
    let first_mtime = fs::metadata(&daily_path)?.modified()?;

    std::thread::sleep(Duration::from_millis(1100));

    let second_output = run_slipbox(&args)?;
    assert!(second_output.status.success(), "{second_output:?}");
    let second: NodeRecord = serde_json::from_slice(&second_output.stdout)?;
    assert_eq!(second.node_key, first.node_key);
    let second_mtime = fs::metadata(&daily_path)?.modified()?;
    assert_eq!(second_mtime, first_mtime);

    Ok(())
}

#[test]
fn daily_commands_report_structured_json_failures() -> Result<()> {
    let (_workspace, root, db) = build_fixture()?;
    let invalid_date = daily_command(
        &root,
        &db,
        "ensure",
        &[
            "--date".to_owned(),
            "05/12/2026".to_owned(),
            "--json".to_owned(),
        ],
    );
    let output = run_slipbox(&invalid_date)?;
    assert_eq!(output.status.code(), Some(1));
    let error: ErrorPayload = serde_json::from_slice(&output.stderr)?;
    assert!(
        error
            .error
            .message
            .contains("invalid daily date \"05/12/2026\"")
    );

    let absolute_directory = daily_command(
        &root,
        &db,
        "show",
        &[
            "--directory".to_owned(),
            root.display().to_string(),
            "--json".to_owned(),
        ],
    );
    let absolute_output = run_slipbox(&absolute_directory)?;
    assert_eq!(absolute_output.status.code(), Some(1));
    let absolute_error: ErrorPayload = serde_json::from_slice(&absolute_output.stderr)?;
    assert!(
        absolute_error
            .error
            .message
            .contains("daily --directory must be relative to --root")
    );

    for (args, expected_message) in [
        (
            vec![
                "--directory".to_owned(),
                "../outside".to_owned(),
                "--json".to_owned(),
            ],
            "daily file path must stay within --root",
        ),
        (
            vec![
                "--file-format".to_owned(),
                "../%Y-%m-%d.org".to_owned(),
                "--json".to_owned(),
            ],
            "daily file path must stay within --root",
        ),
        (
            vec![
                "--directory".to_owned(),
                "".to_owned(),
                "--file-format".to_owned(),
                "".to_owned(),
                "--json".to_owned(),
            ],
            "daily file path must not be empty",
        ),
        (
            vec![
                "--file-format".to_owned(),
                "%Y-%m-%d.txt".to_owned(),
                "--json".to_owned(),
            ],
            "daily file path must end with .org",
        ),
    ] {
        let output = run_slipbox(&daily_command(&root, &db, "ensure", &args))?;
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        let error: ErrorPayload = serde_json::from_slice(&output.stderr)?;
        assert!(
            error.error.message.contains(expected_message),
            "{:?}",
            error.error.message
        );
    }

    Ok(())
}
