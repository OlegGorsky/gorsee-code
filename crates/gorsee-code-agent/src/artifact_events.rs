use serde_json::json;

use crate::{
    artifact_signals::{DiffSignal, VerificationSignal},
    events::EventSink,
    session_artifacts::RunArtifacts,
    AgentRunError,
};
use gorsee_code_core::EventKind;

pub(crate) fn record_artifact_outcomes(
    sink: &mut EventSink<'_>,
    artifacts: &RunArtifacts,
) -> Result<(), AgentRunError> {
    record_diff_signal(sink, &artifacts.diff)?;
    record_verification_signal(sink, &artifacts.verification)?;
    Ok(())
}

fn record_diff_signal(sink: &mut EventSink<'_>, signal: &DiffSignal) -> Result<(), AgentRunError> {
    if !signal.emit {
        return Ok(());
    }
    sink.push(
        None,
        EventKind::DiffReady,
        json!({
            "summary": signal.summary,
            "status": signal.status,
            "files_changed": signal.files_changed,
            "additions": signal.additions,
            "deletions": signal.deletions,
            "artifact": signal.artifact,
        }),
    )
}

fn record_verification_signal(
    sink: &mut EventSink<'_>,
    signal: &VerificationSignal,
) -> Result<(), AgentRunError> {
    if !signal.emit {
        return Ok(());
    }
    sink.push(
        None,
        EventKind::TestFinished,
        json!({
            "summary": signal.summary,
            "status": signal.status,
            "command": signal.command,
            "artifact": signal.artifact,
        }),
    )
}
