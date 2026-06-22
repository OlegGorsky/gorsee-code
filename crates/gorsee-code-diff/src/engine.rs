use std::{
    fs,
    path::{Path, PathBuf},
};

use git2::{Oid, Repository, Status, StatusOptions};
use similar::{ChangeTag, TextDiff};
use thiserror::Error;

use crate::model::{
    DiffHunk, DiffLine, DiffLineKind, DiffSummary, FileDiff, FileStatus, WorkspaceDiff,
};

#[derive(Debug, Error)]
pub enum DiffError {
    #[error("git error: {0}")]
    Git(#[from] git2::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("repository has no working directory")]
    NoWorkdir,
}

pub fn workspace_diff(root: impl AsRef<Path>) -> Result<WorkspaceDiff, DiffError> {
    let repo = Repository::discover(root)?;
    let mut options = StatusOptions::new();
    options
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .renames_head_to_index(true)
        .renames_index_to_workdir(true);
    let statuses = repo.statuses(Some(&mut options))?;
    let mut files = Vec::new();
    for entry in statuses.iter() {
        let Ok(path) = entry.path() else {
            continue;
        };
        if path.starts_with(".gorsee-code/") {
            continue;
        }
        files.push(file_diff(&repo, Path::new(path), entry.status())?);
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    let summary = diff_summary(&files);
    Ok(WorkspaceDiff { files, summary })
}

pub fn changed_files(root: impl AsRef<Path>) -> Result<Vec<String>, DiffError> {
    Ok(workspace_diff(root)?
        .files
        .into_iter()
        .map(|file| file.path)
        .collect())
}

fn file_diff(repo: &Repository, path: &Path, status: Status) -> Result<FileDiff, DiffError> {
    let status_kind = file_status(status);
    let (old_text, new_text) = file_versions(repo, path, status, status_kind)?;
    let diff = TextDiff::from_lines(&old_text, &new_text);
    let hunks = diff_hunks(&diff);
    let (additions, deletions) = change_counts(&hunks);
    let path_label = path.to_string_lossy().to_string();
    let unified_diff = diff
        .unified_diff()
        .header(&format!("a/{path_label}"), &format!("b/{path_label}"))
        .to_string();
    Ok(FileDiff {
        path: path_label,
        status: status_kind,
        additions,
        deletions,
        hunks,
        unified_diff,
    })
}

fn diff_hunks(diff: &TextDiff<'_, '_, '_, str>) -> Vec<DiffHunk> {
    diff.grouped_ops(3)
        .into_iter()
        .filter_map(|group| {
            let lines = group
                .iter()
                .flat_map(|op| diff.iter_changes(op))
                .map(|change| DiffLine {
                    kind: line_kind(change.tag()),
                    old_lineno: one_based(change.old_index()),
                    new_lineno: one_based(change.new_index()),
                    content: change.value().to_string(),
                })
                .collect::<Vec<_>>();
            (!lines.is_empty()).then(|| hunk(lines))
        })
        .collect()
}

fn change_counts(hunks: &[DiffHunk]) -> (usize, usize) {
    let additions = hunks
        .iter()
        .flat_map(|hunk| &hunk.lines)
        .filter(|line| line.kind == DiffLineKind::Insert)
        .count();
    let deletions = hunks
        .iter()
        .flat_map(|hunk| &hunk.lines)
        .filter(|line| line.kind == DiffLineKind::Delete)
        .count();
    (additions, deletions)
}

fn hunk(lines: Vec<DiffLine>) -> DiffHunk {
    let old_start = lines
        .iter()
        .filter_map(|line| line.old_lineno)
        .min()
        .unwrap_or(0);
    let new_start = lines
        .iter()
        .filter_map(|line| line.new_lineno)
        .min()
        .unwrap_or(0);
    let old_lines = lines
        .iter()
        .filter(|line| line.old_lineno.is_some())
        .count();
    let new_lines = lines
        .iter()
        .filter(|line| line.new_lineno.is_some())
        .count();
    DiffHunk {
        old_start,
        old_lines,
        new_start,
        new_lines,
        lines,
    }
}

fn one_based(index: Option<usize>) -> Option<usize> {
    index.map(|index| index + 1)
}

fn line_kind(tag: ChangeTag) -> DiffLineKind {
    match tag {
        ChangeTag::Equal => DiffLineKind::Context,
        ChangeTag::Delete => DiffLineKind::Delete,
        ChangeTag::Insert => DiffLineKind::Insert,
    }
}

fn file_versions(
    repo: &Repository,
    path: &Path,
    status: Status,
    status_kind: FileStatus,
) -> Result<(String, String), DiffError> {
    if has_worktree_change(status) {
        let old_text = index_text(repo, path)
            .or_else(|| head_text(repo, path))
            .unwrap_or_default();
        let new_text = if status_kind == FileStatus::Deleted {
            String::new()
        } else {
            worktree_text(repo, path)?
        };
        return Ok((old_text, new_text));
    }
    let old_text = head_text(repo, path).unwrap_or_default();
    let new_text = index_text(repo, path).unwrap_or_default();
    Ok((old_text, new_text))
}

fn has_worktree_change(status: Status) -> bool {
    status.intersects(
        Status::WT_NEW
            | Status::WT_MODIFIED
            | Status::WT_DELETED
            | Status::WT_TYPECHANGE
            | Status::WT_RENAMED,
    )
}

fn file_status(status: Status) -> FileStatus {
    if status.intersects(Status::WT_DELETED | Status::INDEX_DELETED) {
        FileStatus::Deleted
    } else if status.intersects(Status::WT_RENAMED | Status::INDEX_RENAMED) {
        FileStatus::Renamed
    } else if status.intersects(Status::WT_TYPECHANGE | Status::INDEX_TYPECHANGE) {
        FileStatus::TypeChanged
    } else if status.intersects(Status::WT_MODIFIED | Status::INDEX_MODIFIED) {
        FileStatus::Modified
    } else if status.intersects(Status::WT_NEW | Status::INDEX_NEW) {
        FileStatus::Added
    } else {
        FileStatus::Unknown
    }
}

fn diff_summary(files: &[FileDiff]) -> DiffSummary {
    DiffSummary {
        files_changed: files.len(),
        additions: files.iter().map(|file| file.additions).sum(),
        deletions: files.iter().map(|file| file.deletions).sum(),
    }
}

fn worktree_text(repo: &Repository, path: &Path) -> Result<String, DiffError> {
    let workdir = repo.workdir().ok_or(DiffError::NoWorkdir)?;
    Ok(read_lossy(workdir.join(path))?)
}

fn index_text(repo: &Repository, path: &Path) -> Option<String> {
    let index = repo.index().ok()?;
    let entry = index.get_path(path, 0)?;
    blob_text(repo, entry.id)
}

fn head_text(repo: &Repository, path: &Path) -> Option<String> {
    let tree = repo.head().ok()?.peel_to_tree().ok()?;
    let entry = tree.get_path(path).ok()?;
    blob_text(repo, entry.id())
}

fn blob_text(repo: &Repository, oid: Oid) -> Option<String> {
    let blob = repo.find_blob(oid).ok()?;
    Some(String::from_utf8_lossy(blob.content()).to_string())
}

fn read_lossy(path: PathBuf) -> Result<String, std::io::Error> {
    fs::read(path).map(|bytes| String::from_utf8_lossy(&bytes).to_string())
}
