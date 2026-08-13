use rusqlite::params;
use tauri::State;
use crate::db::DbState;
use crate::models::{Workspace, Board, Column, Card, Notification, UserProfile};

// Error handling helper
type CmdResult<T> = Result<T, String>;

fn to_string_err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

// ─── Workspaces ───

#[tauri::command]
pub fn get_workspaces(state: State<'_, DbState>) -> CmdResult<Vec<Workspace>> {
    let conn = state.conn.lock().unwrap();
    let mut stmt = conn.prepare("SELECT id, name, visibility, created_at, archived FROM workspaces WHERE archived = 0 ORDER BY id ASC").map_err(to_string_err)?;

    let workspaces_iter = stmt.query_map([], |row| {
        Ok(Workspace {
            id: row.get(0)?,
            name: row.get(1)?,
            visibility: row.get(2)?,
            created_at: row.get(3)?,
            archived: row.get(4)?,
        })
    }).map_err(to_string_err)?;

    let mut workspaces = Vec::new();
    for w in workspaces_iter {
        workspaces.push(w.map_err(to_string_err)?);
    }

    Ok(workspaces)
}

#[tauri::command]
pub fn create_workspace(name: String, state: State<'_, DbState>) -> CmdResult<Workspace> {
    let conn = state.conn.lock().unwrap();
    conn.execute(
        "INSERT INTO workspaces (name) VALUES (?1)",
        params![name],
    ).map_err(to_string_err)?;

    let id = conn.last_insert_rowid();

    // Every workspace gets a hidden Inbox board for unsorted quick tasks.
    crate::db::ensure_inbox_board(&conn, id).map_err(to_string_err)?;

    Ok(Workspace {
        id,
        name,
        visibility: "private".to_string(),
        created_at: "".to_string(), // Simplified, normally fetch from DB
        archived: 0,
    })
}

#[tauri::command]
pub fn update_workspace(id: i64, name: String, visibility: String, state: State<'_, DbState>) -> CmdResult<()> {
    let conn = state.conn.lock().unwrap();
    conn.execute(
        "UPDATE workspaces SET name = ?1, visibility = ?2 WHERE id = ?3",
        params![name, visibility, id],
    ).map_err(to_string_err)?;
    Ok(())
}

#[tauri::command]
pub fn archive_workspace(id: i64, state: State<'_, DbState>) -> CmdResult<()> {
    let conn = state.conn.lock().unwrap();
    conn.execute(
        "UPDATE workspaces SET archived = 1 WHERE id = ?1",
        params![id],
    ).map_err(to_string_err)?;
    Ok(())
}

// ─── Boards ───

fn row_to_board(row: &rusqlite::Row) -> rusqlite::Result<Board> {
    Ok(Board {
        id: row.get(0)?,
        workspace_id: row.get(1)?,
        name: row.get(2)?,
        gradient: row.get(3)?,
        is_starred: { let s: i8 = row.get(4)?; s != 0 },
        created_at: row.get(5)?,
        archived: row.get(6)?,
        is_system: { let s: i8 = row.get(7)?; s != 0 },
    })
}

const BOARD_COLUMNS: &str = "id, workspace_id, name, gradient, is_starred, created_at, archived, is_system";

#[tauri::command]
pub fn get_boards(workspace_id: i64, state: State<'_, DbState>) -> CmdResult<Vec<Board>> {
    let conn = state.conn.lock().unwrap();
    let sql = format!("SELECT {} FROM boards WHERE workspace_id = ?1 AND archived = 0 AND is_system = 0 ORDER BY is_starred DESC, id DESC", BOARD_COLUMNS);
    let mut stmt = conn.prepare(&sql).map_err(to_string_err)?;

    let iter = stmt.query_map(params![workspace_id], row_to_board).map_err(to_string_err)?;

    let mut res = Vec::new();
    for i in iter { res.push(i.map_err(to_string_err)?); }
    Ok(res)
}

#[tauri::command]
pub fn get_board(id: i64, state: State<'_, DbState>) -> CmdResult<Board> {
    let conn = state.conn.lock().unwrap();
    let sql = format!("SELECT {} FROM boards WHERE id = ?1", BOARD_COLUMNS);
    let mut stmt = conn.prepare(&sql).map_err(to_string_err)?;

    let board = stmt.query_row(params![id], row_to_board).map_err(to_string_err)?;
    Ok(board)
}

#[tauri::command]
pub fn create_board(workspace_id: i64, name: String, gradient: String, state: State<'_, DbState>) -> CmdResult<Board> {
    let conn = state.conn.lock().unwrap();
    conn.execute(
        "INSERT INTO boards (workspace_id, name, gradient) VALUES (?1, ?2, ?3)",
        params![workspace_id, name, gradient],
    ).map_err(to_string_err)?;

    let id = conn.last_insert_rowid();
    Ok(Board {
        id, workspace_id, name, gradient, is_starred: false, created_at: "".into(), archived: 0, is_system: false
    })
}

#[tauri::command]
pub fn update_board(id: i64, name: String, gradient: String, is_starred: bool, state: State<'_, DbState>) -> CmdResult<()> {
    let conn = state.conn.lock().unwrap();
    conn.execute(
        "UPDATE boards SET name = ?1, gradient = ?2, is_starred = ?3 WHERE id = ?4",
        params![name, gradient, if is_starred { 1 } else { 0 }, id],
    ).map_err(to_string_err)?;
    Ok(())
}

#[tauri::command]
pub fn archive_board(id: i64, state: State<'_, DbState>) -> CmdResult<()> {
    let conn = state.conn.lock().unwrap();
    conn.execute("UPDATE boards SET archived = 1 WHERE id = ?1", params![id]).map_err(to_string_err)?;
    Ok(())
}

#[tauri::command]
pub fn get_archived_boards(workspace_id: i64, state: State<'_, DbState>) -> CmdResult<Vec<Board>> {
    let conn = state.conn.lock().unwrap();
    let sql = format!("SELECT {} FROM boards WHERE workspace_id = ?1 AND archived = 1 AND is_system = 0", BOARD_COLUMNS);
    let mut stmt = conn.prepare(&sql).map_err(to_string_err)?;

    let iter = stmt.query_map(params![workspace_id], row_to_board).map_err(to_string_err)?;

    let mut res = Vec::new();
    for i in iter { res.push(i.map_err(to_string_err)?); }
    Ok(res)
}

#[tauri::command]
pub fn restore_board(id: i64, state: State<'_, DbState>) -> CmdResult<()> {
    let conn = state.conn.lock().unwrap();
    conn.execute("UPDATE boards SET archived = 0 WHERE id = ?1", params![id]).map_err(to_string_err)?;
    Ok(())
}

// ─── Columns ───

#[tauri::command]
pub fn get_columns(board_id: i64, state: State<'_, DbState>) -> CmdResult<Vec<Column>> {
    let conn = state.conn.lock().unwrap();
    let mut stmt = conn.prepare("SELECT id, board_id, name, position, created_at, archived FROM columns WHERE board_id = ?1 AND archived = 0 ORDER BY position ASC").map_err(to_string_err)?;

    let iter = stmt.query_map(params![board_id], |row| {
        Ok(Column {
            id: row.get(0)?,
            board_id: row.get(1)?,
            name: row.get(2)?,
            position: row.get(3)?,
            created_at: row.get(4)?,
            archived: row.get(5)?,
        })
    }).map_err(to_string_err)?;

    let mut res = Vec::new();
    for i in iter { res.push(i.map_err(to_string_err)?); }
    Ok(res)
}

#[tauri::command]
pub fn create_column(board_id: i64, name: String, state: State<'_, DbState>) -> CmdResult<Column> {
    let conn = state.conn.lock().unwrap();

    // Get max position
    let pos: i64 = conn.query_row(
        "SELECT COALESCE(MAX(position), -1) + 1 FROM columns WHERE board_id = ?1",
        params![board_id],
        |row| row.get(0)
    ).unwrap_or(0);

    conn.execute(
        "INSERT INTO columns (board_id, name, position) VALUES (?1, ?2, ?3)",
        params![board_id, name, pos],
    ).map_err(to_string_err)?;

    let id = conn.last_insert_rowid();
    Ok(Column { id, board_id, name, position: pos, created_at: "".into(), archived: 0 })
}

#[tauri::command]
pub fn update_column(id: i64, name: String, state: State<'_, DbState>) -> CmdResult<()> {
    let conn = state.conn.lock().unwrap();
    conn.execute("UPDATE columns SET name = ?1 WHERE id = ?2", params![name, id]).map_err(to_string_err)?;
    Ok(())
}

#[tauri::command]
pub fn archive_column(id: i64, state: State<'_, DbState>) -> CmdResult<()> {
    let conn = state.conn.lock().unwrap();
    conn.execute("UPDATE columns SET archived = 1 WHERE id = ?1", params![id]).map_err(to_string_err)?;
    Ok(())
}

#[tauri::command]
pub fn reorder_columns(board_id: i64, column_ids: Vec<i64>, state: State<'_, DbState>) -> CmdResult<()> {
    let mut conn = state.conn.lock().unwrap();
    let tx = conn.transaction().map_err(to_string_err)?;

    for (i, id) in column_ids.iter().enumerate() {
        tx.execute(
            "UPDATE columns SET position = ?1 WHERE id = ?2 AND board_id = ?3",
            params![i as i64, id, board_id],
        ).map_err(to_string_err)?;
    }

    tx.commit().map_err(to_string_err)?;
    Ok(())
}

// ─── Cards ───

const CARD_COLUMNS: &str = "id, column_id, title, description, position, due_date, created_at, archived, is_mistake, mistake_marked_at, mistake_resolved_at";

fn row_to_card(row: &rusqlite::Row) -> rusqlite::Result<Card> {
    Ok(Card {
        id: row.get(0)?,
        column_id: row.get(1)?,
        title: row.get(2)?,
        description: row.get(3)?,
        position: row.get(4)?,
        due_date: row.get(5)?,
        created_at: row.get(6)?,
        archived: row.get(7)?,
        is_mistake: { let m: i8 = row.get(8)?; m != 0 },
        mistake_marked_at: row.get(9)?,
        mistake_resolved_at: row.get(10)?,
        labels: Vec::new(),
        board_id: None,
        board_name: None,
        column_name: None,
    })
}

/// Same as `row_to_card`, but for queries that additionally join in
/// `boards.id, boards.name, columns.name` as the last three columns
/// (used by the planner and mistake-tracking dashboards).
fn row_to_card_with_board(row: &rusqlite::Row) -> rusqlite::Result<Card> {
    let mut card = row_to_card(row)?;
    card.board_id = row.get(11)?;
    card.board_name = row.get(12)?;
    card.column_name = row.get(13)?;
    Ok(card)
}

#[tauri::command]
pub fn get_cards(column_id: i64, state: State<'_, DbState>) -> CmdResult<Vec<Card>> {
    let conn = state.conn.lock().unwrap();
    let sql = format!("SELECT {} FROM cards WHERE column_id = ?1 AND archived = 0 ORDER BY position ASC", CARD_COLUMNS);
    let mut stmt = conn.prepare(&sql).map_err(to_string_err)?;

    let iter = stmt.query_map(params![column_id], row_to_card).map_err(to_string_err)?;

    let mut res = Vec::new();
    for i in iter { res.push(i.map_err(to_string_err)?); }
    Ok(res)
}

#[tauri::command]
pub fn create_card(column_id: i64, title: String, description: String, state: State<'_, DbState>) -> CmdResult<Card> {
    let conn = state.conn.lock().unwrap();

    let pos: i64 = conn.query_row(
        "SELECT COALESCE(MAX(position), -1) + 1 FROM cards WHERE column_id = ?1",
        params![column_id],
        |row| row.get(0)
    ).unwrap_or(0);

    conn.execute(
        "INSERT INTO cards (column_id, title, description, position) VALUES (?1, ?2, ?3, ?4)",
        params![column_id, title, description, pos],
    ).map_err(to_string_err)?;

    let id = conn.last_insert_rowid();
    Ok(Card {
        id, column_id, title, description, position: pos, due_date: None, created_at: "".into(), archived: 0,
        is_mistake: false, mistake_marked_at: None, mistake_resolved_at: None,
        labels: vec![], board_id: None, board_name: None, column_name: None,
    })
}

#[tauri::command]
pub fn update_card(id: i64, title: String, description: String, due_date: Option<String>, state: State<'_, DbState>) -> CmdResult<()> {
    let conn = state.conn.lock().unwrap();
    conn.execute(
        "UPDATE cards SET title = ?1, description = ?2, due_date = ?3 WHERE id = ?4",
        params![title, description, due_date, id],
    ).map_err(to_string_err)?;
    Ok(())
}

#[tauri::command]
pub fn archive_card(id: i64, state: State<'_, DbState>) -> CmdResult<()> {
    let conn = state.conn.lock().unwrap();
    conn.execute("UPDATE cards SET archived = 1 WHERE id = ?1", params![id]).map_err(to_string_err)?;
    Ok(())
}

/// Moves a card to `new_column_id` at `new_position`, renumbering only the
/// neighboring cards actually affected by the move (not the whole board).
#[tauri::command]
pub fn update_card_position(id: i64, new_column_id: i64, new_position: i64, state: State<'_, DbState>) -> CmdResult<()> {
    let mut conn = state.conn.lock().unwrap();
    let tx = conn.transaction().map_err(to_string_err)?;

    let (old_column_id, old_position): (i64, i64) = tx.query_row(
        "SELECT column_id, position FROM cards WHERE id = ?1",
        params![id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).map_err(to_string_err)?;

    if old_column_id == new_column_id {
        if new_position > old_position {
            // Card moved down within the same column: close the gap it left.
            tx.execute(
                "UPDATE cards SET position = position - 1 WHERE column_id = ?1 AND position > ?2 AND position <= ?3",
                params![old_column_id, old_position, new_position],
            ).map_err(to_string_err)?;
        } else if new_position < old_position {
            // Card moved up within the same column: make room for it.
            tx.execute(
                "UPDATE cards SET position = position + 1 WHERE column_id = ?1 AND position >= ?2 AND position < ?3",
                params![old_column_id, new_position, old_position],
            ).map_err(to_string_err)?;
        }
        tx.execute(
            "UPDATE cards SET position = ?1 WHERE id = ?2",
            params![new_position, id],
        ).map_err(to_string_err)?;
    } else {
        // Close the gap left behind in the source column.
        tx.execute(
            "UPDATE cards SET position = position - 1 WHERE column_id = ?1 AND position > ?2",
            params![old_column_id, old_position],
        ).map_err(to_string_err)?;
        // Make room for the card in the destination column.
        tx.execute(
            "UPDATE cards SET position = position + 1 WHERE column_id = ?1 AND position >= ?2",
            params![new_column_id, new_position],
        ).map_err(to_string_err)?;
        tx.execute(
            "UPDATE cards SET column_id = ?1, position = ?2 WHERE id = ?3",
            params![new_column_id, new_position, id],
        ).map_err(to_string_err)?;
    }

    tx.commit().map_err(to_string_err)?;
    Ok(())
}

#[tauri::command]
pub fn export_board(_board_id: i64, _state: State<'_, DbState>) -> CmdResult<String> {
    // Just a stub for exporting, returning some JSON
    Ok(r#"{"status": "exported"}"#.to_string())
}

// ─── Labels ───

#[tauri::command]
pub fn get_labels(board_id: i64, state: State<'_, DbState>) -> CmdResult<Vec<crate::models::Label>> {
    let conn = state.conn.lock().unwrap();
    let mut stmt = conn.prepare("SELECT id, board_id, name, color FROM labels WHERE board_id = ?1").map_err(to_string_err)?;
    let iter = stmt.query_map(params![board_id], |row| {
        Ok(crate::models::Label {
            id: row.get(0)?,
            board_id: row.get(1)?,
            name: row.get(2)?,
            color: row.get(3)?,
        })
    }).map_err(to_string_err)?;
    let mut res = Vec::new();
    for i in iter { res.push(i.map_err(to_string_err)?); }
    Ok(res)
}

#[tauri::command]
pub fn create_label(board_id: i64, name: String, color: String, state: State<'_, DbState>) -> CmdResult<crate::models::Label> {
    let conn = state.conn.lock().unwrap();
    conn.execute(
        "INSERT INTO labels (board_id, name, color) VALUES (?1, ?2, ?3)",
        params![board_id, name, color],
    ).map_err(to_string_err)?;
    let id = conn.last_insert_rowid();
    Ok(crate::models::Label { id, board_id, name, color })
}

#[tauri::command]
pub fn add_label_to_card(card_id: i64, label_id: i64, state: State<'_, DbState>) -> CmdResult<()> {
    let conn = state.conn.lock().unwrap();
    conn.execute(
        "INSERT OR IGNORE INTO card_labels (card_id, label_id) VALUES (?1, ?2)",
        params![card_id, label_id],
    ).map_err(to_string_err)?;
    Ok(())
}

#[tauri::command]
pub fn remove_label_from_card(card_id: i64, label_id: i64, state: State<'_, DbState>) -> CmdResult<()> {
    let conn = state.conn.lock().unwrap();
    conn.execute(
        "DELETE FROM card_labels WHERE card_id = ?1 AND label_id = ?2",
        params![card_id, label_id],
    ).map_err(to_string_err)?;
    Ok(())
}

// ─── Notifications ───

#[tauri::command]
pub fn get_notifications(state: State<'_, DbState>) -> CmdResult<Vec<Notification>> {
    let conn = state.conn.lock().unwrap();
    let mut stmt = conn.prepare("SELECT id, title, body, created_at, read FROM notifications ORDER BY created_at DESC, id DESC").map_err(to_string_err)?;
    let iter = stmt.query_map([], |row| {
        Ok(Notification {
            id: row.get(0)?,
            title: row.get(1)?,
            body: row.get(2)?,
            created_at: row.get(3)?,
            read: { let r: i8 = row.get(4)?; r != 0 },
        })
    }).map_err(to_string_err)?;
    let mut res = Vec::new();
    for i in iter { res.push(i.map_err(to_string_err)?); }
    Ok(res)
}

#[tauri::command]
pub fn mark_all_notifications_read(state: State<'_, DbState>) -> CmdResult<()> {
    let conn = state.conn.lock().unwrap();
    conn.execute("UPDATE notifications SET read = 1 WHERE read = 0", ()).map_err(to_string_err)?;
    Ok(())
}

// ─── User profile ───

#[tauri::command]
pub fn get_user_profile(state: State<'_, DbState>) -> CmdResult<UserProfile> {
    let conn = state.conn.lock().unwrap();
    let profile = conn.query_row(
        "SELECT avatar_initials, display_name, theme FROM user_profile WHERE id = 1",
        [],
        |row| Ok(UserProfile {
            avatar_initials: row.get(0)?,
            display_name: row.get(1)?,
            theme: row.get(2)?,
        })
    ).map_err(to_string_err)?;
    Ok(profile)
}

#[tauri::command]
pub fn update_user_profile(display_name: String, avatar_initials: String, theme: String, state: State<'_, DbState>) -> CmdResult<()> {
    let conn = state.conn.lock().unwrap();
    conn.execute(
        "UPDATE user_profile SET display_name = ?1, avatar_initials = ?2, theme = ?3 WHERE id = 1",
        params![display_name, avatar_initials, theme],
    ).map_err(to_string_err)?;
    Ok(())
}

// ─── Recently viewed boards ───

#[tauri::command]
pub fn record_board_view(board_id: i64, state: State<'_, DbState>) -> CmdResult<()> {
    let conn = state.conn.lock().unwrap();
    conn.execute(
        "INSERT INTO board_recent_views (board_id) VALUES (?1)",
        params![board_id],
    ).map_err(to_string_err)?;
    Ok(())
}

#[tauri::command]
pub fn get_recent_boards(workspace_id: i64, limit: i64, state: State<'_, DbState>) -> CmdResult<Vec<Board>> {
    let conn = state.conn.lock().unwrap();
    let sql = format!(
        "SELECT b.{cols_prefixed}
         FROM boards b
         INNER JOIN (
             SELECT board_id, MAX(opened_at) AS last_opened
             FROM board_recent_views
             GROUP BY board_id
         ) v ON v.board_id = b.id
         WHERE b.workspace_id = ?1 AND b.archived = 0 AND b.is_system = 0
         ORDER BY v.last_opened DESC
         LIMIT ?2",
        cols_prefixed = BOARD_COLUMNS.split(", ").collect::<Vec<_>>().join(", b.")
    );
    let mut stmt = conn.prepare(&sql).map_err(to_string_err)?;

    let iter = stmt.query_map(params![workspace_id, limit], row_to_board).map_err(to_string_err)?;

    let mut res = Vec::new();
    for i in iter { res.push(i.map_err(to_string_err)?); }
    Ok(res)
}

// ─── Inbox (hidden system board) ───

#[tauri::command]
pub fn get_inbox_column(workspace_id: i64, state: State<'_, DbState>) -> CmdResult<Column> {
    let conn = state.conn.lock().unwrap();
    let column_id = crate::db::ensure_inbox_board(&conn, workspace_id).map_err(to_string_err)?;
    let column = conn.query_row(
        "SELECT id, board_id, name, position, created_at, archived FROM columns WHERE id = ?1",
        params![column_id],
        |row| Ok(Column {
            id: row.get(0)?,
            board_id: row.get(1)?,
            name: row.get(2)?,
            position: row.get(3)?,
            created_at: row.get(4)?,
            archived: row.get(5)?,
        })
    ).map_err(to_string_err)?;
    Ok(column)
}

// ─── Planner ───

#[tauri::command]
pub fn get_cards_with_due_dates(workspace_id: i64, state: State<'_, DbState>) -> CmdResult<Vec<Card>> {
    let conn = state.conn.lock().unwrap();
    let sql = format!(
        "SELECT {cols}, b.id, b.name, col.name
         FROM cards c
         INNER JOIN columns col ON col.id = c.column_id
         INNER JOIN boards b ON b.id = col.board_id
         WHERE b.workspace_id = ?1 AND c.archived = 0 AND c.due_date IS NOT NULL
         ORDER BY c.due_date ASC",
        cols = CARD_COLUMNS.split(", ").map(|c| format!("c.{}", c)).collect::<Vec<_>>().join(", ")
    );
    let mut stmt = conn.prepare(&sql).map_err(to_string_err)?;

    let iter = stmt.query_map(params![workspace_id], row_to_card_with_board).map_err(to_string_err)?;

    let mut res = Vec::new();
    for i in iter { res.push(i.map_err(to_string_err)?); }
    Ok(res)
}

// ─── Mistake tracking ───

#[tauri::command]
pub fn mark_card_mistake(card_id: i64, state: State<'_, DbState>) -> CmdResult<()> {
    let conn = state.conn.lock().unwrap();
    conn.execute(
        "UPDATE cards SET is_mistake = 1, mistake_marked_at = datetime('now'), mistake_resolved_at = NULL WHERE id = ?1",
        params![card_id],
    ).map_err(to_string_err)?;

    let title: String = conn.query_row(
        "SELECT title FROM cards WHERE id = ?1",
        params![card_id],
        |row| row.get(0),
    ).unwrap_or_default();

    conn.execute(
        "INSERT INTO notifications (title, body) VALUES (?1, ?2)",
        params!["Отмечена ошибка", format!("Карточка «{}» помечена как ошибка", title)],
    ).map_err(to_string_err)?;

    Ok(())
}

#[tauri::command]
pub fn resolve_card_mistake(card_id: i64, state: State<'_, DbState>) -> CmdResult<()> {
    let conn = state.conn.lock().unwrap();
    conn.execute(
        "UPDATE cards SET mistake_resolved_at = datetime('now') WHERE id = ?1 AND is_mistake = 1",
        params![card_id],
    ).map_err(to_string_err)?;
    Ok(())
}

#[tauri::command]
pub fn get_mistake_cards(workspace_id: i64, state: State<'_, DbState>) -> CmdResult<Vec<Card>> {
    let conn = state.conn.lock().unwrap();
    let sql = format!(
        "SELECT {cols}, b.id, b.name, col.name
         FROM cards c
         INNER JOIN columns col ON col.id = c.column_id
         INNER JOIN boards b ON b.id = col.board_id
         WHERE b.workspace_id = ?1 AND c.is_mistake = 1
         ORDER BY c.mistake_marked_at DESC",
        cols = CARD_COLUMNS.split(", ").map(|c| format!("c.{}", c)).collect::<Vec<_>>().join(", ")
    );
    let mut stmt = conn.prepare(&sql).map_err(to_string_err)?;

    let iter = stmt.query_map(params![workspace_id], row_to_card_with_board).map_err(to_string_err)?;

    let mut res = Vec::new();
    for i in iter { res.push(i.map_err(to_string_err)?); }
    Ok(res)
}
