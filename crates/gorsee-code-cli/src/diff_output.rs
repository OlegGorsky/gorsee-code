use serde_json::Value;

pub(crate) fn render_structured_diff(value: &Value, details: &str) -> String {
    let Some(diff) = value.get("diff") else {
        return format!("diff:\n{}\n", details.trim_end());
    };
    let Some(summary) = diff.get("summary") else {
        return format!("diff:\n{}\n", details.trim_end());
    };
    let files_changed = summary_usize(summary, "files_changed");
    let additions = summary_usize(summary, "additions");
    let deletions = summary_usize(summary, "deletions");
    if files_changed == 0 {
        return "diff: clean\n".into();
    }
    let mut out = format!(
        "diff: {} files changed, +{} -{}\n",
        files_changed, additions, deletions
    );
    render_files(&mut out, diff);
    if !details.trim().is_empty() {
        out.push_str("\ndetails:\n");
        out.push_str(details.trim_end());
        out.push('\n');
    }
    out
}

fn render_files(out: &mut String, diff: &Value) {
    let Some(files) = diff.get("files").and_then(Value::as_array) else {
        return;
    };
    for file in files {
        let status = file
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let path = file
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let additions = summary_usize(file, "additions");
        let deletions = summary_usize(file, "deletions");
        out.push_str(&format!("- {status} {path} +{additions} -{deletions}\n"));
    }
}

fn summary_usize(value: &Value, key: &str) -> usize {
    value.get(key).and_then(Value::as_u64).unwrap_or(0) as usize
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn structured_diff_renderer_leads_with_summary_and_keeps_patch_as_details() {
        let rendered = render_structured_diff(
            &json!({
                "diff": {
                    "summary": {"files_changed": 1, "additions": 2, "deletions": 1},
                    "files": [{
                        "path": "src/main.rs",
                        "status": "modified",
                        "additions": 2,
                        "deletions": 1
                    }]
                }
            }),
            "diff --git a/src/main.rs b/src/main.rs\n",
        );

        assert!(rendered.starts_with("diff: 1 files changed, +2 -1"));
        assert!(rendered.contains("- modified src/main.rs +2 -1"));
        assert!(rendered.contains("\ndetails:\ndiff --git"));
    }
}
