use std::{fs, path::Path};

use chrono::{DateTime, Utc};
use gorsee_code_artifacts::ArtifactRecord;
use gorsee_code_core::ModelCapability;
use gorsee_code_hooks::{builtin_hooks, HookDefinition};
use gorsee_code_limits::UsageWindow;
use gorsee_code_skills::{builtin_skills, Skill};
use gorsee_code_tool_runtime::ToolManifest;
use gorsee_code_tools::builtin_registry;
use gorsee_code_ui_state::{workspace_state, BudgetView, MissionControlState, SessionView};

#[derive(Debug, Clone)]
pub struct GatewayState {
    pub started_at: DateTime<Utc>,
    pub mission: MissionControlState,
    pub capabilities: Vec<ModelCapability>,
    pub tools: Vec<ToolManifest>,
    pub skills: Vec<Skill>,
    pub hooks: Vec<HookDefinition>,
    pub limits: Vec<UsageWindow>,
    pub artifacts: Vec<ArtifactRecord>,
}

impl GatewayState {
    pub fn workspace(workspace: impl AsRef<Path>) -> Self {
        let workspace = workspace.as_ref();
        let tools = builtin_registry(workspace)
            .map(|registry| registry.manifests())
            .unwrap_or_default();
        Self {
            started_at: Utc::now(),
            mission: workspace_state(workspace),
            capabilities: configured_capabilities(),
            tools,
            skills: builtin_skills(),
            hooks: builtin_hooks(),
            limits: Vec::new(),
            artifacts: workspace_artifacts(workspace),
        }
    }

    pub fn sample(workspace: impl AsRef<Path>) -> Self {
        Self::workspace(workspace)
    }

    pub fn sessions(&self) -> Vec<SessionView> {
        vec![self.mission.session.clone()]
    }

    pub fn usage(&self) -> BudgetView {
        self.mission.budget.clone()
    }
}

fn workspace_artifacts(workspace: &Path) -> Vec<ArtifactRecord> {
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
        _ => "text/plain",
    }
    .into()
}

fn configured_capabilities() -> Vec<ModelCapability> {
    vec![
        ModelCapability {
            id: "neurogate/gpt-5".into(),
            owned_by: Some("neurogate".into()),
            credit_multiplier: 3.0,
            supports_streaming: true,
            supports_tools: false,
            context_window: None,
        },
        ModelCapability {
            id: "neurogate/qwen-coder-fast".into(),
            owned_by: Some("neurogate".into()),
            credit_multiplier: 0.7,
            supports_streaming: true,
            supports_tools: false,
            context_window: None,
        },
    ]
}
