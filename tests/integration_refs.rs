use std::fs;

use anyhow::Result;
use slipbox_index::scan_root;
use slipbox_store::Database;
use tempfile::tempdir;

#[test]
fn indexes_refs_and_resolves_nodes_from_them() -> Result<()> {
    let workspace = tempdir()?;
    let root = workspace.path().join("notes");
    fs::create_dir_all(&root)?;

    fs::write(
        root.join("paper.org"),
        ":PROPERTIES:\n:ID: 5b9a7400-f59c-4ef9-acbb-045b69af98f1\n:ROAM_REFS: \"http://site.net/docs/01. introduction - hello world.html\" cite:thrun2005 [cite:@smith2024; @doe2021]\n:END:\n#+title: Probabilistic Robotics\n",
    )?;

    let files = scan_root(&root)?;
    let database_path = workspace.path().join("slipbox.sqlite");
    let mut database = Database::open(&database_path)?;
    database.sync_index(&files)?;

    let node = database
        .search_nodes("thrun2005", 10)?
        .into_iter()
        .find(|candidate| candidate.title == "Probabilistic Robotics")
        .expect("ref search should find the paper node");
    assert_eq!(
        node.refs,
        vec![
            "http://site.net/docs/01. introduction - hello world.html",
            "@thrun2005",
            "@smith2024",
            "@doe2021",
        ]
    );

    let refs = database.search_refs("smith", 10)?;
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].reference, "@smith2024");
    assert_eq!(refs[0].node.title, "Probabilistic Robotics");

    let from_cite = database
        .node_from_ref("cite:thrun2005")?
        .expect("cite ref should resolve");
    assert_eq!(from_cite.title, "Probabilistic Robotics");

    let from_org_cite = database
        .node_from_ref("[cite:@smith2024]")?
        .expect("org-cite ref should resolve");
    assert_eq!(from_org_cite.title, "Probabilistic Robotics");

    let from_url = database
        .node_from_ref("http://site.net/docs/01. introduction - hello world.html")?
        .expect("URL ref should resolve");
    assert_eq!(from_url.title, "Probabilistic Robotics");

    Ok(())
}
