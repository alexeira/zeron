use super::*;
use zeron_proto::{SessionTask, TaskUpdate};

impl RegistryDoc {
    pub fn task(&self, task_id: &str) -> Result<Option<SessionTask>, DocError> {
        Ok(self
            .overlay_row(KIND_TASKS, task_id)
            .and_then(|row| row_to::<SessionTask>(&row)))
    }

    pub fn task_exists(&self, task_id: &str) -> bool {
        self.row_exists(KIND_TASKS, task_id)
    }

    pub fn task_deleted(&self, task_id: &str) -> bool {
        self.overlay_row_or_tombstone(KIND_TASKS, task_id)
            .is_some_and(|row| row.deleted)
    }

    pub fn read_tasks(&self) -> Result<Vec<SessionTask>, DocError> {
        let mut tasks: Vec<SessionTask> = self.read_kind::<SessionTask>(KIND_TASKS);
        tasks.sort_by(|a, b| a.created_at.cmp(&b.created_at).then(a.id.cmp(&b.id)));
        Ok(tasks)
    }

    pub fn task_board(&self, board_chat_id: &str) -> Result<Vec<SessionTask>, DocError> {
        Ok(self
            .read_tasks()?
            .into_iter()
            .filter(|task| task.board_chat_id == board_chat_id)
            .collect())
    }

    pub fn upsert_task(&mut self, task: &SessionTask) -> Result<(), DocError> {
        let set = fields([
            ("id", json!(task.id)),
            ("boardChatId", json!(task.board_chat_id)),
            ("text", json!(task.text)),
            ("status", serde_json::to_value(task.status)?),
            ("ownerChatId", opt_str(task.owner_chat_id.as_deref())),
            ("note", opt_str(task.note.as_deref())),
            ("createdBy", json!(task.created_by)),
            ("createdAt", json!(task.created_at.timestamp_millis())),
            ("updatedAt", json!(task.updated_at.timestamp_millis())),
        ]);
        self.write(KIND_TASKS, &task.id.clone(), OpKind::Upsert, set);
        Ok(())
    }

    pub fn update_task(&mut self, update: &TaskUpdate, at: DateTime<Utc>) -> Result<(), DocError> {
        let mut set = fields([("updatedAt", json!(at.timestamp_millis()))]);
        if let Some(text) = update.text.as_deref() {
            set.insert("text".into(), json!(text));
        }
        if let Some(status) = update.status {
            set.insert("status".into(), serde_json::to_value(status)?);
        }
        if let Some(owner) = &update.owner_chat_id {
            set.insert("ownerChatId".into(), opt_str(owner.as_deref()));
        }
        if let Some(note) = &update.note {
            set.insert("note".into(), opt_str(note.as_deref()));
        }
        self.write(KIND_TASKS, &update.id, OpKind::Update, set);
        Ok(())
    }

    pub fn delete_task(&mut self, task_id: &str) -> bool {
        let existed = self.task_exists(task_id);
        self.delete_row_ops(&[(KIND_TASKS, task_id)]);
        existed
    }
}
