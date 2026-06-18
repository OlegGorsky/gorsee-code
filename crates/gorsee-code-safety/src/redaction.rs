use regex::Regex;

#[derive(Clone, Debug)]
pub struct Redactor {
    patterns: Vec<Regex>,
}

impl Redactor {
    pub fn new(user_patterns: &[String]) -> Result<Self, regex::Error> {
        let mut raw = vec![
            r"(?i)bearer\s+[a-z0-9._\-]+".to_string(),
            r#"(?i)(api[_-]?key|token|secret|password)\s*[:=]\s*['"]?[^'"\s]+"#.to_string(),
            r"(?is)-----BEGIN [A-Z ]*PRIVATE KEY-----.*?-----END [A-Z ]*PRIVATE KEY-----"
                .to_string(),
            r"(?i)cookie:\s*[^\n\r]+".to_string(),
        ];
        raw.extend(user_patterns.iter().cloned());

        let patterns = raw
            .into_iter()
            .map(|pattern| Regex::new(&pattern))
            .collect::<Result<_, _>>()?;
        Ok(Self { patterns })
    }

    pub fn redact(&self, input: &str) -> String {
        self.patterns
            .iter()
            .fold(input.to_string(), |text, pattern| {
                pattern.replace_all(&text, "[REDACTED]").to_string()
            })
    }
}

impl Default for Redactor {
    fn default() -> Self {
        Self::new(&[]).expect("built-in redaction patterns must compile")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_bearer_and_assignment_secrets() {
        let redactor = Redactor::default();
        let text = "Authorization: Bearer abc.def\nauth_token=secret123";
        let redacted = redactor.redact(text);
        assert!(!redacted.contains("abc.def"));
        assert!(!redacted.contains("secret123"));
    }
}
