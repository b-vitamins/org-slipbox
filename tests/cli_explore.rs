use std::fs;
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;

use anyhow::Result;
use serde::Deserialize;
use slipbox_core::{
    ExplorationEntry, ExplorationExplanation, ExplorationSectionKind, ExploreResult,
};
use slipbox_index::scan_root;
use slipbox_store::Database;
use tempfile::tempdir;

#[derive(Debug, Deserialize)]
struct ErrorPayload {
    error: ErrorMessage,
}

#[derive(Debug, Deserialize)]
struct ErrorMessage {
    message: String,
}

fn slipbox_binary() -> &'static str {
    env!("CARGO_BIN_EXE_slipbox")
}

fn build_indexed_fixture() -> Result<(tempfile::TempDir, String, String, String)> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    fs::write(
        root.join("older.org"),
        r#"#+title: Older

* Dormant Bridge
:PROPERTIES:
:ID: dormant-bridge-id
:ROAM_REFS: cite:shared2024
:END:
Links to [[id:neighbor-id]] and [[id:support-id]].

* Support
:PROPERTIES:
:ID: support-id
:END:
Support body.
"#,
    )?;
    sleep(Duration::from_millis(10));
    fs::write(
        root.join("focus.org"),
        r#"#+title: Focus

* TODO Focus
:PROPERTIES:
:ID: focus-id
:ROAM_REFS: cite:shared2024 cite:focus2024
:END:
SCHEDULED: <2026-05-01 Thu>
DEADLINE: <2026-05-03 Sat>
Links to [[id:neighbor-id][Neighbor]] and [[id:neighbor-id][Neighbor again]].

* Neighbor
:PROPERTIES:
:ID: neighbor-id
:END:
Neighbor body.
"#,
    )?;
    sleep(Duration::from_millis(10));
    fs::write(
        root.join("context.org"),
        r#"#+title: Context

* Source
:PROPERTIES:
:ID: source-id
:END:
Points to [[id:focus-id][Focus]] twice: [[id:focus-id][Focus]].

* Reflink Source
:PROPERTIES:
:ID: reflink-id
:END:
This mentions cite:shared2024 near Anonymous Focus.

* TODO Dual Match Peer
SCHEDULED: <2026-05-01 Thu>
DEADLINE: <2026-05-03 Sat>
Matches both planning fields directly.

* NEXT Cross Match Peer
SCHEDULED: <2026-05-03 Sat>
DEADLINE: <2026-05-01 Thu>
Matches both planning dates through opposite fields.

* TODO Keyword Only Peer
Shares only the same task state.

* WAIT Deadline Peer
DEADLINE: <2026-05-03 Sat>
Shares only the focus deadline.

* TODO Anonymous Focus
SCHEDULED: <2026-05-01 Thu>
DEADLINE: <2026-05-03 Sat>
:PROPERTIES:
:ROAM_REFS: cite:shared2024
:END:
Anonymous anchor body.
"#,
    )?;
    fs::write(
        root.join("related.org"),
        r#"#+title: Related

* TODO Unresolved Thread
:PROPERTIES:
:ID: unresolved-id
:ROAM_REFS: cite:shared2024
:END:
Unresolved body.

* Weak Thread
:PROPERTIES:
:ID: weak-id
:ROAM_REFS: cite:shared2024
:END:
Weakly integrated body.
"#,
    )?;

    let files = scan_root(&root)?;
    let db = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&db)?;
    database.sync_index(&files)?;
    let anonymous_anchor_key = database
        .anchors_in_file("context.org")?
        .into_iter()
        .find(|anchor| anchor.title == "Anonymous Focus")
        .map(|anchor| anchor.node_key)
        .expect("anonymous focus anchor should exist");

    Ok((
        workspace,
        root.display().to_string(),
        db.display().to_string(),
        anonymous_anchor_key,
    ))
}

fn explore_command(
    root: &str,
    db: &str,
    target_args: &[&str],
    extra_args: &[&str],
) -> Result<std::process::Output> {
    let mut args = vec![
        "explore",
        "--root",
        root,
        "--db",
        db,
        "--server-program",
        slipbox_binary(),
        "--json",
    ];
    args.extend_from_slice(target_args);
    args.extend_from_slice(extra_args);
    Ok(Command::new(slipbox_binary()).args(args).output()?)
}

#[test]
fn explore_command_supports_structure_lens_and_unique() -> Result<()> {
    let (_workspace, root, db, _) = build_indexed_fixture()?;

    let output = explore_command(&root, &db, &["--id", "focus-id"], &["--lens", "structure"])?;
    assert!(output.status.success(), "{output:?}");
    let parsed: ExploreResult = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        parsed
            .sections
            .iter()
            .map(|section| section.kind)
            .collect::<Vec<_>>(),
        vec![
            ExplorationSectionKind::Backlinks,
            ExplorationSectionKind::ForwardLinks,
        ]
    );
    assert_eq!(parsed.sections[0].entries.len(), 2);
    assert_eq!(parsed.sections[1].entries.len(), 2);

    let unique_output = explore_command(
        &root,
        &db,
        &["--id", "focus-id"],
        &["--lens", "structure", "--unique"],
    )?;
    assert!(unique_output.status.success(), "{unique_output:?}");
    let unique: ExploreResult = serde_json::from_slice(&unique_output.stdout)?;
    assert_eq!(unique.sections[0].entries.len(), 1);
    assert_eq!(unique.sections[1].entries.len(), 1);

    Ok(())
}

#[test]
fn explore_command_supports_refs_lens_with_explanations() -> Result<()> {
    let (_workspace, root, db, anonymous_anchor_key) = build_indexed_fixture()?;

    let output = explore_command(
        &root,
        &db,
        &["--key", &anonymous_anchor_key],
        &["--lens", "refs"],
    )?;
    assert!(output.status.success(), "{output:?}");
    let parsed: ExploreResult = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        parsed
            .sections
            .iter()
            .map(|section| section.kind)
            .collect::<Vec<_>>(),
        vec![
            ExplorationSectionKind::Reflinks,
            ExplorationSectionKind::UnlinkedReferences,
        ]
    );
    assert!(parsed.sections[0].entries.iter().any(|entry| matches!(
        entry,
        ExplorationEntry::Reflink { record }
        if record.source_anchor.title == "Reflink Source"
            && record.matched_reference == "cite:shared2024"
            && record.explanation == ExplorationExplanation::SharedReference {
                reference: "cite:shared2024".to_owned(),
            }
    )));
    assert!(parsed.sections[1].entries.iter().any(|entry| matches!(
        entry,
        ExplorationEntry::UnlinkedReference { record }
        if record.source_anchor.title == "Reflink Source"
            && record.matched_text == "Anonymous Focus"
            && record.explanation == ExplorationExplanation::UnlinkedReference {
                matched_text: "Anonymous Focus".to_owned(),
            }
    )));

    Ok(())
}

#[test]
fn explore_command_supports_time_lens_and_limit() -> Result<()> {
    let (_workspace, root, db, anonymous_anchor_key) = build_indexed_fixture()?;

    let output = explore_command(
        &root,
        &db,
        &["--key", &anonymous_anchor_key],
        &["--lens", "time", "--limit", "1"],
    )?;
    assert!(output.status.success(), "{output:?}");
    let parsed: ExploreResult = serde_json::from_slice(&output.stdout)?;
    assert_eq!(parsed.sections.len(), 1);
    assert_eq!(
        parsed.sections[0].kind,
        ExplorationSectionKind::TimeNeighbors
    );
    assert_eq!(parsed.sections[0].entries.len(), 1);
    assert!(matches!(
        &parsed.sections[0].entries[0],
        ExplorationEntry::Anchor { record } if record.anchor.title == "Dual Match Peer"
    ));

    Ok(())
}

#[test]
fn explore_command_supports_tasks_lens() -> Result<()> {
    let (_workspace, root, db, anonymous_anchor_key) = build_indexed_fixture()?;

    let output = explore_command(
        &root,
        &db,
        &["--key", &anonymous_anchor_key],
        &["--lens", "tasks"],
    )?;
    assert!(output.status.success(), "{output:?}");
    let parsed: ExploreResult = serde_json::from_slice(&output.stdout)?;
    assert_eq!(parsed.sections.len(), 1);
    assert_eq!(
        parsed.sections[0].kind,
        ExplorationSectionKind::TaskNeighbors
    );
    assert!(parsed.sections[0].entries.iter().any(|entry| matches!(
        entry,
        ExplorationEntry::Anchor { record }
            if record.anchor.title == "Dual Match Peer"
                && matches!(
                    record.explanation,
                    ExplorationExplanation::TaskNeighbor { ref shared_todo_keyword, ref planning_relations }
                    if shared_todo_keyword.as_deref() == Some("TODO") && !planning_relations.is_empty()
                )
    )));

    Ok(())
}

#[test]
fn explore_command_supports_bridges_lens() -> Result<()> {
    let (_workspace, root, db, _) = build_indexed_fixture()?;

    let output = explore_command(&root, &db, &["--id", "focus-id"], &["--lens", "bridges"])?;
    assert!(output.status.success(), "{output:?}");
    let parsed: ExploreResult = serde_json::from_slice(&output.stdout)?;
    assert_eq!(parsed.sections.len(), 1);
    assert_eq!(
        parsed.sections[0].kind,
        ExplorationSectionKind::BridgeCandidates
    );
    assert!(parsed.sections[0].entries.iter().any(|entry| matches!(
        entry,
        ExplorationEntry::Anchor { record }
            if record.anchor.title == "Dormant Bridge"
                && matches!(
                    record.explanation,
                    ExplorationExplanation::BridgeCandidate { ref references, ref via_notes }
                    if references == &vec!["@shared2024".to_owned()]
                        && via_notes.len() == 1
                        && via_notes[0].title == "Neighbor"
                )
    )));

    Ok(())
}

#[test]
fn explore_command_supports_dormant_lens() -> Result<()> {
    let (_workspace, root, db, _) = build_indexed_fixture()?;

    let output = explore_command(&root, &db, &["--id", "focus-id"], &["--lens", "dormant"])?;
    assert!(output.status.success(), "{output:?}");
    let parsed: ExploreResult = serde_json::from_slice(&output.stdout)?;
    assert_eq!(parsed.sections.len(), 1);
    assert_eq!(
        parsed.sections[0].kind,
        ExplorationSectionKind::DormantNotes
    );
    assert!(parsed.sections[0].entries.iter().any(|entry| matches!(
        entry,
        ExplorationEntry::Anchor { record }
            if record.anchor.title == "Dormant Bridge"
                && matches!(
                    record.explanation,
                    ExplorationExplanation::DormantSharedReference { ref references, .. }
                    if references == &vec!["@shared2024".to_owned()]
                )
    )));

    Ok(())
}

#[test]
fn explore_command_supports_unresolved_lens() -> Result<()> {
    let (_workspace, root, db, _) = build_indexed_fixture()?;

    let output = explore_command(&root, &db, &["--id", "focus-id"], &["--lens", "unresolved"])?;
    assert!(output.status.success(), "{output:?}");
    let parsed: ExploreResult = serde_json::from_slice(&output.stdout)?;
    assert_eq!(
        parsed
            .sections
            .iter()
            .map(|section| section.kind)
            .collect::<Vec<_>>(),
        vec![
            ExplorationSectionKind::UnresolvedTasks,
            ExplorationSectionKind::WeaklyIntegratedNotes,
        ]
    );
    assert!(parsed.sections[0].entries.iter().any(|entry| matches!(
        entry,
        ExplorationEntry::Anchor { record }
            if record.anchor.title == "Unresolved Thread"
                && record.explanation == ExplorationExplanation::UnresolvedSharedReference {
                    references: vec!["@shared2024".to_owned()],
                    todo_keyword: "TODO".to_owned(),
                }
    )));
    assert!(parsed.sections[1].entries.iter().any(|entry| matches!(
        entry,
        ExplorationEntry::Anchor { record }
            if record.anchor.title == "Weak Thread"
                && record.explanation == ExplorationExplanation::WeaklyIntegratedSharedReference {
                    references: vec!["@shared2024".to_owned()],
                    structural_link_count: 0,
                }
    )));

    Ok(())
}

#[test]
fn explore_command_rejects_unique_outside_structure() -> Result<()> {
    let (_workspace, root, db, _) = build_indexed_fixture()?;

    let output = explore_command(
        &root,
        &db,
        &["--id", "focus-id"],
        &["--lens", "refs", "--unique"],
    )?;

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let parsed: ErrorPayload = serde_json::from_slice(&output.stderr)?;
    assert!(
        parsed
            .error
            .message
            .contains("explore unique is only supported for the structure lens")
    );

    Ok(())
}

#[test]
fn explore_command_reports_unknown_targets() -> Result<()> {
    let (_workspace, root, db, _) = build_indexed_fixture()?;

    let output = explore_command(
        &root,
        &db,
        &["--id", "missing-id"],
        &["--lens", "structure"],
    )?;

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let parsed: ErrorPayload = serde_json::from_slice(&output.stderr)?;
    assert!(parsed.error.message.contains("unknown node id: missing-id"));

    Ok(())
}

#[test]
fn explore_command_prints_human_output_with_explanations() -> Result<()> {
    let (_workspace, root, db, _) = build_indexed_fixture()?;

    let output = Command::new(slipbox_binary())
        .args([
            "explore",
            "--root",
            &root,
            "--db",
            &db,
            "--server-program",
            slipbox_binary(),
            "--id",
            "focus-id",
            "--lens",
            "bridges",
        ])
        .output()?;

    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout)?;
    assert!(stdout.contains("lens: bridges"));
    assert!(stdout.contains("[bridge candidates]"));
    assert!(stdout.contains("Dormant Bridge"));
    assert!(stdout.contains("why: shared references @shared2024; via Neighbor ["));
    assert!(output.stderr.is_empty());

    Ok(())
}
