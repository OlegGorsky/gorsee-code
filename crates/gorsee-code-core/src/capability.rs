use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelCapability {
    pub id: String,
    pub owned_by: Option<String>,
    pub credit_multiplier: f64,
    pub supports_streaming: bool,
    pub supports_tools: bool,
    pub context_window: Option<u64>,
}

impl ModelCapability {
    pub fn relative_cost_label(&self) -> &'static str {
        match self.credit_multiplier {
            n if n <= 0.75 => "cheap",
            n if n <= 1.5 => "standard",
            n if n <= 3.0 => "expensive",
            _ => "premium",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cost_label_uses_multiplier_bands() {
        let mut capability = ModelCapability {
            id: "m".into(),
            owned_by: None,
            credit_multiplier: 1.0,
            supports_streaming: false,
            supports_tools: false,
            context_window: None,
        };
        assert_eq!(capability.relative_cost_label(), "standard");
        capability.credit_multiplier = 3.1;
        assert_eq!(capability.relative_cost_label(), "premium");
    }
}
