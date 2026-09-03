use rusqlite::{Connection, OptionalExtension, Result, params};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// File name of the database inside the app data directory.
pub const DB_FILE_NAME: &str = "trello_clone.db";

/// Sub-directory of the app data directory holding automatic backups.
pub const BACKUP_DIR_NAME: &str = "backups";

/// Sub-directory of the app data directory holding the sidebar background
/// pictures, one per workspace that has one.
pub const BACKGROUNDS_DIR_NAME: &str = "backgrounds";

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

/// Ошибки шифрования приходят строками, а `init` объявлен через
/// `rusqlite::Result`. Тот же приём уже используется в `write_backup`.
fn as_sqlite_error(message: String) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(
        std::io::ErrorKind::Other,
        message,
    )))
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

    // Ключ достаётся до открытия базы: без него открывать нечего. Любая
    // неудача здесь — причина не запуститься, а не работать дальше. Открыть
    // зашифрованную базу с неверным ключом означало бы получить пустую и
    // писать в неё поверх настоящих данных.
    let key = crate::crypto::load_or_create_key().map_err(as_sqlite_error)?;

    // Незашифрованный файл на этом месте — база, созданная версией до
    // шифрования. Переводим её один раз; исходник остаётся рядом под `.bak`.
    if crate::crypto::is_plaintext_database(&db_path) {
        crate::crypto::encrypt_existing_database(&db_path, &key).map_err(as_sqlite_error)?;
    }

    let conn = crate::crypto::open_encrypted(&db_path, &key)?;

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
            archived INTEGER DEFAULT 0,
            background_image_path TEXT
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
            is_final INTEGER DEFAULT 0,
            FOREIGN KEY (board_id) REFERENCES boards(id)
        )",
        (),
    )?;

    // Local directory of people, used only as a label on cards ("исполнитель",
    // "автор"). Deliberately not accounts: no credentials, no per-workspace
    // membership, nothing leaves this file. Must exist before `cards`, which
    // references it.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS members (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            initials TEXT NOT NULL DEFAULT '',
            color TEXT NOT NULL DEFAULT '#6366f1',
            is_self INTEGER NOT NULL DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now'))
        )",
        (),
    )?;

    // Exactly one row may be the user themselves. A partial unique index is the
    // only way SQLite can enforce that; without it a bug elsewhere could quietly
    // produce two "self" members and the profile modal would edit whichever
    // came back first.
    conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_members_single_self
         ON members(is_self) WHERE is_self = 1",
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
            assignee_id INTEGER REFERENCES members(id),
            author_id INTEGER REFERENCES members(id),
            priority TEXT DEFAULT 'Medium' CHECK (priority IN ('Low', 'Medium', 'High')),
            FOREIGN KEY (column_id) REFERENCES columns(id)
        )",
        (),
    )?;

    // Sub-tasks inside a card. Created after `cards`, which it references.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS checklist_items (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            card_id INTEGER NOT NULL,
            text TEXT NOT NULL,
            is_done INTEGER NOT NULL DEFAULT 0,
            position INTEGER NOT NULL DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now')),
            FOREIGN KEY (card_id) REFERENCES cards(id)
        )",
        (),
    )?;

    // Комментарии к карточке. Автор — ссылка на участника, а не текст: имя
    // человека может измениться, и подпись под старым комментарием должна
    // измениться вместе с ним. NULL означает, что участника удалили, —
    // комментарий при этом остаётся, потому что написанное никуда не делось.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS card_comments (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            card_id INTEGER NOT NULL,
            author_id INTEGER,
            body TEXT NOT NULL,
            created_at TEXT DEFAULT (datetime('now')),
            FOREIGN KEY (card_id) REFERENCES cards(id),
            FOREIGN KEY (author_id) REFERENCES members(id)
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
            theme TEXT DEFAULT 'dark',
            due_reminders_enabled INTEGER DEFAULT 1,
            due_reminder_hours INTEGER DEFAULT 24
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
    // Holds a *file name* inside `<app_dir>/backgrounds`, not an absolute path.
    // The app data directory has already moved once (see `LEGACY_APP_DIR`), and
    // a backup restored on another machine would carry a path that no longer
    // exists; a bare name survives both.
    if !table_has_column(conn, "workspaces", "background_image_path") {
        conn.execute("ALTER TABLE workspaces ADD COLUMN background_image_path TEXT", ())?;
    }
    // Финальная колонка — конец пути карточки: попав туда, она уже не
    // возвращается (см. `update_card_position`). Флаг живёт на колонке, а не
    // на карточке, потому что «финальность» — свойство этапа процесса, и
    // задаётся один раз на доску, а не по одной карточке.
    if !table_has_column(conn, "columns", "is_final") {
        conn.execute("ALTER TABLE columns ADD COLUMN is_final INTEGER DEFAULT 0", ())?;
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
    // `ADD COLUMN ... REFERENCES` is only legal while foreign keys are on if the
    // column defaults to NULL — which these do. The flip side is that SQLite
    // offers no way to attach `ON DELETE SET NULL` after the fact, so
    // `delete_member` clears these columns by hand before deleting the row.
    if !table_has_column(conn, "cards", "assignee_id") {
        conn.execute("ALTER TABLE cards ADD COLUMN assignee_id INTEGER REFERENCES members(id)", ())?;
    }
    if !table_has_column(conn, "cards", "author_id") {
        conn.execute("ALTER TABLE cards ADD COLUMN author_id INTEGER REFERENCES members(id)", ())?;
    }
    if !table_has_column(conn, "cards", "priority") {
        // Unlike some databases, SQLite hands existing rows the declared default
        // when a column is added, so every current card becomes 'Medium' without
        // a separate UPDATE.
        conn.execute(
            "ALTER TABLE cards ADD COLUMN priority TEXT DEFAULT 'Medium'
             CHECK (priority IN ('Low', 'Medium', 'High'))",
            (),
        )?;
    }

    // Когда по этой карточке уже показали напоминание о сроке. NULL — ещё не
    // показывали. Хранится в UTC, как и остальные отметки времени в базе; это
    // флаг «сделано», а не то, что читает человек.
    if !table_has_column(conn, "cards", "due_reminder_sent_at") {
        conn.execute("ALTER TABLE cards ADD COLUMN due_reminder_sent_at TEXT", ())?;
    }
    // Настройки напоминаний живут в `user_profile` — той же единственной
    // строке, где уже лежит тема оформления: это настройки приложения, а не
    // свойства человека и не свойства пространства.
    if !table_has_column(conn, "user_profile", "due_reminders_enabled") {
        conn.execute(
            "ALTER TABLE user_profile ADD COLUMN due_reminders_enabled INTEGER DEFAULT 1",
            (),
        )?;
    }
    if !table_has_column(conn, "user_profile", "due_reminder_hours") {
        conn.execute(
            "ALTER TABLE user_profile ADD COLUMN due_reminder_hours INTEGER DEFAULT 24",
            (),
        )?;
    }

    // Ensure the singleton user profile row exists
    conn.execute("INSERT OR IGNORE INTO user_profile (id) VALUES (1)", ())?;

    // ─── user_profile → members ───
    migrate_profile_into_members(conn)?;

    // The workspace-wide card list joins cards → columns → boards for a whole
    // workspace at once; none of those foreign keys had an index before.
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_cards_column     ON cards(column_id);
         CREATE INDEX IF NOT EXISTS idx_cards_assignee   ON cards(assignee_id);
         CREATE INDEX IF NOT EXISTS idx_columns_board    ON columns(board_id);
         CREATE INDEX IF NOT EXISTS idx_boards_workspace ON boards(workspace_id);
         -- Every card on a board asks for its checklist counts, so this one is
         -- read far more often than it is written.
         CREATE INDEX IF NOT EXISTS idx_checklist_card   ON checklist_items(card_id);
         -- Комментарии всегда читаются пачкой по одной карточке.
         CREATE INDEX IF NOT EXISTS idx_comments_card     ON card_comments(card_id);",
    )?;

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

/// Seeds the member directory from the existing single-user profile.
///
/// The profile modal in the header has been storing a name and initials in
/// `user_profile` since long before members existed; those are the user's own,
/// so they become the `is_self` member rather than being asked for a second
/// time.
///
/// `user_profile` is deliberately left in place afterwards. It still owns
/// `theme`, which is an application setting and not a property of a person, and
/// keeping the old name/initials columns untouched means this migration can be
/// re-examined later instead of being a one-way door.
///
/// Runs once: it does nothing at all if an `is_self` member already exists.
fn migrate_profile_into_members(conn: &Connection) -> Result<()> {
    let already_migrated: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM members WHERE is_self = 1)",
        [],
        |row| row.get(0),
    )?;
    if already_migrated {
        return Ok(());
    }

    let (name, initials): (String, String) = conn.query_row(
        "SELECT display_name, avatar_initials FROM user_profile WHERE id = 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    conn.execute(
        "INSERT INTO members (name, initials, color, is_self) VALUES (?1, ?2, ?3, 1)",
        params![name, initials, crate::models::MEMBER_COLORS[0]],
    )?;
    let self_id = conn.last_insert_rowid();

    // Every card that existed before this migration was written by the only
    // person the app has ever had. Backfilling is done here — once, at the
    // moment the self member appears — rather than on every start, so that
    // clearing a card's author later actually sticks.
    conn.execute(
        "UPDATE cards SET author_id = ?1 WHERE author_id IS NULL",
        params![self_id],
    )?;

    log::info!("Профиль перенесён в справочник участников (id = {})", self_id);
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
/// Uses SQLite's own `VACUUM INTO` rather than a file copy: it takes a
/// consistent snapshot even if something else is mid-write, which a plain
/// `fs::copy` cannot promise.
///
/// Раньше здесь был онлайн-API бэкапа (`conn.backup`). С шифрованием он не
/// годится: rusqlite открывает файл-приёмник сам, без ключа, и копия страниц
/// ложится в базу без криптоконтекста. `VACUUM INTO` выполняется тем же
/// соединением с тем же ключом и пишет полноценный зашифрованный файл —
/// открыть его можно только этим приложением на этой учётной записи.
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
    // VACUUM INTO создаёт файл сам и отказывается писать в существующий.
    if dest.exists() {
        fs::remove_file(&dest).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    }
    conn.execute("VACUUM INTO ?1", params![dest.to_string_lossy()])?;
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
