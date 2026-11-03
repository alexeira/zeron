use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const MAX_TASK_TEXT_BYTES: usize = 2000;
pub const MAX_TASK_NOTE_BYTES: usize = 4000;
pub const MAX_TASKS_PER_REQUEST: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TaskStatus {
    #[default]
    Pending,
    InProgress,
    Blocked,
    Completed,
}

impl TaskStatus {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "pending" | "todo" | "open" => Some(Self::Pending),
            "inprogress" | "in_progress" | "in-progress" | "active" | "working" => {
                Some(Self::InProgress)
            }
            "blocked" | "waiting" | "stuck" => Some(Self::Blocked),
            "completed" | "complete" | "done" => Some(Self::Completed),
            _ => None,
        }
    }
}

pub const TASK_STATUSES: [TaskStatus; 4] = [
    TaskStatus::Pending,
    TaskStatus::InProgress,
    TaskStatus::Blocked,
    TaskStatus::Completed,
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionTask {
    pub id: String,
    pub board_chat_id: String,
    pub text: String,
    #[serde(default, deserialize_with = "lenient_task_status")]
    pub status: TaskStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_chat_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub created_by: String,
    #[serde(with = "epoch_ms")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "epoch_ms")]
    pub updated_at: DateTime<Utc>,
}

mod epoch_ms {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S: Serializer>(at: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_i64(at.timestamp_millis())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<DateTime<Utc>, D::Error> {
        let ms = i64::deserialize(deserializer)?;
        DateTime::from_timestamp_millis(ms).ok_or_else(|| {
            serde::de::Error::custom(format!("{ms} is not a timestamp in milliseconds"))
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewTask {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_chat_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<TaskStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskUpdate {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<TaskStatus>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_optional"
    )]
    pub owner_chat_id: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "present_optional"
    )]
    pub note: Option<Option<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TaskWriteOutcome {
    Updated,
    Deleted,
    Missing,
    OtherBoard,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskWriteResult {
    pub id: String,
    pub outcome: TaskWriteOutcome,
}

impl TaskUpdate {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            text: None,
            status: None,
            owner_chat_id: None,
            note: None,
        }
    }
}

pub fn present_optional<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

fn lenient_task_status<'de, D>(deserializer: D) -> Result<TaskStatus, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = Option::<String>::deserialize(deserializer)?;
    Ok(raw
        .as_deref()
        .and_then(TaskStatus::parse)
        .unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn task() -> SessionTask {
        SessionTask {
            id: "task-1".into(),
            board_chat_id: "chat-1".into(),
            text: "ship it".into(),
            status: TaskStatus::InProgress,
            owner_chat_id: Some("chat-2".into()),
            note: None,
            created_by: "chat-1".into(),
            created_at: DateTime::from_timestamp_millis(1_000).unwrap(),
            updated_at: DateTime::from_timestamp_millis(2_000).unwrap(),
        }
    }

    #[test]
    fn statuses_round_trip_as_camel_case() {
        assert_eq!(
            serde_json::to_value(TaskStatus::InProgress).unwrap(),
            json!("inProgress")
        );
        for status in TASK_STATUSES {
            let value: TaskStatus = serde_json::from_value(json!(status)).unwrap();
            assert_eq!(value, status);
        }
    }

    #[test]
    fn a_model_spelling_parses_to_the_same_status() {
        assert_eq!(
            TaskStatus::parse("in_progress"),
            Some(TaskStatus::InProgress)
        );
        assert_eq!(TaskStatus::parse(" Done "), Some(TaskStatus::Completed));
        assert_eq!(TaskStatus::parse("Blocked"), Some(TaskStatus::Blocked));
        assert_eq!(TaskStatus::parse("nonsense"), None);
    }

    #[test]
    fn an_unknown_status_from_a_newer_peer_reads_as_pending() {
        let raw = json!({
            "id": "task-1", "boardChatId": "chat-1", "text": "t",
            "status": "deferred", "createdBy": "chat-1",
            "createdAt": 1000, "updatedAt": 2000,
        });
        let parsed: SessionTask = serde_json::from_value(raw).unwrap();
        assert_eq!(parsed.status, TaskStatus::Pending);
    }

    #[test]
    fn a_task_reads_back_from_its_wire_shape() {
        let value: Value = serde_json::to_value(task()).unwrap();
        assert_eq!(value["boardChatId"], json!("chat-1"));
        assert_eq!(value["status"], json!("inProgress"));
        assert!(value.get("note").is_none());
        let back: SessionTask = serde_json::from_value(value).unwrap();
        assert_eq!(back, task());
    }

    #[test]
    fn an_update_tells_absent_from_null() {
        let untouched: TaskUpdate = serde_json::from_value(json!({ "id": "task-1" })).unwrap();
        assert_eq!(untouched.owner_chat_id, None);
        assert_eq!(untouched.note, None);

        let cleared: TaskUpdate =
            serde_json::from_value(json!({ "id": "task-1", "ownerChatId": null })).unwrap();
        assert_eq!(cleared.owner_chat_id, Some(None));

        let reassigned: TaskUpdate =
            serde_json::from_value(json!({ "id": "task-1", "ownerChatId": "chat-3" })).unwrap();
        assert_eq!(reassigned.owner_chat_id, Some(Some("chat-3".into())));

        let written: Value = serde_json::to_value(TaskUpdate {
            id: "task-1".into(),
            text: None,
            status: None,
            owner_chat_id: Some(None),
            note: None,
        })
        .unwrap();
        assert_eq!(written, json!({ "id": "task-1", "ownerChatId": null }));
    }
}
