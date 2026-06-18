use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandKind {
    UserMessage,
    Approve,
    Deny,
    Pause,
    Resume,
    SelectModel,
    ChangeBudget,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Command {
    pub id: Uuid,
    pub session_id: String,
    pub kind: CommandKind,
    pub payload: Value,
}

impl Command {
    pub fn new(session_id: impl Into<String>, kind: CommandKind, payload: Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            session_id: session_id.into(),
            kind,
            payload,
        }
    }
}
