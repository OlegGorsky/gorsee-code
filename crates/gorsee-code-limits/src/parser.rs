use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageWindow {
    pub label: String,
    pub credits_used: f64,
    pub credit_limit: f64,
    pub requests_used: u64,
    pub request_limit: u64,
    pub started_at: Option<String>,
    pub ends_at: Option<String>,
}

impl UsageWindow {
    pub fn credit_percent(&self) -> f64 {
        percent(self.credits_used, self.credit_limit)
    }

    pub fn request_percent(&self) -> f64 {
        percent(self.requests_used as f64, self.request_limit as f64)
    }
}

pub fn parse_usage_windows(value: &Value) -> Vec<UsageWindow> {
    let Some(row) = value.pointer("/usage/rows/0").and_then(Value::as_object) else {
        return Vec::new();
    };

    ["5Hours", "24Hours", "7Days", "30Days"]
        .into_iter()
        .map(|suffix| UsageWindow {
            label: suffix.to_string(),
            credits_used: number(row.get(&format!("credits{suffix}"))),
            credit_limit: number(row.get(&format!("creditLimit{suffix}"))),
            requests_used: integer(row.get(&format!("requests{suffix}"))),
            request_limit: integer(row.get(&format!("requestLimit{suffix}"))),
            started_at: string(row.get(&format!("window{suffix}StartedAt"))),
            ends_at: string(row.get(&format!("window{suffix}EndsAt"))),
        })
        .collect()
}

fn number(value: Option<&Value>) -> f64 {
    value.and_then(Value::as_f64).unwrap_or_default()
}

fn integer(value: Option<&Value>) -> u64 {
    value.and_then(Value::as_u64).unwrap_or_default()
}

fn string(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(ToOwned::to_owned)
}

fn percent(used: f64, limit: f64) -> f64 {
    if limit <= 0.0 {
        0.0
    } else {
        used * 100.0 / limit
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_neurogate_usage_rows() {
        let payload = json!({
            "usage": {
                "rows": [{
                    "creditLimit5Hours": 100.0,
                    "credits5Hours": 40.0,
                    "requestLimit5Hours": 1000,
                    "requests5Hours": 125,
                    "window5HoursStartedAt": "2026-06-18T10:00:00Z",
                    "window5HoursEndsAt": "2026-06-18T15:00:00Z"
                }]
            }
        });

        let windows = parse_usage_windows(&payload);
        assert_eq!(windows[0].label, "5Hours");
        assert_eq!(windows[0].credit_percent(), 40.0);
        assert_eq!(windows[0].request_percent(), 12.5);
    }
}
