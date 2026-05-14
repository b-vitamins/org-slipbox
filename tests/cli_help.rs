use anyhow::Result;

mod support;

use support::run_slipbox;

fn help(args: &[&str]) -> Result<String> {
    let output = run_slipbox(&args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>())?;
    assert!(output.status.success(), "{output:?}");
    Ok(String::from_utf8(output.stdout)?)
}

fn assert_contains_all(output: &str, expected: &[&str]) {
    for needle in expected {
        assert!(
            output.contains(needle),
            "expected help to contain {needle:?}\n\n{output}"
        );
    }
}

#[test]
fn top_level_help_maps_taxonomy_without_repeated_transport_phrase() -> Result<()> {
    let output = help(&["--help"])?;

    assert_contains_all(
        &output,
        &[
            "Knowledge management using Org",
            "Command families:",
            "Notes:",
            "Relations:",
            "Exploration:",
            "Reviews:",
            "Assets:",
            "System:",
            "Use --json on daemon-backed commands for stable machine output",
        ],
    );
    assert!(!output.contains("canonical headless connection path"));
    Ok(())
}

#[test]
fn shared_daemon_options_are_clear_on_read_commands() -> Result<()> {
    let output = help(&["node", "search", "--help"])?;

    assert_contains_all(
        &output,
        &[
            "Search text matched against indexed note titles",
            "--root <ROOT>",
            "Org source root",
            "--db <DB>",
            "Derived SQLite index path",
            "--server-program <PATH>",
            "Defaults to the current binary",
            "--json",
            "stable JSON",
            "<QUERY>",
        ],
    );
    Ok(())
}

#[test]
fn family_help_pages_show_their_visible_long_descriptions() -> Result<()> {
    let cases: &[(&[&str], &[&str])] = &[
        (
            &["node", "--help"],
            &[
                "Inspect and mutate note records and anchor identity",
                "refresh affected index state before returning",
            ],
        ),
        (
            &["audit", "--help"],
            &[
                "Run corpus-health audits over the derived index",
                "Audits are read-only unless --save-review is set",
            ],
        ),
        (
            &["workflow", "--help"],
            &[
                "Discover, inspect, and run named workflows",
                "workflow show --spec",
                "does not require --root or --db",
            ],
        ),
        (
            &["pack", "--help"],
            &[
                "Manage declarative workbench packs",
                "pack validate",
                "local file/stdin inspection",
            ],
        ),
    ];

    for (args, expected) in cases {
        let output = help(args)?;
        assert_contains_all(&output, expected);
    }
    Ok(())
}

#[test]
fn write_command_help_documents_mutation_and_read_your_writes() -> Result<()> {
    let note = help(&["note", "--help"])?;
    assert_contains_all(
        &note,
        &[
            "Create file notes and append headings",
            "mutates Org files through the daemon",
            "follow-up reads observe the write",
        ],
    );

    let edit = help(&["edit", "--help"])?;
    assert_contains_all(
        &edit,
        &[
            "Move or rewrite Org structure",
            "changed/removed files",
            "stable StructuralWriteReport",
        ],
    );
    Ok(())
}

#[test]
fn local_inspection_and_report_output_help_are_explicit() -> Result<()> {
    let workflow_show = help(&["workflow", "show", "--help"])?;
    assert_contains_all(
        &workflow_show,
        &[
            "--spec <PATH>",
            "Local workflow spec JSON path",
            "Does not start the daemon",
        ],
    );

    let pack_validate = help(&["pack", "validate", "--help"])?;
    assert_contains_all(
        &pack_validate,
        &[
            "Read workbench pack JSON from this path",
            "Does not start the daemon",
            "--json",
            "stable JSON",
        ],
    );

    let workflow_run = help(&["workflow", "run", "--help"])?;
    assert_contains_all(
        &workflow_run,
        &[
            "--output <PATH>",
            "rendered report",
            "--jsonl",
            "JSON Lines report output",
            "--save-review",
        ],
    );
    Ok(())
}

#[test]
fn preview_apply_help_documents_confirmation_and_safety() -> Result<()> {
    let remediation = help(&["review", "remediation", "apply", "--help"])?;
    assert_contains_all(
        &remediation,
        &[
            "Apply one supported remediation action",
            "revalidates the saved preview",
            "--confirm-unlink-dangling-link",
            "Replacement text",
        ],
    );

    let rewrite = help(&["link", "rewrite-slipbox", "apply", "--help"])?;
    assert_contains_all(
        &rewrite,
        &[
            "Apply supported `slipbox:` link rewrites",
            "--confirm-replace-slipbox-links",
            "changed-file refresh status",
        ],
    );
    Ok(())
}
