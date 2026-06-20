#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum CenterPanel {
    #[default]
    Timeline,
    Diff,
    Sessions,
    Models,
    Instructions,
    Skills,
    Mcp,
    Limits,
    Terminal,
}
