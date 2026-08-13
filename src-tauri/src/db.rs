use rusqlite::{Connection, OptionalExtension, Result, params};
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
            is_system INTEGER DEFAULT 0,
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
            is_mistake INTEGER DEFAULT 0,
            mistake_marked_at TEXT,
            mistake_resolved_at TEXT,
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

    conn.execute(
        "CREATE TABLE IF NOT EXISTS notifications (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            title TEXT NOT NULL,
            body TEXT DEFAULT '',
            created_at TEXT DEFAULT (datetime('now')),
            read INTEGER DEFAULT 0
        )",
        (),
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS user_profile (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            avatar_initials TEXT DEFAULT 'TF',
            display_name TEXT DEFAULT 'Пользователь',
            theme TEXT DEFAULT 'dark'
        )",
        (),
    )?;

    conn.execute(
        "CREATE TABLE IF NOT EXISTS board_recent_views (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            board_id INTEGER NOT NULL,
            opened_at TEXT DEFAULT (datetime('now')),
            FOREIGN KEY (board_id) REFERENCES boards(id)
        )",
        (),
    )?;

    // ─── Migrations for pre-existing databases ───
    // (CREATE TABLE IF NOT EXISTS above only creates columns on brand-new tables;
    // existing installs need ALTER TABLE for newly introduced columns.)
    if !table_has_column(&conn, "boards", "is_system") {
        conn.execute("ALTER TABLE boards ADD COLUMN is_system INTEGER DEFAULT 0", ())?;
    }
    if !table_has_column(&conn, "cards", "is_mistake") {
        conn.execute("ALTER TABLE cards ADD COLUMN is_mistake INTEGER DEFAULT 0", ())?;
    }
    if !table_has_column(&conn, "cards", "mistake_marked_at") {
        conn.execute("ALTER TABLE cards ADD COLUMN mistake_marked_at TEXT", ())?;
    }
    if !table_has_column(&conn, "cards", "mistake_resolved_at") {
        conn.execute("ALTER TABLE cards ADD COLUMN mistake_resolved_at TEXT", ())?;
    }

    // Ensure the singleton user profile row exists
    conn.execute("INSERT OR IGNORE INTO user_profile (id) VALUES (1)", ())?;

    // Backfill a hidden Inbox board for every existing workspace
    {
        let mut stmt = conn.prepare("SELECT id FROM workspaces WHERE archived = 0")?;
        let ws_ids: Vec<i64> = stmt
            .query_map([], |row| row.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        drop(stmt);
        for ws_id in ws_ids {
            ensure_inbox_board(&conn, ws_id)?;
        }
    }

    Ok(conn)
}

/// Returns true if `table` already has a column named `column`.
fn table_has_column(conn: &Connection, table: &str, column: &str) -> bool {
    let query = format!("PRAGMA table_info({})", table);
    let mut stmt = match conn.prepare(&query) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let cols: Vec<String> = match stmt.query_map([], |row| row.get::<_, String>(1)) {
        Ok(iter) => iter.filter_map(|r| r.ok()).collect(),
        Err(_) => return false,
    };
    cols.iter().any(|c| c == column)
}

/// Ensures the given workspace has a hidden system "Inbox" board with at least
/// one column, creating it if missing. Returns the id of the Inbox column.
pub fn ensure_inbox_board(conn: &Connection, workspace_id: i64) -> Result<i64> {
    let existing_board: Option<i64> = conn
        .query_row(
            "SELECT id FROM boards WHERE workspace_id = ?1 AND is_system = 1 LIMIT 1",
            params![workspace_id],
            |row| row.get(0),
        )
        .optional()?;

    let board_id = match existing_board {
        Some(id) => id,
        None => {
            conn.execute(
                "INSERT INTO boards (workspace_id, name, gradient, is_system) VALUES (?1, 'Inbox', 'linear-gradient(135deg, #4b5563 0%, #1f2937 100%)', 1)",
                params![workspace_id],
            )?;
            conn.last_insert_rowid()
        }
    };

    let existing_column: Option<i64> = conn
        .query_row(
            "SELECT id FROM columns WHERE board_id = ?1 LIMIT 1",
            params![board_id],
            |row| row.get(0),
        )
        .optional()?;

    let column_id = match existing_column {
        Some(id) => id,
        None => {
            conn.execute(
                "INSERT INTO columns (board_id, name, position) VALUES (?1, 'Inbox', 0)",
                params![board_id],
            )?;
            conn.last_insert_rowid()
        }
    };

    Ok(column_id)
}
