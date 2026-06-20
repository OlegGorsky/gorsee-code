#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum CenterPanel {
    #[default]
    Timeline,
    Project,
    Diff,
    Sessions,
    Models,
    Instructions,
    Skills,
    Mcp,
    Limits,
    Terminal,
}
