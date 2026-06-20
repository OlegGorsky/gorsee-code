use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use gorsee_code_skills::{builtin_skills, find_skill};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PanelItemTarget {
    File(PathBuf),
    SkillFile { id: String, path: PathBuf },
    McpConfig(PathBuf),
    ProjectPath(PathBuf),
    None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PanelItem {
    label: String,
    detail: String,
    target: PanelItemTarget,
}

impl PanelItem {
    pub(crate) fn new(
        label: impl Into<String>,
        detail: impl Into<String>,
        target: PanelItemTarget,
    ) -> Self {
        Self {
            label: label.into(),
            detail: detail.into(),
            target,
        }
    }

    pub(crate) fn label(&self) -> &str {
        &self.label
    }

    pub(crate) fn detail(&self) -> &str {
        &self.detail
    }

    pub(crate) fn target(&self) -> &PanelItemTarget {
        &self.target
    }
}

pub(crate) fn instruction_items(root: &Path) -> Vec<PanelItem> {
    ["AGENTS.md", "GORSEE.md", "README.md"]
        .into_iter()
        .map(|file| {
            let exists = root.join(file).exists();
            let detail = if exists {
                "редактировать"
            } else {
                "создать"
            };
            PanelItem::new(file, detail, PanelItemTarget::File(PathBuf::from(file)))
        })
        .collect()
}

pub(crate) fn project_items(root: &Path) -> Vec<PanelItem> {
    let mut items = vec![
        PanelItem::new(
            "Текущий проект",
            root.display().to_string(),
            PanelItemTarget::None,
        ),
        PanelItem::new("Ввести путь", "/project <путь>", PanelItemTarget::None),
    ];
    if let Some(parent) = root.parent() {
        items.push(PanelItem::new(
            "Родительская папка",
            parent.display().to_string(),
            PanelItemTarget::ProjectPath(parent.to_path_buf()),
        ));
    }
    if let Ok(home) = std::env::var("HOME") {
        let home = PathBuf::from(home);
        if home != root {
            items.push(PanelItem::new(
                "Домашняя папка",
                home.display().to_string(),
                PanelItemTarget::ProjectPath(home),
            ));
        }
    }
    items
}

pub(crate) fn skill_items(root: &Path) -> Vec<PanelItem> {
    let mut items = builtin_skills()
        .into_iter()
        .map(|skill| {
            let id = skill.id.clone();
            let path = skill_path(root, &id);
            let detail = if root.join(&path).exists() {
                "проектный override"
            } else {
                skill.description.as_str()
            };
            PanelItem::new(id.clone(), detail, PanelItemTarget::SkillFile { id, path })
        })
        .collect::<Vec<_>>();
    items.extend(project_skill_items(root));
    items.push(PanelItem::new(
        "Новый скилл",
        "создать .gorsee-code/skills/new-skill.md",
        PanelItemTarget::SkillFile {
            id: "new-skill".into(),
            path: PathBuf::from(".gorsee-code/skills/new-skill.md"),
        },
    ));
    items
}

pub(crate) fn mcp_items(root: &Path) -> Vec<PanelItem> {
    [".mcp.json", ".cursor/mcp.json", ".codex/mcp.json"]
        .into_iter()
        .map(|file| {
            let exists = root.join(file).exists();
            let detail = if exists {
                "редактировать"
            } else {
                "создать"
            };
            PanelItem::new(
                file,
                detail,
                PanelItemTarget::McpConfig(PathBuf::from(file)),
            )
        })
        .collect()
}

pub(crate) fn limit_items() -> Vec<PanelItem> {
    vec![
        PanelItem::new(
            "Сессия",
            "токены и стоп-порог проекта",
            PanelItemTarget::None,
        ),
        PanelItem::new(
            "Окно 5ч",
            "live-лимит через /limits watch",
            PanelItemTarget::None,
        ),
        PanelItem::new(
            "Окно 7ч",
            "live-лимит через /limits watch",
            PanelItemTarget::None,
        ),
    ]
}

pub(crate) fn ensure_target(root: &Path, target: &PanelItemTarget) -> Result<Option<PathBuf>> {
    match target {
        PanelItemTarget::File(path) | PanelItemTarget::McpConfig(path) => {
            ensure_file(root, path, default_text(path))?;
            Ok(Some(path.clone()))
        }
        PanelItemTarget::SkillFile { id, path } => {
            ensure_file(root, path, skill_template(id))?;
            Ok(Some(path.clone()))
        }
        PanelItemTarget::ProjectPath(_) | PanelItemTarget::None => Ok(None),
    }
}

fn skill_path(_root: &Path, id: &str) -> PathBuf {
    PathBuf::from(".gorsee-code")
        .join("skills")
        .join(format!("{id}.md"))
}

fn project_skill_items(root: &Path) -> Vec<PanelItem> {
    let skills_dir = root.join(".gorsee-code").join("skills");
    let Ok(entries) = fs::read_dir(skills_dir) else {
        return Vec::new();
    };
    let builtin = builtin_skills()
        .into_iter()
        .map(|skill| skill.id)
        .collect::<std::collections::BTreeSet<_>>();
    let mut items = entries
        .flatten()
        .filter(|entry| {
            entry
                .file_type()
                .map(|kind| kind.is_file())
                .unwrap_or(false)
        })
        .filter_map(|entry| {
            let path = entry.path();
            let id = path.file_stem()?.to_str()?.to_string();
            (!builtin.contains(&id)).then(|| {
                PanelItem::new(
                    id,
                    "проектный скилл",
                    PanelItemTarget::File(
                        PathBuf::from(".gorsee-code/skills").join(entry.file_name()),
                    ),
                )
            })
        })
        .collect::<Vec<_>>();
    items.sort_by(|left, right| left.label.cmp(&right.label));
    items
}

fn ensure_file(root: &Path, path: &Path, default: String) -> Result<()> {
    let absolute = root.join(path);
    if absolute.exists() {
        return Ok(());
    }
    if let Some(parent) = absolute.parent() {
        fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    fs::write(&absolute, default).with_context(|| format!("create {}", absolute.display()))
}

fn default_text(path: &Path) -> String {
    match path.file_name().and_then(|name| name.to_str()) {
        Some("mcp.json") => "{\n  \"servers\": {}\n}\n".into(),
        Some(name) if name.ends_with(".json") => "{\n  \"servers\": {}\n}\n".into(),
        _ => "# Project Instructions\n\n".into(),
    }
}

fn skill_template(id: &str) -> String {
    if let Some(skill) = find_skill(id) {
        return format!(
            "# Skill: {}\n\n## Назначение\n{}\n\n## Агенты\n{}\n\n## Инструменты\n{}\n\n## Инструкции\n{}\n\n## Проверка\n\n",
            skill.id,
            skill.description,
            skill.default_agents.join(", "),
            skill.allowed_tools.join(", "),
            skill.instructions
        );
    }
    format!("# Skill: {id}\n\n## Назначение\n\n## Инструкции\n\n## Проверка\n")
}
