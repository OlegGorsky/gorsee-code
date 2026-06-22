use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceDiff {
    pub files: Vec<FileDiff>,
    pub summary: DiffSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffSummary {
    pub files_changed: usize,
    pub additions: usize,
    pub deletions: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileDiff {
    pub path: String,
    pub status: FileStatus,
    pub additions: usize,
    pub deletions: usize,
    pub hunks: Vec<DiffHunk>,
    pub unified_diff: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffHunk {
    pub old_start: usize,
    pub old_lines: usize,
    pub new_start: usize,
    pub new_lines: usize,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub old_lineno: Option<usize>,
    pub new_lineno: Option<usize>,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiffLineKind {
    Context,
    Delete,
    Insert,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    TypeChanged,
    Unknown,
}

impl WorkspaceDiff {
    pub fn is_clean(&self) -> bool {
        self.files.is_empty()
    }

    pub fn render_summary_text(&self) -> String {
        if self.is_clean() {
            return "clean\n".into();
        }
        let mut out = format!(
            "files_changed={} additions={} deletions={}\n",
            self.summary.files_changed, self.summary.additions, self.summary.deletions
        );
        for file in &self.files {
            out.push_str(&format!(
                "{} {} +{} -{}\n",
                file.status.code(),
                file.path,
                file.additions,
                file.deletions
            ));
        }
        out.push('\n');
        for file in &self.files {
            if !file.unified_diff.trim().is_empty() {
                out.push_str(&file.unified_diff);
                if !out.ends_with('\n') {
                    out.push('\n');
                }
            }
        }
        out
    }

    pub fn changed_files_text(&self) -> String {
        self.files
            .iter()
            .map(|file| format!("{} {}", file.status.code(), file.path))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl FileStatus {
    pub fn code(self) -> &'static str {
        match self {
            Self::Added => "A",
            Self::Modified => "M",
            Self::Deleted => "D",
            Self::Renamed => "R",
            Self::TypeChanged => "T",
            Self::Unknown => "?",
        }
    }
}
