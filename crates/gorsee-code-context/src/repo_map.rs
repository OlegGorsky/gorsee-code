use std::{collections::BTreeMap, path::Path};

use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileSummary {
    pub path: String,
    pub language: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoMap {
    pub root: String,
    pub manifests: Vec<String>,
    pub language_counts: BTreeMap<String, usize>,
    pub files: Vec<FileSummary>,
}

pub fn build_repo_map(root: impl AsRef<Path>, max_files: usize) -> RepoMap {
    let root = root.as_ref();
    let mut files = Vec::new();
    let mut manifests = Vec::new();
    let mut language_counts = BTreeMap::new();

    for entry in WalkBuilder::new(root).hidden(false).build().flatten() {
        if !entry.file_type().is_some_and(|kind| kind.is_file()) {
            continue;
        }
        let relative = relative_path(root, entry.path());
        let language = language_for(entry.path());
        if is_manifest(&relative) {
            manifests.push(relative.clone());
        }
        *language_counts.entry(language.clone()).or_insert(0) += 1;
        if files.len() < max_files {
            files.push(FileSummary {
                path: relative,
                language,
            });
        }
    }

    RepoMap {
        root: root.display().to_string(),
        manifests,
        language_counts,
        files,
    }
}

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn is_manifest(path: &str) -> bool {
    matches!(
        path,
        "Cargo.toml" | "package.json" | "pyproject.toml" | "go.mod" | "deno.json" | "bun.lockb"
    )
}

fn language_for(path: &Path) -> String {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
    {
        "rs" => "rust",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" => "javascript",
        "py" => "python",
        "go" => "go",
        "toml" => "toml",
        "json" => "json",
        "md" => "markdown",
        _ => "other",
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn repo_map_finds_manifest_and_language_counts() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("Cargo.toml"), "[package]\nname='x'").unwrap();
        fs::create_dir(temp.path().join("src")).unwrap();
        fs::write(temp.path().join("src/lib.rs"), "").unwrap();

        let map = build_repo_map(temp.path(), 10);
        assert_eq!(map.manifests, ["Cargo.toml"]);
        assert_eq!(map.language_counts["rust"], 1);
    }
}
