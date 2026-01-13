use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub v: u32,
    pub op: Operation,
    pub id: String,
    pub ts: DateTime<Utc>,
    pub by: String,
    pub branch: String,
    pub d: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    Create,
    Update,
    Assign,
    Comment,
    Link,
    Unlink,
    Complete,
    Reopen,
    Archive,
}

impl std::fmt::Display for Operation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Operation::Create => write!(f, "create"),
            Operation::Update => write!(f, "update"),
            Operation::Assign => write!(f, "assign"),
            Operation::Comment => write!(f, "comment"),
            Operation::Link => write!(f, "link"),
            Operation::Unlink => write!(f, "unlink"),
            Operation::Complete => write!(f, "complete"),
            Operation::Reopen => write!(f, "reopen"),
            Operation::Archive => write!(f, "archive"),
        }
    }
}
