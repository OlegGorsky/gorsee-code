use gorsee_code_safety::RiskClass;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub allowed_tools: Vec<String>,
    pub default_agents: Vec<String>,
    pub risk: RiskClass,
    pub instructions: String,
}

pub fn builtin_skills() -> Vec<Skill> {
    vec![repo_audit_skill(), bug_fix_skill(), release_check_skill()]
}

pub fn find_skill(id: &str) -> Option<Skill> {
    builtin_skills().into_iter().find(|skill| skill.id == id)
}

pub fn skill_ids() -> Vec<String> {
    builtin_skills().into_iter().map(|skill| skill.id).collect()
}

fn repo_audit_skill() -> Skill {
    skill(
        "repo-audit",
        "Repository Audit",
        "Read-only repository risk and structure audit.",
        &[
            "list_files",
            "search_text",
            "read_file",
            "repo_map",
            "git_diff",
        ],
        &["architect", "scout", "validator"],
        RiskClass::Read,
    )
}

fn bug_fix_skill() -> Skill {
    skill(
        "bug-fix",
        "Bug Fix",
        "Trace a defect, propose a bounded patch, and validate tests.",
        &[
            "search_text",
            "read_file",
            "propose_patch",
            "apply_patch",
            "run_test",
            "git_diff",
        ],
        &["architect", "scout", "coder", "validator"],
        RiskClass::Write,
    )
}

fn release_check_skill() -> Skill {
    skill(
        "release-check",
        "Release Check",
        "Run quality gates and prepare a release readiness summary.",
        &["git_status", "git_diff", "run_test", "final_answer"],
        &["validator", "summarizer"],
        RiskClass::Read,
    )
}

fn skill(
    id: &str,
    name: &str,
    description: &str,
    tools: &[&str],
    agents: &[&str],
    risk: RiskClass,
) -> Skill {
    Skill {
        id: id.into(),
        name: name.into(),
        description: description.into(),
        allowed_tools: tools.iter().map(|tool| (*tool).into()).collect(),
        default_agents: agents.iter().map(|agent| (*agent).into()).collect(),
        risk,
        instructions: format!("{name}: {description}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_include_foundation_skills() {
        assert_eq!(skill_ids(), ["repo-audit", "bug-fix", "release-check"]);
    }
}
