use std::{
    fs,
    path::{Path, PathBuf},
};

use chrono::{DateTime, Utc};
use gorsee_code_artifacts::ArtifactRecord;
use gorsee_code_core::{Event, EventKind, ModelCapability};
use gorsee_code_hooks::{builtin_hooks, HookDefinition};
use gorsee_code_limits::UsageWindow;
use gorsee_code_safety::Redactor;
use gorsee_code_session::{
    ApprovalDecision, ApprovalRecord, SessionManifest, SessionStore, SessionStoreError,
};
use gorsee_code_skills::{builtin_skills, Skill};
use gorsee_code_tool_runtime::ToolManifest;
use gorsee_code_tools::builtin_registry;
use gorsee_code_ui_state::{workspace_state, BudgetView, SessionView, WorkspaceState};
use serde_json::{json, Value};

use crate::{
    artifacts::workspace_artifacts,
    sessions::{
        budget_view, decision_event, diff_files, read_session_view, session_view, DiffView,
    },
};

#[derive(Debug, Clone)]
pub struct GatewayState {
    workspace_path: PathBuf,
    pub started_at: DateTime<Utc>,
    pub workspace: WorkspaceState,
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
            workspace_path: workspace.to_path_buf(),
            started_at: Utc::now(),
            workspace: workspace_state(workspace),
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
        let sessions = self.stored_sessions();
        if sessions.is_empty() {
            vec![self.workspace.session.clone()]
        } else {
            sessions
        }
    }

    pub fn usage(&self) -> BudgetView {
        self.workspace.budget.clone()
    }

    pub(crate) fn workspace_path(&self) -> &Path {
        &self.workspace_path
    }

    pub(crate) fn session_root(&self) -> PathBuf {
        self.workspace_path.join(".gorsee-code")
    }

    pub fn create_session(
        &self,
        id: String,
        repo: String,
        branch: String,
    ) -> Result<SessionView, SessionStoreError> {
        let manifest = SessionManifest::new(id, repo, branch);
        self.store().create(&manifest)?;
        Ok(session_view(&self.workspace_path, &manifest))
    }

    pub fn session(&self, session_id: &str) -> Result<Option<SessionView>, SessionStoreError> {
        Ok(self
            .read_manifest(session_id)?
            .map(|manifest| session_view(&self.workspace_path, &manifest)))
    }

    pub fn session_events(&self, session_id: &str) -> Result<Vec<Event>, SessionStoreError> {
        self.store().read_events(session_id)
    }

    pub fn record_message(
        &self,
        session_id: &str,
        message: String,
    ) -> Result<Option<Event>, SessionStoreError> {
        if self.read_manifest(session_id)?.is_none() {
            return Ok(None);
        }
        self.append_event(
            session_id,
            EventKind::AgentMessage,
            json!({ "message": message }),
        )
        .map(Some)
    }

    pub fn pause_session(
        &self,
        session_id: &str,
    ) -> Result<Option<SessionView>, SessionStoreError> {
        self.update_status(session_id, "paused", EventKind::SessionPaused)
    }

    pub fn resume_session(
        &self,
        session_id: &str,
    ) -> Result<Option<SessionView>, SessionStoreError> {
        self.update_status(session_id, "running", EventKind::SessionResumed)
    }

    pub fn decide_approval(
        &self,
        session_id: &str,
        approval_id: &str,
        decision: ApprovalDecision,
    ) -> Result<Option<ApprovalRecord>, SessionStoreError> {
        if self.read_manifest(session_id)?.is_none() {
            return Ok(None);
        }
        let approval = self
            .store()
            .decide_approval(session_id, approval_id, decision)?;
        self.append_event(
            session_id,
            decision_event(decision),
            json!({ "approval_id": approval.id, "tool": approval.tool_name }),
        )?;
        Ok(Some(approval))
    }

    pub(crate) fn has_pending_execution(
        &self,
        session_id: &str,
    ) -> Result<Option<bool>, SessionStoreError> {
        if self.read_manifest(session_id)?.is_none() {
            return Ok(None);
        }
        Ok(Some(
            self.store()
                .session_dir(session_id)
                .join("execution.json")
                .is_file(),
        ))
    }

    pub(crate) fn approval(
        &self,
        session_id: &str,
        approval_id: &str,
    ) -> Result<Option<ApprovalRecord>, SessionStoreError> {
        if self.read_manifest(session_id)?.is_none() {
            return Ok(None);
        }
        Ok(self
            .store()
            .read_approvals(session_id)?
            .into_iter()
            .find(|approval| approval.id == approval_id))
    }

    pub fn session_usage(&self, session_id: &str) -> Result<Option<BudgetView>, SessionStoreError> {
        Ok(self
            .read_manifest(session_id)?
            .map(|manifest| budget_view(&manifest)))
    }

    pub fn session_limits(
        &self,
        session_id: &str,
    ) -> Result<Option<Vec<UsageWindow>>, SessionStoreError> {
        if self.read_manifest(session_id)?.is_none() {
            return Ok(None);
        }
        Ok(Some(self.limits.clone()))
    }

    pub(crate) fn session_diff(
        &self,
        session_id: &str,
    ) -> Result<Option<Vec<DiffView>>, SessionStoreError> {
        if self.read_manifest(session_id)?.is_none() {
            return Ok(None);
        }
        Ok(Some(diff_files(
            self.store().session_dir(session_id).join("patches"),
        )))
    }

    fn append_event(
        &self,
        session_id: &str,
        kind: EventKind,
        payload: Value,
    ) -> Result<Event, SessionStoreError> {
        let store = self.store();
        let sequence = store.read_events(session_id)?.len() as u64 + 1;
        let event = Event::new(sequence, session_id, None, kind, payload);
        store.append_event(&event)?;
        Ok(event)
    }

    fn read_manifest(
        &self,
        session_id: &str,
    ) -> Result<Option<SessionManifest>, SessionStoreError> {
        let store = self.store();
        if !store
            .session_dir(session_id)
            .join("manifest.json")
            .is_file()
        {
            return Ok(None);
        }
        store.read_manifest(session_id).map(Some)
    }

    fn stored_sessions(&self) -> Vec<SessionView> {
        let sessions_dir = self.workspace_path.join(".gorsee-code").join("sessions");
        let Ok(entries) = fs::read_dir(sessions_dir) else {
            return Vec::new();
        };
        entries
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| read_session_view(&self.workspace_path, entry.path()))
            .collect()
    }

    fn store(&self) -> SessionStore {
        SessionStore::new(
            self.workspace_path.join(".gorsee-code"),
            Redactor::default(),
        )
    }

    fn update_status(
        &self,
        session_id: &str,
        status: &str,
        kind: EventKind,
    ) -> Result<Option<SessionView>, SessionStoreError> {
        let Some(mut manifest) = self.read_manifest(session_id)? else {
            return Ok(None);
        };
        manifest.status = status.into();
        self.store().write_manifest(&manifest)?;
        self.append_event(session_id, kind, json!({ "status": status }))?;
        Ok(Some(session_view(&self.workspace_path, &manifest)))
    }
}

fn configured_capabilities() -> Vec<ModelCapability> {
    vec![
        ModelCapability {
            id: "glm-5.1".into(),
            owned_by: Some("neurogate".into()),
            credit_multiplier: 1.85,
            supports_streaming: true,
            supports_tools: false,
            context_window: None,
        },
        ModelCapability {
            id: "deepseek-v4-pro".into(),
            owned_by: Some("neurogate".into()),
            credit_multiplier: 0.4,
            supports_streaming: true,
            supports_tools: false,
            context_window: None,
        },
        ModelCapability {
            id: "vibe-lite-1".into(),
            owned_by: Some("neurogate".into()),
            credit_multiplier: 0.1,
            supports_streaming: true,
            supports_tools: false,
            context_window: None,
        },
    ]
}
