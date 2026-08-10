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
    pub labels: Vec<Label>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Label {
    pub id: i64,
    pub board_id: i64,
    pub name: String,
    pub color: String,
}
