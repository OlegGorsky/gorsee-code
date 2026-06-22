pub mod command;
pub mod files;
pub mod final_answer;
pub mod git;
pub mod mcp;
pub mod patch;
pub mod repo;
pub mod test_runner;

use std::path::Path;

use gorsee_code_safety::{OutputBounds, PathPolicy, PathPolicyError, PermissionPolicy, Redactor};
use gorsee_code_tool_runtime::ToolRegistry;

pub fn builtin_registry(root: impl AsRef<Path>) -> Result<ToolRegistry, PathPolicyError> {
    let policy = PermissionPolicy::balanced();
    let redactor = Redactor::default();
    let bounds = OutputBounds::default();
    let path_policy = PathPolicy::new(root.as_ref())?;
    let root = path_policy.root().to_path_buf();
    let mut registry = ToolRegistry::new(policy, bounds, redactor);

    registry.register(files::ListFilesTool::new(root.clone()));
    registry.register(files::ReadFileTool::new(path_policy.clone()));
    registry.register(files::SearchTextTool::new(path_policy.clone()));
    registry.register(repo::RepoMapTool::new(root.clone()));
    registry.register(git::GitDiffTool::new(root.clone()));
    registry.register(git::GitChangedFilesTool::new(root.clone()));
    registry.register(git::GitStatusTool::new(root.clone()));
    registry.register(git::GitRecentFilesTool::new(root.clone()));
    registry.register(mcp::McpInventoryTool::new(root.clone()));
    registry.register(mcp::McpCallTool::new(root.clone()));
    mcp::register_server_tools(&mut registry, root.clone());
    registry.register(patch::ProposePatchTool::new(path_policy.clone()));
    registry.register(patch::ApplyPatchTool::new(path_policy));
    registry.register(command::RunCommandTool::new(root.clone()));
    registry.register(test_runner::RunTestTool::new(root));
    registry.register(final_answer::FinalAnswerTool);

    Ok(registry)
}
