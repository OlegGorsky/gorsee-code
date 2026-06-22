use std::fs;

use gorsee_code_safety::PathPolicy;
use gorsee_code_tool_runtime::ToolRuntimeError;

use super::PatchFile;

pub(super) fn files_from_unified_patch(
    patch: &str,
    paths: &PathPolicy,
) -> Result<Vec<PatchFile>, ToolRuntimeError> {
    let lines = patch.lines().collect::<Vec<_>>();
    let mut files = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        if !lines[index].starts_with("--- ") {
            index += 1;
            continue;
        }
        let old_path = header_path(lines[index], "--- ")?;
        index += 1;
        if index >= lines.len() || !lines[index].starts_with("+++ ") {
            return Err(handler("unified patch missing +++ header"));
        }
        let new_path = header_path(lines[index], "+++ ")?;
        index += 1;
        let start = index;
        while index < lines.len() && !lines[index].starts_with("--- ") {
            index += 1;
        }
        files.push(apply_unified_file(
            paths,
            old_path,
            new_path,
            &lines[start..],
        )?);
    }
    if files.is_empty() {
        return Err(handler("patch contains no file changes"));
    }
    Ok(files)
}

fn apply_unified_file(
    paths: &PathPolicy,
    old_path: &str,
    new_path: &str,
    hunk_lines: &[&str],
) -> Result<PatchFile, ToolRuntimeError> {
    let path = clean_patch_path(if new_path == "/dev/null" {
        old_path
    } else {
        new_path
    })?;
    let original = if old_path == "/dev/null" {
        Vec::new()
    } else {
        let target = paths
            .resolve_existing(clean_patch_path(old_path)?)
            .map_err(handler)?;
        fs::read_to_string(target)
            .map_err(handler)?
            .lines()
            .map(ToOwned::to_owned)
            .collect()
    };
    let content = apply_hunks(&original, hunk_lines)?;
    Ok(PatchFile { path, content })
}

fn apply_hunks(original: &[String], lines: &[&str]) -> Result<String, ToolRuntimeError> {
    let mut output = Vec::new();
    let mut cursor = 0usize;
    let mut index = 0usize;
    while index < lines.len() {
        if !lines[index].starts_with("@@ ") {
            index += 1;
            continue;
        }
        let old_start = parse_old_start(lines[index])?;
        let hunk_start = old_start.saturating_sub(1);
        if hunk_start < cursor || hunk_start > original.len() {
            return Err(handler("patch hunk is outside file bounds"));
        }
        output.extend_from_slice(&original[cursor..hunk_start]);
        cursor = hunk_start;
        index += 1;
        while index < lines.len() && !lines[index].starts_with("@@ ") {
            apply_hunk_line(lines[index], original, &mut output, &mut cursor)?;
            index += 1;
        }
    }
    output.extend_from_slice(&original[cursor..]);
    let mut content = output.join("\n");
    if !content.is_empty() {
        content.push('\n');
    }
    Ok(content)
}

fn apply_hunk_line(
    line: &str,
    original: &[String],
    output: &mut Vec<String>,
    cursor: &mut usize,
) -> Result<(), ToolRuntimeError> {
    let Some((marker, text)) = line.split_at_checked(1) else {
        return Ok(());
    };
    match marker {
        "+" => output.push(text.to_string()),
        "-" => consume_original(text, original, cursor)?,
        " " => {
            consume_original(text, original, cursor)?;
            output.push(text.to_string());
        }
        "\\" => {}
        _ => return Err(handler("unsupported unified patch line")),
    }
    Ok(())
}

fn consume_original(
    expected: &str,
    original: &[String],
    cursor: &mut usize,
) -> Result<(), ToolRuntimeError> {
    let actual = original
        .get(*cursor)
        .ok_or_else(|| handler("patch hunk exceeds source file"))?;
    if actual != expected {
        return Err(handler("patch context does not match source file"));
    }
    *cursor += 1;
    Ok(())
}

fn parse_old_start(header: &str) -> Result<usize, ToolRuntimeError> {
    let range = header
        .split_whitespace()
        .find(|part| part.starts_with('-'))
        .ok_or_else(|| handler("patch hunk missing old range"))?;
    range
        .trim_start_matches('-')
        .split(',')
        .next()
        .unwrap_or("0")
        .parse::<usize>()
        .map_err(handler)
}

fn header_path<'a>(line: &'a str, prefix: &str) -> Result<&'a str, ToolRuntimeError> {
    line.strip_prefix(prefix)
        .and_then(|value| value.split_whitespace().next())
        .ok_or_else(|| handler("unified patch header missing path"))
}

fn clean_patch_path(path: &str) -> Result<String, ToolRuntimeError> {
    let path = path
        .strip_prefix("a/")
        .or_else(|| path.strip_prefix("b/"))
        .unwrap_or(path);
    if path == "/dev/null" || path.trim().is_empty() {
        return Err(handler("unified patch missing target path"));
    }
    Ok(path.to_string())
}

fn handler(error: impl std::fmt::Display) -> ToolRuntimeError {
    ToolRuntimeError::Handler(error.to_string())
}
