use std::fs;

use anyhow::Result;
use slipbox_index::scan_root;
use slipbox_store::Database;
use tempfile::tempdir;

#[test]
fn indexes_planning_lines_and_queries_agenda_ranges() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;

    fs::write(
        root.join("with-times.org"),
        ":PROPERTIES:\n:ID: 9a20ca6c-5555-41c9-a039-ac38bf59c7a9\n:END:\n#+title: With Times\n\n* Scheduled heading\nSCHEDULED: <2024-07-16 Tue>\n:PROPERTIES:\n:ID: a523c198-4cb4-44d2-909c-a0e3258089cd\n:END:\n\n* Deadline heading\nDEADLINE: <2024-07-17 Tue>\n:PROPERTIES:\n:ID: 3ab84701-d1c1-463f-b5c6-715e6ff5a0bf\n:END:\n\n* DONE Full planning-line\nDEADLINE: <2024-07-17 Tue> CLOSED: <2024-07-17 Tue> SCHEDULED: <2024-07-17 Tue>\n:PROPERTIES:\n:ID: 52a56921-3e2b-46d6-9090-dfe6afcb8504\n:END:\n\n* With CLOSED but no DONE\nCLOSED: <2024-07-17 Tue>\n:PROPERTIES:\n:ID: 8d277b3c-b207-4326-8ead-c514e93c7f79\n:END:\n",
    )?;

    let files = scan_root(&root)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_index(&files)?;

    let scheduled = database
        .search_nodes("scheduled heading", 10)?
        .into_iter()
        .find(|node| node.title == "Scheduled heading")
        .expect("scheduled heading should exist");
    assert_eq!(
        scheduled.explicit_id.as_deref(),
        Some("a523c198-4cb4-44d2-909c-a0e3258089cd")
    );
    assert_eq!(
        scheduled.scheduled_for.as_deref(),
        Some("2024-07-16T00:00:00")
    );

    let full_planning = database
        .search_nodes("full planning", 10)?
        .into_iter()
        .find(|node| node.title == "Full planning-line")
        .expect("full planning heading should exist");
    assert_eq!(full_planning.todo_keyword.as_deref(), Some("DONE"));
    assert_eq!(
        full_planning.deadline_for.as_deref(),
        Some("2024-07-17T00:00:00")
    );
    assert_eq!(
        full_planning.closed_at.as_deref(),
        Some("2024-07-17T00:00:00")
    );

    let agenda = database.agenda_nodes("2024-07-17T00:00:00", "2024-07-17T23:59:59", 10)?;
    let agenda_titles = agenda
        .into_iter()
        .map(|node| node.title)
        .collect::<Vec<_>>();
    assert_eq!(
        agenda_titles,
        vec!["Deadline heading", "Full planning-line"]
    );

    Ok(())
}
