use std::{fs, path::Path, process::Command};

use crate::{workspace_diff, DiffLineKind, FileStatus};

#[test]
fn diff_counts_unstaged_change_against_index() {
    let temp = tempfile::tempdir().unwrap();
    git(temp.path(), ["init"]);
    fs::write(temp.path().join("tracked.txt"), "original\n").unwrap();
    git(temp.path(), ["add", "tracked.txt"]);
    fs::write(temp.path().join("tracked.txt"), "changed\n").unwrap();

    let diff = workspace_diff(temp.path()).unwrap();

    assert_eq!(diff.summary.files_changed, 1);
    assert_eq!(diff.summary.additions, 1);
    assert_eq!(diff.summary.deletions, 1);
    assert_eq!(diff.files[0].status, FileStatus::Modified);
    assert!(diff.files[0].unified_diff.contains("+changed"));
}

#[test]
fn modified_file_reports_structured_hunk_lines() {
    let temp = tempfile::tempdir().unwrap();
    git(temp.path(), ["init"]);
    fs::write(temp.path().join("tracked.txt"), "one\ntwo\nthree\n").unwrap();
    git(temp.path(), ["add", "tracked.txt"]);
    fs::write(temp.path().join("tracked.txt"), "one\nTWO\nthree\n").unwrap();

    let diff = workspace_diff(temp.path()).unwrap();
    let hunk = &diff.files[0].hunks[0];

    assert_eq!(hunk.old_start, 1);
    assert_eq!(hunk.new_start, 1);
    assert_eq!(hunk.old_lines, 3);
    assert_eq!(hunk.new_lines, 3);
    assert!(hunk
        .lines
        .iter()
        .any(|line| line.kind == DiffLineKind::Delete && line.content == "two\n"));
    assert!(hunk
        .lines
        .iter()
        .any(|line| line.kind == DiffLineKind::Insert && line.content == "TWO\n"));
}

#[test]
fn diff_reports_untracked_file_as_added() {
    let temp = tempfile::tempdir().unwrap();
    git(temp.path(), ["init"]);
    fs::write(temp.path().join("new.txt"), "hello\n").unwrap();

    let diff = workspace_diff(temp.path()).unwrap();

    assert_eq!(diff.files[0].status, FileStatus::Added);
    assert_eq!(diff.files[0].additions, 1);
    assert!(diff.changed_files_text().contains("A new.txt"));
}

#[test]
fn added_file_reports_insert_only_hunk() {
    let temp = tempfile::tempdir().unwrap();
    git(temp.path(), ["init"]);
    fs::write(temp.path().join("new.txt"), "hello\nworld\n").unwrap();

    let diff = workspace_diff(temp.path()).unwrap();
    let hunk = &diff.files[0].hunks[0];

    assert_eq!(hunk.old_start, 0);
    assert_eq!(hunk.old_lines, 0);
    assert_eq!(hunk.new_start, 1);
    assert_eq!(hunk.new_lines, 2);
    assert!(hunk
        .lines
        .iter()
        .all(|line| line.kind == DiffLineKind::Insert));
}

fn git<const N: usize>(root: &Path, args: [&str; N]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .status()
        .unwrap();
    assert!(status.success());
}
