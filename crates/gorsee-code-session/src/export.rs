use gorsee_code_core::Event;
use gorsee_code_safety::Redactor;

use crate::SessionManifest;

pub fn export_markdown(
    manifest: &SessionManifest,
    events: &[Event],
    redactor: &Redactor,
) -> String {
    let mut markdown = format!(
        "# Gorsee Code Session {}\n\n- Repo: `{}`\n- Branch: `{}`\n- Status: `{}`\n\n## Events\n\n",
        manifest.id, manifest.repo, manifest.branch, manifest.status
    );
    for event in events {
        let line = format!(
            "- {} `{:?}` {}\n",
            event.sequence, event.kind, event.payload
        );
        markdown.push_str(&redactor.redact(&line));
    }
    markdown
}
