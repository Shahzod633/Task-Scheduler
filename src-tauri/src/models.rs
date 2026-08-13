use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Workspace {
    pub id: i64,
    pub name: String,
    pub visibility: String,
    pub created_at: String,
    pub archived: i8,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Board {
    pub id: i64,
    pub workspace_id: i64,
    pub name: String,
    pub gradient: String,
    pub is_starred: bool,
    pub created_at: String,
    pub archived: i8,
    pub is_system: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Column {
    pub id: i64,
    pub board_id: i64,
    pub name: String,
    pub position: i64,
    pub created_at: String,
    pub archived: i8,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Card {
    pub id: i64,
    pub column_id: i64,
    pub title: String,
    pub description: String,
    pub position: i64,
    pub due_date: Option<String>,
    pub created_at: String,
    pub archived: i8,
    pub is_mistake: bool,
    pub mistake_marked_at: Option<String>,
    pub mistake_resolved_at: Option<String>,
    pub labels: Vec<Label>,
    /// Populated only by queries that join across boards/columns (e.g. planner, mistake dashboard).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub board_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub board_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Label {
    pub id: i64,
    pub board_id: i64,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Notification {
    pub id: i64,
    pub title: String,
    pub body: String,
    pub created_at: String,
    pub read: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserProfile {
    pub avatar_initials: String,
    pub display_name: String,
    pub theme: String,
}
