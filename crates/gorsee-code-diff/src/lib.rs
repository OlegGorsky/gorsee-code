mod engine;
mod model;

#[cfg(test)]
mod tests;

pub use engine::{changed_files, workspace_diff, DiffError};
pub use model::{
    DiffHunk, DiffLine, DiffLineKind, DiffSummary, FileDiff, FileStatus, WorkspaceDiff,
};
