use std::{fs, path::PathBuf};

use gorsee_code_safety::{PathPolicy, RiskClass};
use gorsee_code_tool_runtime::{Tool, ToolManifest, ToolOutput, ToolRuntimeError};
use serde_json::{json, Value};

mod patch_unified;

pub struct ProposePatchTool {
    paths: PathPolicy,
}

pub struct ApplyPatchTool {
    paths: PathPolicy,
}

impl ProposePatchTool {
    pub fn new(paths: PathPolicy) -> Self {
        Self { paths }
    }
}

impl ApplyPatchTool {
    pub fn new(paths: PathPolicy) -> Self {
        Self { paths }
    }
}

impl Tool for ProposePatchTool {
    fn manifest(&self) -> ToolManifest {
        manifest(
            "propose_patch",
            "Validate and describe a proposed file patch",
            RiskClass::Read,
        )
    }

    fn run(&self, args: Value) -> Result<ToolOutput, ToolRuntimeError> {
        let files = patch_files(&args, &self.paths)?;
        for file in &files {
            self.paths.resolve_for_write(&file.path).map_err(handler)?;
        }
        let bytes = total_bytes(&files);
        let summary = format!("Would write {bytes} bytes to {} file(s)", files.len());
        Ok(ToolOutput {
            text: summary,
            json: Some(json!({ "files": files_json(&files), "bytes": bytes, "dry_run": true })),
            truncated: false,
        })
    }
}

impl Tool for ApplyPatchTool {
    fn manifest(&self) -> ToolManifest {
        manifest(
            "apply_patch",
            "Write approved file content inside the workspace",
            RiskClass::Write,
        )
    }

    fn run(&self, args: Value) -> Result<ToolOutput, ToolRuntimeError> {
        let files = patch_files(&args, &self.paths)?;
        let targets = files
            .iter()
            .map(|file| {
                self.paths
                    .resolve_for_write(&file.path)
                    .map(|target| (file, target))
                    .map_err(handler)
            })
            .collect::<Result<Vec<_>, _>>()?;
        for (file, target) in targets {
            write_file(target, &file.content)?;
        }
        let bytes = total_bytes(&files);
        Ok(ToolOutput::text(format!(
            "wrote {bytes} bytes to {} file(s)",
            files.len()
        )))
    }
}

#[derive(Debug, Clone)]
pub(super) struct PatchFile {
    pub(super) path: String,
    pub(super) content: String,
}

fn patch_files(args: &Value, paths: &PathPolicy) -> Result<Vec<PatchFile>, ToolRuntimeError> {
    if let Some(files) = args.get("files").and_then(Value::as_array) {
        return files.iter().map(patch_file).collect();
    }
    if let Some(patch) = args.get("patch").and_then(Value::as_str) {
        return patch_unified::files_from_unified_patch(patch, paths);
    }
    Ok(vec![patch_file(args)?])
}

fn patch_file(args: &Value) -> Result<PatchFile, ToolRuntimeError> {
    Ok(PatchFile {
        path: path_arg(args)?.to_string(),
        content: arg(args, "content")?.to_string(),
    })
}

fn path_arg(args: &Value) -> Result<&str, ToolRuntimeError> {
    args.get("path")
        .or_else(|| args.get("file_path"))
        .and_then(Value::as_str)
        .ok_or_else(|| handler("missing path"))
}

fn arg<'a>(args: &'a Value, name: &str) -> Result<&'a str, ToolRuntimeError> {
    args.get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| handler(format!("missing {name}")))
}

fn write_file(target: PathBuf, content: &str) -> Result<(), ToolRuntimeError> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(handler)?;
    }
    fs::write(&target, content).map_err(handler)
}

fn total_bytes(files: &[PatchFile]) -> usize {
    files.iter().map(|file| file.content.len()).sum()
}

fn files_json(files: &[PatchFile]) -> Value {
    Value::Array(
        files
            .iter()
            .map(|file| json!({ "path": file.path, "bytes": file.content.len() }))
            .collect(),
    )
}

fn manifest(name: &str, description: &str, risk: RiskClass) -> ToolManifest {
    ToolManifest {
        name: name.into(),
        description: description.into(),
        risk,
        capabilities: vec!["files:write".into(), "files:edit".into()],
    }
}

fn handler(error: impl std::fmt::Display) -> ToolRuntimeError {
    ToolRuntimeError::Handler(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patch_tools_advertise_write_and_edit_capabilities() {
        let temp = tempfile::tempdir().unwrap();
        let policy = PathPolicy::new(temp.path()).unwrap();

        let propose = ProposePatchTool::new(policy.clone()).manifest();
        let apply = ApplyPatchTool::new(policy).manifest();

        assert_eq!(propose.risk, RiskClass::Read);
        assert_eq!(apply.risk, RiskClass::Write);
        for manifest in [propose, apply] {
            assert!(manifest.capabilities.contains(&"files:write".into()));
            assert!(manifest.capabilities.contains(&"files:edit".into()));
        }
    }

    #[test]
    fn patch_tools_accept_file_path_alias_from_model_calls() {
        let temp = tempfile::tempdir().unwrap();
        let policy = PathPolicy::new(temp.path()).unwrap();

        let proposed = ProposePatchTool::new(policy.clone())
            .run(json!({ "file_path": "hello.txt", "content": "hello\n" }))
            .unwrap();
        ApplyPatchTool::new(policy)
            .run(json!({ "file_path": "hello.txt", "content": "hello\n" }))
            .unwrap();

        assert_eq!(proposed.json.unwrap()["files"][0]["path"], "hello.txt");
        assert_eq!(
            fs::read_to_string(temp.path().join("hello.txt")).unwrap(),
            "hello\n"
        );
    }

    #[test]
    fn patch_tools_accept_batch_files_from_model_calls() {
        let temp = tempfile::tempdir().unwrap();
        let policy = PathPolicy::new(temp.path()).unwrap();

        let args = json!({
            "files": [
                { "path": "hello.txt", "content": "hello\n" },
                { "file_path": "src/lib.rs", "content": "pub fn ok() {}\n" }
            ]
        });
        let proposed = ProposePatchTool::new(policy.clone())
            .run(args.clone())
            .unwrap();
        ApplyPatchTool::new(policy).run(args).unwrap();

        assert_eq!(proposed.json.unwrap()["files"][1]["path"], "src/lib.rs");
        assert_eq!(
            fs::read_to_string(temp.path().join("hello.txt")).unwrap(),
            "hello\n"
        );
        assert_eq!(
            fs::read_to_string(temp.path().join("src/lib.rs")).unwrap(),
            "pub fn ok() {}\n"
        );
    }

    #[test]
    fn apply_patch_accepts_unified_new_file_patch() {
        let temp = tempfile::tempdir().unwrap();
        let policy = PathPolicy::new(temp.path()).unwrap();

        ApplyPatchTool::new(policy)
            .run(json!({
                "patch": "--- /dev/null\n+++ b/hello.txt\n@@ -0,0 +1 @@\n+hello-from-gorsee\n"
            }))
            .unwrap();

        assert_eq!(
            fs::read_to_string(temp.path().join("hello.txt")).unwrap(),
            "hello-from-gorsee\n"
        );
    }

    #[test]
    fn apply_patch_accepts_unified_modify_patch() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("hello.txt"), "one\ntwo\nthree\n").unwrap();
        let policy = PathPolicy::new(temp.path()).unwrap();

        ApplyPatchTool::new(policy)
            .run(json!({
                "patch": "--- a/hello.txt\n+++ b/hello.txt\n@@ -1,3 +1,3 @@\n one\n-two\n+changed\n three\n"
            }))
            .unwrap();

        assert_eq!(
            fs::read_to_string(temp.path().join("hello.txt")).unwrap(),
            "one\nchanged\nthree\n"
        );
    }
}
