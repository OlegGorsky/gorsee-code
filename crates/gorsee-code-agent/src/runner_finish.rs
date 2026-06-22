use gorsee_code_artifacts::ArtifactRecord;
use gorsee_code_core::{AgentProfile, EventKind, TaskSpec};
use gorsee_code_session::{SessionManifest, SessionStore};
use serde_json::json;

use crate::{events::EventSink, AgentRunError};

pub(crate) fn manifest_for(
    session_id: &str,
    spec: &TaskSpec,
    agents: &[AgentProfile],
) -> SessionManifest {
    let mut manifest = SessionManifest::new(session_id, &spec.repo_path, "unknown");
    manifest.agents = agents.iter().map(|agent| agent.id().to_string()).collect();
    manifest.budget.tokens_limit = spec.budget_tokens;
    manifest
}

pub(crate) fn finish_success(
    store: &SessionStore,
    sink: &mut EventSink<'_>,
    manifest: &mut SessionManifest,
    skill_id: Option<&str>,
    artifacts: &[ArtifactRecord],
) -> Result<(), AgentRunError> {
    finish_completed(store, sink, manifest, skill_id, artifacts)
}

pub(crate) fn finish_turn_success(
    store: &SessionStore,
    sink: &mut EventSink<'_>,
    manifest: &mut SessionManifest,
    skill_id: Option<&str>,
    artifacts: &[ArtifactRecord],
) -> Result<(), AgentRunError> {
    finish_completed(store, sink, manifest, skill_id, artifacts)
}

pub(crate) fn finish_unsuccessful(
    store: &SessionStore,
    sink: &mut EventSink<'_>,
    manifest: &mut SessionManifest,
    error: &AgentRunError,
) -> Result<(), AgentRunError> {
    match error {
        AgentRunError::WaitingApproval(_) => finish_waiting_approval(store, manifest),
        _ => finish_failure(store, sink, manifest, error),
    }
}

fn finish_completed(
    store: &SessionStore,
    sink: &mut EventSink<'_>,
    manifest: &mut SessionManifest,
    skill_id: Option<&str>,
    artifacts: &[ArtifactRecord],
) -> Result<(), AgentRunError> {
    for artifact in artifacts {
        sink.push(
            None,
            EventKind::ArtifactCreated,
            json!({ "artifact": artifact }),
        )?;
    }
    if let Some(skill_id) = skill_id {
        sink.push(
            None,
            EventKind::SkillFinished,
            json!({ "skill": skill_id, "status": "finished" }),
        )?;
    }
    sink.push(
        None,
        EventKind::TurnFinished,
        json!({ "status": "finished" }),
    )?;
    manifest.status = "ready".into();
    store.write_manifest(manifest)?;
    Ok(())
}

fn finish_failure(
    store: &SessionStore,
    sink: &mut EventSink<'_>,
    manifest: &mut SessionManifest,
    error: &AgentRunError,
) -> Result<(), AgentRunError> {
    sink.push(
        None,
        EventKind::Error,
        json!({ "error": error.to_string() }),
    )?;
    manifest.status = "failed".into();
    store.write_manifest(manifest)?;
    Ok(())
}

fn finish_waiting_approval(
    store: &SessionStore,
    manifest: &mut SessionManifest,
) -> Result<(), AgentRunError> {
    manifest.status = "waiting_approval".into();
    store.write_manifest(manifest)?;
    Ok(())
}
