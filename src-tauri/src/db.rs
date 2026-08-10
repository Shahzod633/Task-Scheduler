use rusqlite::{Connection, Result};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

pub struct DbState {
    pub conn: Mutex<Connection>,
}

pub fn init(app_dir: &PathBuf) -> Result<Connection> {
    if !app_dir.exists() {
        fs::create_dir_all(app_dir).expect("Failed to create app data directory");
    }
    
    let db_path = app_dir.join("trello_clone.db");
    let conn = Connection::open(&db_path)?;
    
    // Create tables
    conn.execute(
        "CREATE TABLE IF NOT EXISTS workspaces (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            visibility TEXT DEFAULT 'private',
            created_at TEXT DEFAULT (datetime('now')),
            archived INTEGER DEFAULT 0
        )",
        (),
    )?;
    
    conn.execute(
        "CREATE TABLE IF NOT EXISTS boards (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            workspace_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            gradient TEXT DEFAULT 'linear-gradient(135deg, #667eea 0%, #764ba2 100%)',
            is_starred INTEGER DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now')),
            archived INTEGER DEFAULT 0,
            FOREIGN KEY (workspace_id) REFERENCES workspaces(id)
        )",
        (),
    )?;
    
    conn.execute(
        "CREATE TABLE IF NOT EXISTS columns (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            board_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            position INTEGER NOT NULL,
            created_at TEXT DEFAULT (datetime('now')),
            archived INTEGER DEFAULT 0,
            FOREIGN KEY (board_id) REFERENCES boards(id)
        )",
        (),
    )?;
    
    conn.execute(
        "CREATE TABLE IF NOT EXISTS cards (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            column_id INTEGER NOT NULL,
            title TEXT NOT NULL,
            description TEXT DEFAULT '',
            position INTEGER NOT NULL,
            due_date TEXT,
            created_at TEXT DEFAULT (datetime('now')),
            archived INTEGER DEFAULT 0,
            FOREIGN KEY (column_id) REFERENCES columns(id)
        )",
        (),
    )?;
    
    conn.execute(
        "CREATE TABLE IF NOT EXISTS labels (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            board_id INTEGER NOT NULL,
            name TEXT DEFAULT '',
            color TEXT NOT NULL,
            FOREIGN KEY (board_id) REFERENCES boards(id)
        )",
        (),
    )?;
    
    conn.execute(
        "CREATE TABLE IF NOT EXISTS card_labels (
            card_id INTEGER NOT NULL,
            label_id INTEGER NOT NULL,
            PRIMARY KEY (card_id, label_id),
            FOREIGN KEY (card_id) REFERENCES cards(id),
            FOREIGN KEY (label_id) REFERENCES labels(id)
        )",
        (),
    )?;
    
    Ok(conn)
}
