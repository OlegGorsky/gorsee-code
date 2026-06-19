use std::{fs, path::Path};

use gorsee_code_artifacts::ArtifactRecord;

pub(crate) fn workspace_artifacts(workspace: &Path) -> Vec<ArtifactRecord> {
    let sessions_dir = workspace.join(".gorsee-code").join("sessions");
    let Ok(sessions) = fs::read_dir(sessions_dir) else {
        return Vec::new();
    };
    sessions
        .filter_map(|entry| entry.ok())
        .flat_map(|entry| artifact_files(entry.path().join("artifacts")))
        .collect()
}

fn artifact_files(dir: impl AsRef<Path>) -> Vec<ArtifactRecord> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .file_type()
                .map(|kind| kind.is_file())
                .unwrap_or(false)
        })
        .map(|entry| artifact_record(entry.path()))
        .collect()
}

fn artifact_record(path: impl AsRef<Path>) -> ArtifactRecord {
    let path = path.as_ref();
    let id = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("artifact")
        .to_string();
    ArtifactRecord {
        id,
        path: path.display().to_string(),
        mime: mime_for_path(path),
    }
}

fn mime_for_path(path: &Path) -> String {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("md") | Some("markdown") => "text/markdown",
        Some("json") => "application/json",
        Some("jsonl") => "application/x-ndjson",
        Some("patch") => "text/x-diff",
        _ => "text/plain",
    }
    .into()
}
