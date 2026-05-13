use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Result;
use serde_json::Value;
use slipbox_index::scan_root;
use slipbox_store::Database;
use tempfile::{TempDir, tempdir};

fn slipbox_binary() -> &'static str {
    env!("CARGO_BIN_EXE_slipbox")
}

struct ContractFixture {
    _workspace: TempDir,
    root: PathBuf,
    db: PathBuf,
}

fn build_contract_fixture() -> Result<ContractFixture> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;
    fs::write(
        root.join("main.org"),
        r#"#+title: Main

* Extract Contract
:PROPERTIES:
:ID: extract-contract-id
:END:
Extract contract body phrase.

* Target Contract
:PROPERTIES:
:ID: target-contract-id
:END:
Target body.
"#,
    )?;
    fs::write(
        root.join("link-source.org"),
        "#+title: Link Source\n\nSee [[slipbox:Link Target][Target label]].\n",
    )?;
    fs::write(
        root.join("link-target.org"),
        "#+title: Link Target\n\nLink target body.\n",
    )?;
    fs::write(
        root.join("broken-link.org"),
        "#+title: Broken Link\n\nSee [[slipbox:Missing Target][Missing]].\n",
    )?;
    fs::write(
        root.join("dangling.org"),
        r#":PROPERTIES:
:ID: dangling-source-id
:END:
#+title: Dangling Source

Points to [[id:missing-id][Missing]].
"#,
    )?;
    fs::write(root.join("stale.org"), "#+title: Stale\n")?;

    let db = workspace.path().join("slipbox.sqlite");
    let files = scan_root(&root)?;
    let mut database = Database::open(&db)?;
    database.sync_index(&files)?;
    fs::remove_file(root.join("stale.org"))?;
    fs::write(root.join("new.org"), "#+title: New\n")?;

    Ok(ContractFixture {
        _workspace: workspace,
        root,
        db,
    })
}

fn root_string(path: &Path) -> String {
    path.to_str().expect("test path should be utf-8").to_owned()
}

fn base_args(root: &Path, db: &Path) -> Vec<String> {
    vec![
        "--root".to_owned(),
        root_string(root),
        "--db".to_owned(),
        root_string(db),
        "--server-program".to_owned(),
        slipbox_binary().to_owned(),
        "--json".to_owned(),
    ]
}

fn run_command(args: &[String]) -> Result<std::process::Output> {
    Ok(Command::new(slipbox_binary()).args(args).output()?)
}

fn json_command_path(
    command_path: &[&str],
    root: &Path,
    db: &Path,
    extra: &[&str],
) -> Result<std::process::Output> {
    let mut args = command_path
        .iter()
        .map(|word| (*word).to_owned())
        .collect::<Vec<_>>();
    args.extend(base_args(root, db));
    args.extend(extra.iter().map(|value| (*value).to_owned()));
    run_command(&args)
}

fn sorted_keys(value: &Value) -> Vec<String> {
    let object = value.as_object().expect("expected JSON object");
    let mut keys: Vec<String> = object.keys().cloned().collect();
    keys.sort();
    keys
}

fn assert_exact_object_keys(value: &Value, expected: &[&str]) {
    let mut expected_keys = expected
        .iter()
        .map(|key| (*key).to_owned())
        .collect::<Vec<_>>();
    expected_keys.sort();
    assert_eq!(sorted_keys(value), expected_keys);
}

fn assert_success_json(output: std::process::Output) -> Result<Value> {
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn assert_error_failure(output: &std::process::Output, needle: &str) {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    let parsed: Value =
        serde_json::from_slice(&output.stderr).expect("stderr should be structured JSON");
    assert_exact_object_keys(&parsed, &["error"]);
    let message = parsed["error"]["message"]
        .as_str()
        .expect("error message should be a string");
    assert!(message.contains(needle), "{message}");
}

fn assert_node_record_keys(value: &Value) {
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

fn assert_file_record_keys(value: &Value) {
    assert_exact_object_keys(value, &["file_path", "title", "mtime_ns", "node_count"]);
}

fn assert_occurrence_record_keys(value: &Value) {
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

fn assert_structural_report_keys(value: &Value) {
    assert_exact_object_keys(
        value,
        &[
            "operation",
            "changed_files",
            "removed_files",
            "index_refresh",
            "result",
        ],
    );
    if !value["result"].is_null() {
        let result = &value["result"];
        match result["kind"].as_str() {
            Some("node") => {
                assert_exact_object_keys(result, &["kind", "node"]);
                assert_node_record_keys(&result["node"]);
            }
            Some("anchor") => {
                assert_exact_object_keys(result, &["kind", "anchor"]);
                assert_node_record_keys(&result["anchor"]);
            }
            other => panic!("unexpected structural result kind: {other:?}"),
        }
    }
}

fn assert_link_preview_keys(value: &Value) {
    assert_exact_object_keys(value, &["preview"]);
    let preview = &value["preview"];
    assert_exact_object_keys(preview, &["file_path", "rewrites"]);
    let entry = &preview["rewrites"][0];
    assert_exact_object_keys(
        entry,
        &[
            "line",
            "column",
            "preview",
            "link_text",
            "title_or_alias",
            "description",
            "target",
            "target_explicit_id",
            "replacement",
        ],
    );
    assert_node_record_keys(&entry["target"]);
}

fn assert_link_application_keys(value: &Value) {
    assert_exact_object_keys(value, &["application"]);
    let application = &value["application"];
    assert_exact_object_keys(
        application,
        &[
            "file_path",
            "rewrites",
            "changed_files",
            "removed_files",
            "index_refresh",
        ],
    );
    let entry = &application["rewrites"][0];
    assert_exact_object_keys(
        entry,
        &[
            "line",
            "column",
            "title_or_alias",
            "target_node_key",
            "target_explicit_id",
            "replacement",
        ],
    );
}

fn assert_file_diagnostic_keys(value: &Value) {
    assert_exact_object_keys(
        value,
        &[
            "file_path",
            "absolute_path",
            "exists",
            "eligible",
            "indexed",
            "index_record",
            "issues",
        ],
    );
    if !value["index_record"].is_null() {
        assert_file_record_keys(&value["index_record"]);
    }
}

fn assert_diagnostics_keys(value: &Value, kind: &str) {
    assert_exact_object_keys(value, &["diagnostic"]);
    let diagnostic = &value["diagnostic"];
    match kind {
        "file" => assert_file_diagnostic_keys(diagnostic),
        "node" => {
            assert_exact_object_keys(diagnostic, &["node", "file", "line_present", "issues"]);
            assert_node_record_keys(&diagnostic["node"]);
            assert_file_diagnostic_keys(&diagnostic["file"]);
        }
        "index" => {
            assert_exact_object_keys(
                diagnostic,
                &[
                    "root",
                    "eligible_files",
                    "indexed_files",
                    "missing_from_index",
                    "indexed_but_missing",
                    "indexed_but_ineligible",
                    "status",
                    "status_consistent",
                    "index_current",
                ],
            );
            assert_exact_object_keys(
                &diagnostic["status"],
                &["files_indexed", "nodes_indexed", "links_indexed"],
            );
        }
        other => panic!("unexpected diagnostics kind: {other}"),
    }
}

fn assert_remediation_preview_keys(value: &Value) {
    assert_exact_object_keys(value, &["preview"]);
    let preview = &value["preview"];
    assert_exact_object_keys(
        preview,
        &[
            "review_id",
            "finding_id",
            "status",
            "preview_identity",
            "kind",
            "source",
            "missing_explicit_id",
            "file_path",
            "line",
            "column",
            "preview",
            "suggestion",
            "confidence",
            "reason",
        ],
    );
    assert_node_record_keys(&preview["source"]);
    assert_exact_object_keys(
        &preview["preview_identity"],
        &[
            "kind",
            "source_node_key",
            "missing_explicit_id",
            "file_path",
            "line",
            "column",
            "preview",
        ],
    );
}

fn assert_remediation_application_keys(value: &Value) {
    assert_exact_object_keys(value, &["application"]);
    let application = &value["application"];
    assert_exact_object_keys(
        application,
        &[
            "review_id",
            "finding_id",
            "preview_identity",
            "action",
            "changed_files",
            "removed_files",
            "index_refresh",
        ],
    );
    assert_exact_object_keys(
        &application["preview_identity"],
        &[
            "kind",
            "source_node_key",
            "missing_explicit_id",
            "file_path",
            "line",
            "column",
            "preview",
        ],
    );
    assert_exact_object_keys(
        &application["action"],
        &[
            "kind",
            "source_node_key",
            "missing_explicit_id",
            "file_path",
            "line",
            "column",
            "preview",
            "replacement_text",
        ],
    );
}

fn assert_audit_review_keys(value: &Value) {
    assert_exact_object_keys(
        value,
        &[
            "review_id",
            "title",
            "summary",
            "kind",
            "audit",
            "limit",
            "findings",
        ],
    );
}

fn save_dangling_review(fixture: &ContractFixture, review_id: &str) -> Result<String> {
    let saved = json_command_path(
        &["audit", "dangling-links"],
        &fixture.root,
        &fixture.db,
        &["--save-review", "--review-id", review_id],
    )?;
    assert!(saved.status.success(), "{saved:?}");

    let shown = assert_success_json(json_command_path(
        &["review", "show"],
        &fixture.root,
        &fixture.db,
        &[review_id],
    )?)?;
    assert_exact_object_keys(&shown, &["review"]);
    assert_audit_review_keys(&shown["review"]);
    Ok(shown["review"]["findings"][0]["finding_id"]
        .as_str()
        .expect("finding id should be a string")
        .to_owned())
}

#[test]
fn structural_link_and_diagnostics_json_contracts_round_trip() -> Result<()> {
    let fixture = build_contract_fixture()?;

    let extracted = assert_success_json(json_command_path(
        &["edit", "extract-subtree"],
        &fixture.root,
        &fixture.db,
        &[
            "--source-id",
            "extract-contract-id",
            "--file",
            "extracted-contract.org",
        ],
    )?)?;
    assert_structural_report_keys(&extracted);
    assert_eq!(extracted["operation"], "extract-subtree");
    assert_eq!(extracted["result"]["kind"], "node");
    assert_eq!(
        extracted["result"]["node"]["file_path"],
        "extracted-contract.org"
    );

    let extracted_node = assert_success_json(json_command_path(
        &["node", "show"],
        &fixture.root,
        &fixture.db,
        &["--id", "extract-contract-id"],
    )?)?;
    assert_node_record_keys(&extracted_node);
    assert_eq!(extracted_node["file_path"], "extracted-contract.org");

    let files = assert_success_json(json_command_path(
        &["file", "list"],
        &fixture.root,
        &fixture.db,
        &[],
    )?)?;
    assert_exact_object_keys(&files, &["files"]);
    assert!(
        files["files"]
            .as_array()
            .expect("files should be an array")
            .iter()
            .any(|file| file == "extracted-contract.org")
    );

    let occurrence = assert_success_json(json_command_path(
        &["search", "occurrences"],
        &fixture.root,
        &fixture.db,
        &["Extract contract body phrase"],
    )?)?;
    assert_exact_object_keys(&occurrence, &["occurrences"]);
    assert_occurrence_record_keys(&occurrence["occurrences"][0]);

    let link_preview = assert_success_json(json_command_path(
        &["link", "rewrite-slipbox", "preview"],
        &fixture.root,
        &fixture.db,
        &["--file", "link-source.org"],
    )?)?;
    assert_link_preview_keys(&link_preview);

    let link_apply = assert_success_json(json_command_path(
        &["link", "rewrite-slipbox", "apply"],
        &fixture.root,
        &fixture.db,
        &[
            "--file",
            "link-source.org",
            "--confirm-replace-slipbox-links",
        ],
    )?)?;
    assert_link_application_keys(&link_apply);

    let file_diagnostic = assert_success_json(json_command_path(
        &["diagnose", "file"],
        &fixture.root,
        &fixture.db,
        &["--file", "new.org"],
    )?)?;
    assert_diagnostics_keys(&file_diagnostic, "file");
    assert_eq!(
        file_diagnostic["diagnostic"]["issues"][0],
        "missing-from-index"
    );

    let node_diagnostic = assert_success_json(json_command_path(
        &["diagnose", "node"],
        &fixture.root,
        &fixture.db,
        &["--id", "dangling-source-id"],
    )?)?;
    assert_diagnostics_keys(&node_diagnostic, "node");

    let index_diagnostic = assert_success_json(json_command_path(
        &["diagnose", "index"],
        &fixture.root,
        &fixture.db,
        &[],
    )?)?;
    assert_diagnostics_keys(&index_diagnostic, "index");
    assert!(
        index_diagnostic["diagnostic"]["missing_from_index"]
            .as_array()
            .expect("missing_from_index should be an array")
            .iter()
            .any(|file| file == "new.org")
    );

    Ok(())
}

#[test]
fn remediation_json_contracts_round_trip_through_review_file_and_search() -> Result<()> {
    let fixture = build_contract_fixture()?;
    let review_id = "review/audit/contracts/remediation";
    let finding_id = save_dangling_review(&fixture, review_id)?;

    let preview = assert_success_json(json_command_path(
        &["review", "remediation", "preview"],
        &fixture.root,
        &fixture.db,
        &[review_id, &finding_id],
    )?)?;
    assert_remediation_preview_keys(&preview);

    let apply = assert_success_json(json_command_path(
        &["review", "remediation", "apply"],
        &fixture.root,
        &fixture.db,
        &[review_id, &finding_id, "--confirm-unlink-dangling-link"],
    )?)?;
    assert_remediation_application_keys(&apply);

    let shown_review = assert_success_json(json_command_path(
        &["review", "show"],
        &fixture.root,
        &fixture.db,
        &[review_id],
    )?)?;
    assert_exact_object_keys(&shown_review, &["review"]);
    assert_audit_review_keys(&shown_review["review"]);

    let file_search = assert_success_json(json_command_path(
        &["file", "search"],
        &fixture.root,
        &fixture.db,
        &["Dangling"],
    )?)?;
    assert_exact_object_keys(&file_search, &["files"]);
    assert_file_record_keys(&file_search["files"][0]);

    let occurrence = assert_success_json(json_command_path(
        &["search", "occurrences"],
        &fixture.root,
        &fixture.db,
        &["Points to Missing"],
    )?)?;
    assert_exact_object_keys(&occurrence, &["occurrences"]);
    assert_occurrence_record_keys(&occurrence["occurrences"][0]);

    Ok(())
}

#[test]
fn structural_remediation_link_and_diagnostics_failures_are_structured() -> Result<()> {
    let fixture = build_contract_fixture()?;

    let invalid_target = json_command_path(
        &["edit", "refile-subtree"],
        &fixture.root,
        &fixture.db,
        &[
            "--source-id",
            "missing-source-id",
            "--target-id",
            "target-contract-id",
        ],
    )?;
    assert_error_failure(&invalid_target, "unknown node id: missing-source-id");

    let bad_range = json_command_path(
        &["edit", "refile-region"],
        &fixture.root,
        &fixture.db,
        &[
            "--file",
            "main.org",
            "--start",
            "1",
            "--end",
            "1",
            "--target-id",
            "target-contract-id",
        ],
    )?;
    assert_error_failure(&bad_range, "active region range must not be empty");

    let unsafe_edit_path = json_command_path(
        &["edit", "demote-file"],
        &fixture.root,
        &fixture.db,
        &["--file", "../outside.org"],
    )?;
    assert_error_failure(&unsafe_edit_path, "edit file path must stay within --root");

    let unsafe_diagnostic_path = json_command_path(
        &["diagnose", "file"],
        &fixture.root,
        &fixture.db,
        &["--file", "../outside.org"],
    )?;
    assert_error_failure(
        &unsafe_diagnostic_path,
        "diagnostic file path must stay within --root",
    );

    let unresolved_link = json_command_path(
        &["link", "rewrite-slipbox", "preview"],
        &fixture.root,
        &fixture.db,
        &["--file", "broken-link.org"],
    )?;
    assert_error_failure(
        &unresolved_link,
        "unresolved slipbox link target Missing Target",
    );

    let orphan_review_id = "review/audit/contracts/orphans";
    let orphan_review = assert_success_json(json_command_path(
        &["audit", "orphan-notes"],
        &fixture.root,
        &fixture.db,
        &["--save-review", "--review-id", orphan_review_id],
    )?)?;
    let orphan_finding_id = orphan_review["review"]["review_id"]
        .as_str()
        .expect("saved review summary should include review_id");
    assert_eq!(orphan_finding_id, orphan_review_id);
    let orphan_show = assert_success_json(json_command_path(
        &["review", "show"],
        &fixture.root,
        &fixture.db,
        &[orphan_review_id],
    )?)?;
    let unsupported_finding = orphan_show["review"]["findings"][0]["finding_id"]
        .as_str()
        .expect("orphan finding id should be a string");
    let unsupported_remediation = json_command_path(
        &["review", "remediation", "preview"],
        &fixture.root,
        &fixture.db,
        &[orphan_review_id, unsupported_finding],
    )?;
    assert_error_failure(&unsupported_remediation, "orphan-note evidence");

    let review_id = "review/audit/contracts/stale";
    let finding_id = save_dangling_review(&fixture, review_id)?;
    let source_path = fixture.root.join("dangling.org");
    let stale_source = fs::read_to_string(&source_path)?
        .replace("[[id:missing-id][Missing]]", "[[id:missing-id][Stale]]");
    fs::write(source_path, stale_source)?;
    let stale_apply = json_command_path(
        &["review", "remediation", "apply"],
        &fixture.root,
        &fixture.db,
        &[review_id, &finding_id, "--confirm-unlink-dangling-link"],
    )?;
    assert_error_failure(
        &stale_apply,
        "remediation action no longer matches file contents",
    );

    Ok(())
}
