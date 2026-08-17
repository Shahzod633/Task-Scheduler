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

// ─── Board export / import ───
//
// A self-contained snapshot of one board. Database ids are deliberately NOT
// reused on import (they would collide with existing rows); labels instead get
// export-local ids that cards reference and the importer remaps.
//
// Every field that a future version might add is `#[serde(default)]`, so an
// export written by an older build still imports cleanly.

/// Bumped whenever the shape below changes incompatibly.
pub const EXPORT_FORMAT_VERSION: i64 = 1;

#[derive(Debug, Serialize, Deserialize)]
pub struct BoardExport {
    pub taskflow_export_version: i64,
    pub exported_at: String,
    pub board: BoardExportBody,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BoardExportBody {
    pub name: String,
    #[serde(default)]
    pub gradient: String,
    #[serde(default)]
    pub is_starred: bool,
    #[serde(default)]
    pub labels: Vec<LabelExport>,
    #[serde(default)]
    pub columns: Vec<ColumnExport>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LabelExport {
    /// Export-local id, referenced by `CardExport::label_ids`.
    pub id: i64,
    #[serde(default)]
    pub name: String,
    pub color: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ColumnExport {
    pub name: String,
    #[serde(default)]
    pub position: i64,
    #[serde(default)]
    pub archived: i8,
    #[serde(default)]
    pub cards: Vec<CardExport>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CardExport {
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub position: i64,
    #[serde(default)]
    pub due_date: Option<String>,
    #[serde(default)]
    pub archived: i8,
    #[serde(default)]
    pub is_mistake: bool,
    #[serde(default)]
    pub mistake_marked_at: Option<String>,
    #[serde(default)]
    pub mistake_resolved_at: Option<String>,
    #[serde(default)]
    pub label_ids: Vec<i64>,
}

/// One automatic backup file, as shown on the Settings screen.
#[derive(Debug, Serialize, Deserialize)]
pub struct BackupInfo {
    pub file_name: String,
    pub size_bytes: u64,
    /// Timestamp parsed out of the file name (`YYYY-MM-DD HH:MM:SS`).
    pub created_at: String,
}
