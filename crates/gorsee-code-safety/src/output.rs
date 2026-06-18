use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputBounds {
    pub max_bytes: usize,
    pub max_lines: usize,
}

impl Default for OutputBounds {
    fn default() -> Self {
        Self {
            max_bytes: 64 * 1024,
            max_lines: 1_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundedOutput {
    pub text: String,
    pub truncated: bool,
}

impl OutputBounds {
    pub fn apply(&self, text: &str) -> BoundedOutput {
        let mut truncated = false;
        let mut lines: Vec<&str> = text.lines().take(self.max_lines).collect();
        if text.lines().count() > self.max_lines {
            truncated = true;
        }

        let mut output = lines.join("\n");
        if output.len() > self.max_bytes {
            output.truncate(self.max_bytes);
            truncated = true;
        }
        if truncated {
            output.push_str("\n[gorsee: output truncated]");
        }
        lines.clear();
        BoundedOutput {
            text: output,
            truncated,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_long_output() {
        let bounds = OutputBounds {
            max_bytes: 5,
            max_lines: 10,
        };
        let output = bounds.apply("abcdef");
        assert!(output.truncated);
        assert!(output.text.contains("truncated"));
    }
}
