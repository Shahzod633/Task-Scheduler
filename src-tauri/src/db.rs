use rusqlite::{Connection, DatabaseName, OptionalExtension, Result, params};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// File name of the database inside the app data directory.
pub const DB_FILE_NAME: &str = "trello_clone.db";

/// Sub-directory of the app data directory holding automatic backups.
pub const BACKUP_DIR_NAME: &str = "backups";

/// How many automatic backups are kept before the oldest ones are pruned.
const BACKUP_KEEP: usize = 10;

/// Minimum age of the newest backup before another one is taken.
///
/// Without this, restarting the app ten times in an afternoon (entirely normal
/// while developing, and not unusual for a user either) would push every older
/// snapshot out of the ring and leave ten near-identical copies of the last
/// hour. Throttling makes the kept copies span days instead of minutes.
const BACKUP_MIN_INTERVAL_SECS: u64 = 60 * 60;

/// App data directory used before the bundle identifier was changed away from
/// the Tauri template default (`com.tauri.dev`). The identifier decides the
/// path of `app_data_dir()`, so changing it points the app at a fresh, empty
/// directory — an existing database has to be carried over once.
const LEGACY_APP_DIR: &str = "com.tauri.dev";

pub struct DbState {
    pub conn: Mutex<Connection>,
    /// App data directory, kept so commands can reach the backups folder.
    pub app_dir: PathBuf,
}

pub fn init(app_dir: &PathBuf) -> Result<Connection> {
    if !app_dir.exists() {
        fs::create_dir_all(app_dir).expect("Failed to create app data directory");
    }

    let db_path = app_dir.join(DB_FILE_NAME);

    // One-time carry-over from the old template identifier's directory.
    migrate_legacy_db(app_dir, &db_path);

    // Whether real data already exists decides if a backup is worth taking:
    // backing up a database that is about to be created empty is pointless.
    let had_existing_db = db_path.exists();

    let conn = Connection::open(&db_path)?;

    // SQLite ignores FOREIGN KEY clauses unless this is switched on per
    // connection — without it the constraints declared below are decorative.
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;

    if had_existing_db {
        // A failed backup must never stop the app from starting.
        if let Err(e) = write_backup(&conn, app_dir) {
            log::warn!("Не удалось создать резервную копию базы: {}", e);
        }
    }

    create_schema(&conn)?;

    Ok(conn)
}

/// Creates every table and applies the column migrations needed by older
/// databases. Split out of `init` so tests can build the same schema on an
/// in-memory connection without touching the filesystem, backups or the
/// legacy-directory migration.
pub fn create_schema(conn: &Connection) -> Result<()> {
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
    if !table_has_column(conn,"boards", "is_system") {
        conn.execute("ALTER TABLE boards ADD COLUMN is_system INTEGER DEFAULT 0", ())?;
    }
    if !table_has_column(conn,"cards", "is_mistake") {
        conn.execute("ALTER TABLE cards ADD COLUMN is_mistake INTEGER DEFAULT 0", ())?;
    }
    if !table_has_column(conn,"cards", "mistake_marked_at") {
        conn.execute("ALTER TABLE cards ADD COLUMN mistake_marked_at TEXT", ())?;
    }
    if !table_has_column(conn,"cards", "mistake_resolved_at") {
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
            ensure_inbox_board(conn, ws_id)?;
        }
    }

    Ok(())
}

/// Copies the database from the pre-rename app data directory if the current
/// one has none yet. Runs before the connection is opened, so the copy is of a
/// closed file. Any failure is logged and ignored — a missing legacy database
/// is the normal case for new installs.
fn migrate_legacy_db(app_dir: &Path, db_path: &Path) {
    if db_path.exists() {
        return;
    }
    let Some(parent) = app_dir.parent() else { return };
    let legacy_path = parent.join(LEGACY_APP_DIR).join(DB_FILE_NAME);
    if !legacy_path.exists() {
        return;
    }
    match fs::copy(&legacy_path, db_path) {
        Ok(_) => log::info!("База перенесена из старого каталога {:?}", legacy_path),
        Err(e) => log::warn!("Не удалось перенести базу из {:?}: {}", legacy_path, e),
    }
}

/// Writes a timestamped snapshot of the database into `<app_dir>/backups` and
/// prunes all but the newest `BACKUP_KEEP` of them.
///
/// Uses SQLite's own backup API rather than a file copy: it takes a consistent
/// snapshot even if something else is mid-write, which a plain `fs::copy`
/// cannot promise.
fn write_backup(conn: &Connection, app_dir: &Path) -> Result<()> {
    let backup_dir = app_dir.join(BACKUP_DIR_NAME);
    fs::create_dir_all(&backup_dir).map_err(|e| {
        rusqlite::Error::ToSqlConversionFailure(Box::new(e))
    })?;

    if backup_is_recent(&backup_dir) {
        log::info!("Свежая резервная копия уже есть, пропуск");
        return Ok(());
    }

    // Local time, so the file name matches what the user sees on the clock.
    let stamp: String = conn.query_row(
        "SELECT strftime('%Y%m%d-%H%M%S', 'now', 'localtime')",
        [],
        |row| row.get(0),
    )?;

    let dest = backup_dir.join(format!("backup-{}.db", stamp));
    conn.backup(DatabaseName::Main, &dest, None)?;
    log::info!("Резервная копия создана: {:?}", dest);

    prune_backups(&backup_dir);
    Ok(())
}

/// True if a backup was written less than `BACKUP_MIN_INTERVAL_SECS` ago.
///
/// Reads modification time rather than parsing the file name: it is what the
/// filesystem already tracks, and a copied or restored folder still gives a
/// sane answer instead of trusting a possibly renamed file.
fn backup_is_recent(backup_dir: &Path) -> bool {
    let Ok(entries) = fs::read_dir(backup_dir) else { return false };

    let newest = entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_str()
                .is_some_and(|n| n.starts_with("backup-") && n.ends_with(".db"))
        })
        .filter_map(|e| e.metadata().ok())
        .filter_map(|m| m.modified().ok())
        .max();

    match newest {
        Some(time) => time
            .elapsed()
            // An elapsed() error means the file is dated in the future (clock
            // change); treat that as "not recent" and back up anyway.
            .map(|age| age.as_secs() < BACKUP_MIN_INTERVAL_SECS)
            .unwrap_or(false),
        None => false,
    }
}

/// Deletes the oldest backups beyond `BACKUP_KEEP`. The timestamp in the file
/// name is zero-padded and big-endian, so sorting the names sorts by age.
fn prune_backups(backup_dir: &Path) {
    let Ok(entries) = fs::read_dir(backup_dir) else { return };

    let mut backups: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("backup-") && n.ends_with(".db"))
        })
        .collect();

    if backups.len() <= BACKUP_KEEP {
        return;
    }

    backups.sort();
    let excess = backups.len() - BACKUP_KEEP;
    for path in backups.into_iter().take(excess) {
        if let Err(e) = fs::remove_file(&path) {
            log::warn!("Не удалось удалить старую копию {:?}: {}", path, e);
        }
    }
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

#[cfg(test)]
#[path = "db_tests.rs"]
mod tests;
