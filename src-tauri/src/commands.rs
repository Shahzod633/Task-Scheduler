use base64::Engine as _;
use rusqlite::{OptionalExtension, params};
use std::collections::HashMap;
use tauri::State;
use crate::db::DbState;
use crate::models::{
    Workspace, Board, Column, Card, Notification, UserProfile, BackupInfo,
    BoardExport, BoardExportBody, LabelExport, ColumnExport, CardExport, MemberExport, DatabaseExport,
    Member, ChecklistItem, CardRow, BoardColumns, WorkspaceCardList, MEMBER_COLORS, PRIORITIES,
    EXPORT_FORMAT_VERSION,
};

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;

// Error handling helper
type CmdResult<T> = Result<T, String>;

fn to_string_err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

// ─── Workspaces ───

#[tauri::command]
pub fn get_workspaces(state: State<'_, DbState>) -> CmdResult<Vec<Workspace>> {
    let conn = state.conn.lock().unwrap();
    let mut stmt = conn.prepare("SELECT id, name, visibility, created_at, archived, background_image_path FROM workspaces WHERE archived = 0 ORDER BY id ASC").map_err(to_string_err)?;

    let workspaces_iter = stmt.query_map([], |row| {
        Ok(Workspace {
            id: row.get(0)?,
            name: row.get(1)?,
            visibility: row.get(2)?,
            created_at: row.get(3)?,
            archived: row.get(4)?,
            background_image_path: row.get(5)?,
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
        background_image_path: None,
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

const CARD_COLUMNS: &str = "id, column_id, title, description, position, due_date, created_at, archived, is_mistake, mistake_marked_at, mistake_resolved_at, assignee_id, author_id, priority";

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
        assignee_id: row.get(11)?,
        author_id: row.get(12)?,
        // Older rows written before the column existed can still read back NULL
        // if the default was somehow bypassed; the board would then have no
        // priority stripe at all rather than a neutral one.
        priority: row.get::<_, Option<String>>(13)?.unwrap_or_else(|| "Medium".to_string()),
        checklist_total: 0,
        checklist_done: 0,
        labels: Vec::new(),
        board_id: None,
        board_name: None,
        column_name: None,
    })
}

/// Two aggregates appended after `CARD_COLUMNS` by `get_cards`, giving the
/// card face its "2 из 5" without a second query per card.
const CHECKLIST_COUNTS: &str = "
    (SELECT COUNT(*) FROM checklist_items ci WHERE ci.card_id = cards.id),
    (SELECT COUNT(*) FROM checklist_items ci WHERE ci.card_id = cards.id AND ci.is_done = 1)";

/// `row_to_card` plus the two checklist aggregates at positions 14 and 15.
fn row_to_card_with_checklist(row: &rusqlite::Row) -> rusqlite::Result<Card> {
    let mut card = row_to_card(row)?;
    card.checklist_total = row.get(14)?;
    card.checklist_done = row.get(15)?;
    Ok(card)
}

/// Same as `row_to_card`, but for queries that additionally join in
/// `boards.id, boards.name, columns.name` as the last three columns
/// (used by the planner and mistake-tracking dashboards).
fn row_to_card_with_board(row: &rusqlite::Row) -> rusqlite::Result<Card> {
    let mut card = row_to_card(row)?;
    card.board_id = row.get(14)?;
    card.board_name = row.get(15)?;
    card.column_name = row.get(16)?;
    Ok(card)
}

#[tauri::command]
pub fn get_cards(column_id: i64, state: State<'_, DbState>) -> CmdResult<Vec<Card>> {
    let conn = state.conn.lock().unwrap();
    let sql = format!(
        "SELECT {}, {} FROM cards WHERE column_id = ?1 AND archived = 0 ORDER BY position ASC",
        CARD_COLUMNS, CHECKLIST_COUNTS
    );
    let mut stmt = conn.prepare(&sql).map_err(to_string_err)?;

    let iter = stmt.query_map(params![column_id], row_to_card_with_checklist).map_err(to_string_err)?;

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

    // A new card is authored by whoever is using the app. Remains editable —
    // sometimes one person files a task on behalf of another.
    let author_id: Option<i64> = conn
        .query_row("SELECT id FROM members WHERE is_self = 1", [], |row| row.get(0))
        .optional()
        .map_err(to_string_err)?;

    conn.execute(
        "INSERT INTO cards (column_id, title, description, position, author_id) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![column_id, title, description, pos, author_id],
    ).map_err(to_string_err)?;

    let id = conn.last_insert_rowid();
    Ok(Card {
        id, column_id, title, description, position: pos, due_date: None, created_at: "".into(), archived: 0,
        is_mistake: false, mistake_marked_at: None, mistake_resolved_at: None,
        assignee_id: None, author_id, priority: "Medium".into(),
        checklist_total: 0, checklist_done: 0,
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

// ─── Export / import ───

/// Serializes a whole board — columns, cards and labels — into the JSON format
/// described in `models::BoardExport`.
///
/// Archived columns and cards are included and keep their `archived` flag: an
/// export is a full snapshot of the board, not just what happens to be visible.
#[tauri::command]
pub fn export_board(board_id: i64, state: State<'_, DbState>) -> CmdResult<String> {
    let conn = state.conn.lock().unwrap();
    let export = build_board_export(&conn, board_id)?;
    serde_json::to_string_pretty(&export).map_err(to_string_err)
}

/// Same as `export_board`, but writes the JSON to `path` (chosen by the user
/// through the native save dialog on the frontend). Returns the path written,
/// so the UI can show it.
#[tauri::command]
pub fn export_board_to_file(board_id: i64, path: String, state: State<'_, DbState>) -> CmdResult<String> {
    let json = export_board(board_id, state)?;
    std::fs::write(&path, json).map_err(|e| format!("Не удалось записать файл: {}", e))?;
    Ok(path)
}

fn build_board_export(conn: &rusqlite::Connection, board_id: i64) -> CmdResult<BoardExport> {
    let sql = format!("SELECT {} FROM boards WHERE id = ?1", BOARD_COLUMNS);
    let board = conn
        .query_row(&sql, params![board_id], row_to_board)
        .map_err(|e| format!("Доска не найдена: {}", e))?;

    // Label ids are carried over as-is and act as export-local ids that cards
    // point at; the importer remaps them to freshly inserted rows.
    let mut label_stmt = conn
        .prepare("SELECT id, name, color FROM labels WHERE board_id = ?1 ORDER BY id ASC")
        .map_err(to_string_err)?;
    let labels: Vec<LabelExport> = label_stmt
        .query_map(params![board_id], |row| {
            Ok(LabelExport { id: row.get(0)?, name: row.get(1)?, color: row.get(2)? })
        })
        .map_err(to_string_err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(to_string_err)?;

    // Only the members this board actually points at — exporting the whole
    // directory would drag unrelated people into a file about one board.
    let mut member_stmt = conn
        .prepare(
            "SELECT DISTINCT m.id, m.name, m.initials, m.color
             FROM members m
             JOIN cards c ON c.assignee_id = m.id OR c.author_id = m.id
             JOIN columns col ON col.id = c.column_id
             WHERE col.board_id = ?1
             ORDER BY m.id ASC",
        )
        .map_err(to_string_err)?;
    let members: Vec<MemberExport> = member_stmt
        .query_map(params![board_id], |row| {
            Ok(MemberExport {
                id: row.get(0)?,
                name: row.get(1)?,
                initials: row.get(2)?,
                color: row.get(3)?,
            })
        })
        .map_err(to_string_err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(to_string_err)?;

    let mut column_stmt = conn
        .prepare("SELECT id, name, position, archived FROM columns WHERE board_id = ?1 ORDER BY position ASC")
        .map_err(to_string_err)?;
    let raw_columns: Vec<(i64, String, i64, i8)> = column_stmt
        .query_map(params![board_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .map_err(to_string_err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(to_string_err)?;

    let mut card_stmt = conn
        .prepare(
            "SELECT id, title, description, position, due_date, archived,
                    is_mistake, mistake_marked_at, mistake_resolved_at,
                    assignee_id, author_id, priority
             FROM cards WHERE column_id = ?1 ORDER BY position ASC",
        )
        .map_err(to_string_err)?;
    let mut card_label_stmt = conn
        .prepare("SELECT label_id FROM card_labels WHERE card_id = ?1")
        .map_err(to_string_err)?;

    let mut columns = Vec::new();
    for (column_id, name, position, archived) in raw_columns {
        // (card id, card without its labels) — the ids are needed for the
        // card_labels lookup below and then dropped.
        let raw_cards: Vec<(i64, CardExport)> = card_stmt
            .query_map(params![column_id], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    CardExport {
                        title: row.get(1)?,
                        description: row.get(2)?,
                        position: row.get(3)?,
                        due_date: row.get(4)?,
                        archived: row.get(5)?,
                        is_mistake: { let m: i8 = row.get(6)?; m != 0 },
                        mistake_marked_at: row.get(7)?,
                        mistake_resolved_at: row.get(8)?,
                        label_ids: Vec::new(),
                        assignee_id: row.get(9)?,
                        author_id: row.get(10)?,
                        priority: row.get(11)?,
                    },
                ))
            })
            .map_err(to_string_err)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(to_string_err)?;

        let mut cards = Vec::new();
        for (card_id, mut card) in raw_cards {
            card.label_ids = card_label_stmt
                .query_map(params![card_id], |row| row.get(0))
                .map_err(to_string_err)?
                .collect::<rusqlite::Result<Vec<i64>>>()
                .map_err(to_string_err)?;
            cards.push(card);
        }

        columns.push(ColumnExport { name, position, archived, cards });
    }

    let exported_at: String = conn
        .query_row("SELECT datetime('now', 'localtime')", [], |row| row.get(0))
        .unwrap_or_default();

    Ok(BoardExport {
        taskflow_export_version: EXPORT_FORMAT_VERSION,
        exported_at,
        board: BoardExportBody {
            name: board.name,
            gradient: board.gradient,
            is_starred: board.is_starred,
            labels,
            members,
            columns,
        },
    })
}

/// Recreates a board from an export produced by `export_board`.
///
/// Runs as a single transaction: a malformed file leaves no half-imported
/// board behind. Ids from the file are never reused — everything is inserted
/// fresh and label references are remapped through `label_id_map`.
#[tauri::command]
pub fn import_board(workspace_id: i64, json_data: String, state: State<'_, DbState>) -> CmdResult<Board> {
    let export: BoardExport = serde_json::from_str(&json_data)
        .map_err(|e| format!("Файл не похож на экспорт TaskFlow: {}", e))?;

    let mut conn = state.conn.lock().unwrap();
    let board_id = import_board_into(&mut conn, workspace_id, export)?;

    let sql = format!("SELECT {} FROM boards WHERE id = ?1", BOARD_COLUMNS);
    conn.query_row(&sql, params![board_id], row_to_board).map_err(to_string_err)
}

/// The body of `import_board`, split off the Tauri `State` so it can be driven
/// by a plain connection in tests. Returns the id of the new board.
fn import_board_into(conn: &mut rusqlite::Connection, workspace_id: i64, export: BoardExport) -> CmdResult<i64> {
    if export.taskflow_export_version > EXPORT_FORMAT_VERSION {
        return Err(format!(
            "Файл создан более новой версией TaskFlow (формат {}, поддерживается {}). Обновите приложение.",
            export.taskflow_export_version, EXPORT_FORMAT_VERSION
        ));
    }

    let name = export.board.name.trim().to_string();
    if name.is_empty() {
        return Err("В файле не указано название доски".to_string());
    }

    let tx = conn.transaction().map_err(to_string_err)?;

    let gradient = if export.board.gradient.is_empty() {
        "linear-gradient(135deg, #667eea 0%, #764ba2 100%)".to_string()
    } else {
        export.board.gradient
    };

    tx.execute(
        "INSERT INTO boards (workspace_id, name, gradient, is_starred) VALUES (?1, ?2, ?3, ?4)",
        params![workspace_id, name, gradient, export.board.is_starred as i64],
    ).map_err(to_string_err)?;
    let board_id = tx.last_insert_rowid();

    // Export-local label id → newly inserted label id.
    let mut label_id_map: HashMap<i64, i64> = HashMap::new();
    for label in &export.board.labels {
        tx.execute(
            "INSERT INTO labels (board_id, name, color) VALUES (?1, ?2, ?3)",
            params![board_id, label.name, label.color],
        ).map_err(to_string_err)?;
        label_id_map.insert(label.id, tx.last_insert_rowid());
    }

    // Export-local member id → member id in this database.
    //
    // Matched by name rather than id: the same number belongs to a different
    // person in another install. A name that is not in the directory yet is
    // added, so importing a board from someone else brings their people with
    // it instead of silently blanking every assignee.
    let mut member_id_map: HashMap<i64, i64> = HashMap::new();
    for member in &export.board.members {
        let name = member.name.trim();
        if name.is_empty() {
            continue;
        }

        let existing: Option<i64> = tx
            .query_row(
                "SELECT id FROM members WHERE lower(trim(name)) = lower(?1) LIMIT 1",
                params![name],
                |row| row.get(0),
            )
            .optional()
            .map_err(to_string_err)?;

        let new_id = match existing {
            Some(id) => id,
            None => {
                let initials = if member.initials.trim().is_empty() {
                    default_initials(name)
                } else {
                    member.initials.trim().to_uppercase()
                };
                let color = if member.color.trim().is_empty() {
                    next_member_color(&tx).map_err(to_string_err)?
                } else {
                    member.color.clone()
                };
                tx.execute(
                    "INSERT INTO members (name, initials, color, is_self) VALUES (?1, ?2, ?3, 0)",
                    params![name, initials, color],
                ).map_err(to_string_err)?;
                tx.last_insert_rowid()
            }
        };
        member_id_map.insert(member.id, new_id);
    }

    for (col_index, column) in export.board.columns.iter().enumerate() {
        tx.execute(
            "INSERT INTO columns (board_id, name, position, archived) VALUES (?1, ?2, ?3, ?4)",
            params![board_id, column.name, col_index as i64, column.archived],
        ).map_err(to_string_err)?;
        let column_id = tx.last_insert_rowid();

        for (card_index, card) in column.cards.iter().enumerate() {
            // A member id missing from the file's own directory is dropped
            // rather than failing the import — same rule as labels.
            let assignee_id = card.assignee_id.and_then(|id| member_id_map.get(&id).copied());
            let author_id = card.author_id.and_then(|id| member_id_map.get(&id).copied());
            let priority = normalize_priority(card.priority.as_deref());

            tx.execute(
                "INSERT INTO cards (column_id, title, description, position, due_date, archived,
                                    is_mistake, mistake_marked_at, mistake_resolved_at,
                                    assignee_id, author_id, priority)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    column_id, card.title, card.description, card_index as i64, card.due_date,
                    card.archived, card.is_mistake as i64, card.mistake_marked_at, card.mistake_resolved_at,
                    assignee_id, author_id, priority
                ],
            ).map_err(to_string_err)?;
            let card_id = tx.last_insert_rowid();

            for old_label_id in &card.label_ids {
                // A card referencing a label missing from the file is skipped
                // rather than failing the whole import.
                if let Some(new_label_id) = label_id_map.get(old_label_id) {
                    tx.execute(
                        "INSERT OR IGNORE INTO card_labels (card_id, label_id) VALUES (?1, ?2)",
                        params![card_id, new_label_id],
                    ).map_err(to_string_err)?;
                }
            }
        }
    }

    tx.commit().map_err(to_string_err)?;
    Ok(board_id)
}

/// Reads a `.json` export from disk and imports it. Path comes from the native
/// open dialog on the frontend.
#[tauri::command]
pub fn import_board_from_file(workspace_id: i64, path: String, state: State<'_, DbState>) -> CmdResult<Board> {
    let json = std::fs::read_to_string(&path)
        .map_err(|e| format!("Не удалось прочитать файл: {}", e))?;
    import_board(workspace_id, json, state)
}

// ─── Checklists (sub-tasks inside a card) ───

fn row_to_checklist_item(row: &rusqlite::Row) -> rusqlite::Result<ChecklistItem> {
    Ok(ChecklistItem {
        id: row.get(0)?,
        card_id: row.get(1)?,
        text: row.get(2)?,
        is_done: { let d: i64 = row.get(3)?; d != 0 },
        position: row.get(4)?,
    })
}

const CHECKLIST_COLUMNS: &str = "id, card_id, text, is_done, position";

#[tauri::command]
pub fn list_checklist_items(card_id: i64, state: State<'_, DbState>) -> CmdResult<Vec<ChecklistItem>> {
    let conn = state.conn.lock().unwrap();
    // `id` breaks ties: without drag-reordering every item shares position 0
    // only if something went wrong, but a stable order still beats an arbitrary
    // one that shuffles between openings.
    let sql = format!(
        "SELECT {} FROM checklist_items WHERE card_id = ?1 ORDER BY position ASC, id ASC",
        CHECKLIST_COLUMNS
    );
    let mut stmt = conn.prepare(&sql).map_err(to_string_err)?;
    let iter = stmt.query_map(params![card_id], row_to_checklist_item).map_err(to_string_err)?;

    let mut res = Vec::new();
    for i in iter { res.push(i.map_err(to_string_err)?); }
    Ok(res)
}

#[tauri::command]
pub fn create_checklist_item(card_id: i64, text: String, state: State<'_, DbState>) -> CmdResult<ChecklistItem> {
    let conn = state.conn.lock().unwrap();

    let text = text.trim().to_string();
    if text.is_empty() {
        return Err("Пункт чек-листа не может быть пустым".to_string());
    }

    let position: i64 = conn.query_row(
        "SELECT COALESCE(MAX(position), -1) + 1 FROM checklist_items WHERE card_id = ?1",
        params![card_id],
        |row| row.get(0),
    ).unwrap_or(0);

    conn.execute(
        "INSERT INTO checklist_items (card_id, text, position) VALUES (?1, ?2, ?3)",
        params![card_id, text, position],
    ).map_err(to_string_err)?;

    Ok(ChecklistItem {
        id: conn.last_insert_rowid(),
        card_id,
        text,
        is_done: false,
        position,
    })
}

/// Flips one item and returns its new state, so the caller does not have to
/// re-read the list to know what happened.
#[tauri::command]
pub fn toggle_checklist_item(id: i64, state: State<'_, DbState>) -> CmdResult<bool> {
    let conn = state.conn.lock().unwrap();

    let changed = conn.execute(
        "UPDATE checklist_items SET is_done = CASE is_done WHEN 0 THEN 1 ELSE 0 END WHERE id = ?1",
        params![id],
    ).map_err(to_string_err)?;
    if changed == 0 {
        return Err("Пункт чек-листа не найден".to_string());
    }

    let is_done: i64 = conn
        .query_row("SELECT is_done FROM checklist_items WHERE id = ?1", params![id], |row| row.get(0))
        .map_err(to_string_err)?;
    Ok(is_done != 0)
}

#[tauri::command]
pub fn delete_checklist_item(id: i64, state: State<'_, DbState>) -> CmdResult<()> {
    let conn = state.conn.lock().unwrap();
    conn.execute("DELETE FROM checklist_items WHERE id = ?1", params![id])
        .map_err(to_string_err)?;
    Ok(())
}

// ─── Members ───
//
// A local directory of names used as card labels ("исполнитель", "автор").
// Not accounts: nothing here authenticates anyone or leaves the machine.

fn row_to_member(row: &rusqlite::Row) -> rusqlite::Result<Member> {
    Ok(Member {
        id: row.get(0)?,
        name: row.get(1)?,
        initials: row.get(2)?,
        color: row.get(3)?,
        is_self: { let s: i64 = row.get(4)?; s != 0 },
        created_at: row.get(5)?,
    })
}

const MEMBER_COLUMNS: &str = "id, name, initials, color, is_self, created_at";

/// First two letters of the first two words — "Иван Петров" → "ИП", a
/// single-word name → its first two letters.
fn default_initials(name: &str) -> String {
    let words: Vec<&str> = name.split_whitespace().collect();
    let initials: String = match words.len() {
        0 => String::new(),
        1 => words[0].chars().take(2).collect(),
        _ => words
            .iter()
            .take(2)
            .filter_map(|w| w.chars().next())
            .collect(),
    };
    initials.to_uppercase()
}

/// Next colour in the fixed palette, chosen by how many members already exist
/// so consecutive additions never land on the same colour.
fn next_member_color(conn: &rusqlite::Connection) -> rusqlite::Result<String> {
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM members", [], |row| row.get(0))?;
    Ok(MEMBER_COLORS[(count as usize) % MEMBER_COLORS.len()].to_string())
}

/// Maps anything to a value the schema's CHECK constraint accepts, so a
/// malformed import or a stale frontend cannot produce a card the database
/// refuses to store.
fn normalize_priority(value: Option<&str>) -> String {
    match value {
        Some(v) if PRIORITIES.contains(&v) => v.to_string(),
        _ => "Medium".to_string(),
    }
}

#[tauri::command]
pub fn list_members(state: State<'_, DbState>) -> CmdResult<Vec<Member>> {
    let conn = state.conn.lock().unwrap();
    // The user first, then everyone else in the order they were added.
    let sql = format!("SELECT {} FROM members ORDER BY is_self DESC, id ASC", MEMBER_COLUMNS);
    let mut stmt = conn.prepare(&sql).map_err(to_string_err)?;
    let iter = stmt.query_map([], row_to_member).map_err(to_string_err)?;

    let mut res = Vec::new();
    for m in iter { res.push(m.map_err(to_string_err)?); }
    Ok(res)
}

#[tauri::command]
pub fn create_member(name: String, state: State<'_, DbState>) -> CmdResult<Member> {
    let conn = state.conn.lock().unwrap();

    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("Имя участника не может быть пустым".to_string());
    }

    let initials = default_initials(&name);
    let color = next_member_color(&conn).map_err(to_string_err)?;

    conn.execute(
        "INSERT INTO members (name, initials, color, is_self) VALUES (?1, ?2, ?3, 0)",
        params![name, initials, color],
    ).map_err(to_string_err)?;

    let id = conn.last_insert_rowid();
    let sql = format!("SELECT {} FROM members WHERE id = ?1", MEMBER_COLUMNS);
    conn.query_row(&sql, params![id], row_to_member).map_err(to_string_err)
}

/// Renames a member and/or changes their avatar colour.
///
/// `initials` is optional: passing `None` re-derives them from the new name,
/// which is what the members screen wants, while the profile form sends the two
/// letters the user typed.
#[tauri::command]
pub fn update_member(
    id: i64,
    name: String,
    color: String,
    initials: Option<String>,
    state: State<'_, DbState>,
) -> CmdResult<()> {
    let conn = state.conn.lock().unwrap();

    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("Имя участника не может быть пустым".to_string());
    }

    let initials = match initials {
        Some(raw) if !raw.trim().is_empty() => raw.trim().chars().take(2).collect::<String>().to_uppercase(),
        _ => default_initials(&name),
    };

    conn.execute(
        "UPDATE members SET name = ?1, initials = ?2, color = ?3 WHERE id = ?4",
        params![name, initials, color, id],
    ).map_err(to_string_err)?;
    Ok(())
}

/// Removes a member from the directory.
///
/// Foreign keys are enforced, and `ALTER TABLE` gave no way to declare
/// `ON DELETE SET NULL` on the two columns pointing here — so the references
/// are cleared explicitly first, inside the same transaction as the delete.
/// Without that, deleting anyone who has ever been assigned a card fails with a
/// constraint error.
#[tauri::command]
pub fn delete_member(id: i64, state: State<'_, DbState>) -> CmdResult<()> {
    let mut conn = state.conn.lock().unwrap();
    delete_member_from(&mut conn, id)
}

fn delete_member_from(conn: &mut rusqlite::Connection, id: i64) -> CmdResult<()> {
    let is_self: bool = conn
        .query_row("SELECT is_self FROM members WHERE id = ?1", params![id], |row| {
            let s: i64 = row.get(0)?;
            Ok(s != 0)
        })
        .optional()
        .map_err(to_string_err)?
        .ok_or_else(|| "Участник не найден".to_string())?;

    if is_self {
        return Err("Нельзя удалить себя из списка участников".to_string());
    }

    let tx = conn.transaction().map_err(to_string_err)?;
    tx.execute("UPDATE cards SET assignee_id = NULL WHERE assignee_id = ?1", params![id])
        .map_err(to_string_err)?;
    tx.execute("UPDATE cards SET author_id = NULL WHERE author_id = ?1", params![id])
        .map_err(to_string_err)?;
    tx.execute("DELETE FROM members WHERE id = ?1", params![id])
        .map_err(to_string_err)?;
    tx.commit().map_err(to_string_err)?;
    Ok(())
}

// ─── Card assignment / priority ───

#[tauri::command]
pub fn update_card_assignee(card_id: i64, member_id: Option<i64>, state: State<'_, DbState>) -> CmdResult<()> {
    let conn = state.conn.lock().unwrap();
    conn.execute(
        "UPDATE cards SET assignee_id = ?1 WHERE id = ?2",
        params![member_id, card_id],
    ).map_err(to_string_err)?;
    Ok(())
}

#[tauri::command]
pub fn update_card_author(card_id: i64, member_id: Option<i64>, state: State<'_, DbState>) -> CmdResult<()> {
    let conn = state.conn.lock().unwrap();
    conn.execute(
        "UPDATE cards SET author_id = ?1 WHERE id = ?2",
        params![member_id, card_id],
    ).map_err(to_string_err)?;
    Ok(())
}

#[tauri::command]
pub fn update_card_priority(card_id: i64, priority: String, state: State<'_, DbState>) -> CmdResult<()> {
    let conn = state.conn.lock().unwrap();
    conn.execute(
        "UPDATE cards SET priority = ?1 WHERE id = ?2",
        params![normalize_priority(Some(priority.as_str())), card_id],
    ).map_err(to_string_err)?;
    Ok(())
}

// ─── Workspace-wide card list (the "Список" screen) ───

/// Every non-archived card in the workspace, across all of its boards, plus the
/// boards' column lists.
///
/// Two queries in total, regardless of how many boards exist: the naive shape
/// of this screen — fetch boards, then columns per board, then cards per column
/// — would be N+1 twice over. The member records are joined in rather than
/// looked up per row for the same reason.
#[tauri::command]
pub fn list_all_cards_in_workspace(workspace_id: i64, state: State<'_, DbState>) -> CmdResult<WorkspaceCardList> {
    let conn = state.conn.lock().unwrap();
    build_workspace_card_list(&conn, workspace_id)
}

/// The body of `list_all_cards_in_workspace`, split off the Tauri `State` so it
/// can be driven by a plain connection in tests.
fn build_workspace_card_list(conn: &rusqlite::Connection, workspace_id: i64) -> CmdResult<WorkspaceCardList> {
    let mut card_stmt = conn.prepare(
        "SELECT c.id, c.title, c.description, c.position, c.due_date,
                COALESCE(c.priority, 'Medium'), c.created_at, c.is_mistake,
                col.id, col.name,
                b.id, b.name, b.is_system,
                a.id, a.name, a.initials, a.color, a.is_self, a.created_at,
                w.id, w.name, w.initials, w.color, w.is_self, w.created_at
         FROM cards c
         INNER JOIN columns col ON col.id = c.column_id
         INNER JOIN boards b ON b.id = col.board_id
         LEFT JOIN members a ON a.id = c.assignee_id
         LEFT JOIN members w ON w.id = c.author_id
         WHERE b.workspace_id = ?1
           AND c.archived = 0 AND col.archived = 0 AND b.archived = 0
         ORDER BY b.name ASC, col.position ASC, c.position ASC",
    ).map_err(to_string_err)?;

    let card_iter = card_stmt.query_map(params![workspace_id], |row| {
        // A LEFT JOIN with no match gives NULL in every joined column, so the
        // member id being NULL is what decides whether there is one at all.
        let assignee = match row.get::<_, Option<i64>>(13)? {
            Some(id) => Some(Member {
                id,
                name: row.get(14)?,
                initials: row.get(15)?,
                color: row.get(16)?,
                is_self: { let s: i64 = row.get(17)?; s != 0 },
                created_at: row.get(18)?,
            }),
            None => None,
        };
        let author = match row.get::<_, Option<i64>>(19)? {
            Some(id) => Some(Member {
                id,
                name: row.get(20)?,
                initials: row.get(21)?,
                color: row.get(22)?,
                is_self: { let s: i64 = row.get(23)?; s != 0 },
                created_at: row.get(24)?,
            }),
            None => None,
        };

        Ok(CardRow {
            id: row.get(0)?,
            title: row.get(1)?,
            description: row.get(2)?,
            position: row.get(3)?,
            due_date: row.get(4)?,
            priority: row.get(5)?,
            created_at: row.get(6)?,
            is_mistake: { let m: i8 = row.get(7)?; m != 0 },
            column_id: row.get(8)?,
            column_name: row.get(9)?,
            board_id: row.get(10)?,
            board_name: row.get(11)?,
            board_is_system: { let s: i64 = row.get(12)?; s != 0 },
            assignee,
            author,
        })
    }).map_err(to_string_err)?;

    let mut cards = Vec::new();
    for c in card_iter { cards.push(c.map_err(to_string_err)?); }

    // Boards with their columns, so each row's status dropdown can offer the
    // columns of its own board.
    let mut col_stmt = conn.prepare(
        "SELECT b.id, b.name, b.is_system,
                col.id, col.board_id, col.name, col.position, col.created_at, col.archived
         FROM boards b
         LEFT JOIN columns col ON col.board_id = b.id AND col.archived = 0
         WHERE b.workspace_id = ?1 AND b.archived = 0
         ORDER BY b.name ASC, col.position ASC",
    ).map_err(to_string_err)?;

    let rows = col_stmt.query_map(params![workspace_id], |row| {
        let column = match row.get::<_, Option<i64>>(3)? {
            Some(id) => Some(Column {
                id,
                board_id: row.get(4)?,
                name: row.get(5)?,
                position: row.get(6)?,
                created_at: row.get(7)?,
                archived: row.get(8)?,
            }),
            None => None,
        };
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, { let s: i64 = row.get(2)?; s != 0 }, column))
    }).map_err(to_string_err)?;

    let mut boards: Vec<BoardColumns> = Vec::new();
    for row in rows {
        let (board_id, board_name, is_system, column) = row.map_err(to_string_err)?;
        // Rows arrive grouped by board, so only the last entry can match.
        if boards.last().map(|b| b.id) != Some(board_id) {
            boards.push(BoardColumns { id: board_id, name: board_name, is_system, columns: Vec::new() });
        }
        if let Some(col) = column {
            boards.last_mut().unwrap().columns.push(col);
        }
    }

    Ok(WorkspaceCardList { cards, boards })
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

// The name and initials now live on the `is_self` row of `members`, so the
// profile form and the assignee dropdowns can never disagree about who the user
// is. `theme` stays in `user_profile`: it is a setting of the application, not
// a property of a person. The command names and signatures are unchanged, so
// the header modal and the Settings page needed no rewiring.

#[tauri::command]
pub fn get_user_profile(state: State<'_, DbState>) -> CmdResult<UserProfile> {
    let conn = state.conn.lock().unwrap();

    let theme: String = conn
        .query_row("SELECT theme FROM user_profile WHERE id = 1", [], |row| row.get(0))
        .map_err(to_string_err)?;

    let (display_name, avatar_initials): (String, String) = conn
        .query_row(
            "SELECT name, initials FROM members WHERE is_self = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(to_string_err)?;

    Ok(UserProfile { avatar_initials, display_name, theme })
}

#[tauri::command]
pub fn update_user_profile(display_name: String, avatar_initials: String, theme: String, state: State<'_, DbState>) -> CmdResult<()> {
    let conn = state.conn.lock().unwrap();

    conn.execute("UPDATE user_profile SET theme = ?1 WHERE id = 1", params![theme])
        .map_err(to_string_err)?;

    conn.execute(
        "UPDATE members SET name = ?1, initials = ?2 WHERE is_self = 1",
        params![display_name, avatar_initials],
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

// ─── Archive (restore) and permanent deletion ───
//
// The archive doubles as the trash: archiving hides an item and keeps it
// recoverable, deleting removes it for good. Deletion is ordered children-first
// because `PRAGMA foreign_keys` is now on and would otherwise reject the parent
// row while references still exist.

#[tauri::command]
pub fn get_archived_columns(board_id: i64, state: State<'_, DbState>) -> CmdResult<Vec<Column>> {
    let conn = state.conn.lock().unwrap();
    let mut stmt = conn.prepare(
        "SELECT id, board_id, name, position, created_at, archived
         FROM columns WHERE board_id = ?1 AND archived = 1 ORDER BY id DESC"
    ).map_err(to_string_err)?;

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

/// Archived cards of a board, newest first, each carrying the name of the
/// column it came from so the archive screen can show where it will return to.
#[tauri::command]
pub fn get_archived_cards(board_id: i64, state: State<'_, DbState>) -> CmdResult<Vec<Card>> {
    let conn = state.conn.lock().unwrap();
    let sql = format!(
        "SELECT {cols}, b.id, b.name, col.name
         FROM cards c
         INNER JOIN columns col ON col.id = c.column_id
         INNER JOIN boards b ON b.id = col.board_id
         WHERE b.id = ?1 AND c.archived = 1
         ORDER BY c.id DESC",
        cols = CARD_COLUMNS.split(", ").map(|c| format!("c.{}", c)).collect::<Vec<_>>().join(", ")
    );
    let mut stmt = conn.prepare(&sql).map_err(to_string_err)?;

    let iter = stmt.query_map(params![board_id], row_to_card_with_board).map_err(to_string_err)?;

    let mut res = Vec::new();
    for i in iter { res.push(i.map_err(to_string_err)?); }
    Ok(res)
}

/// Un-archives a card and appends it to the end of its column.
///
/// The old `position` is deliberately not reused: the column has moved on since
/// the card left it, and restoring into an occupied slot produces duplicate
/// positions. If the column itself was archived meanwhile it is restored too —
/// otherwise the card would come back into a place the user cannot see, and the
/// button would look broken.
#[tauri::command]
pub fn restore_card(id: i64, state: State<'_, DbState>) -> CmdResult<()> {
    let mut conn = state.conn.lock().unwrap();
    let tx = conn.transaction().map_err(to_string_err)?;

    let column_id: i64 = tx.query_row(
        "SELECT column_id FROM cards WHERE id = ?1",
        params![id],
        |row| row.get(0),
    ).map_err(|e| format!("Карточка не найдена: {}", e))?;

    tx.execute("UPDATE columns SET archived = 0 WHERE id = ?1", params![column_id])
        .map_err(to_string_err)?;

    let position: i64 = tx.query_row(
        "SELECT COALESCE(MAX(position), -1) + 1 FROM cards WHERE column_id = ?1 AND archived = 0",
        params![column_id],
        |row| row.get(0),
    ).unwrap_or(0);

    tx.execute(
        "UPDATE cards SET archived = 0, position = ?1 WHERE id = ?2",
        params![position, id],
    ).map_err(to_string_err)?;

    tx.commit().map_err(to_string_err)?;
    Ok(())
}

/// Un-archives a column and appends it to the right-hand end of the board.
#[tauri::command]
pub fn restore_column(id: i64, state: State<'_, DbState>) -> CmdResult<()> {
    let mut conn = state.conn.lock().unwrap();
    let tx = conn.transaction().map_err(to_string_err)?;

    let board_id: i64 = tx.query_row(
        "SELECT board_id FROM columns WHERE id = ?1",
        params![id],
        |row| row.get(0),
    ).map_err(|e| format!("Колонка не найдена: {}", e))?;

    let position: i64 = tx.query_row(
        "SELECT COALESCE(MAX(position), -1) + 1 FROM columns WHERE board_id = ?1 AND archived = 0",
        params![board_id],
        |row| row.get(0),
    ).unwrap_or(0);

    tx.execute(
        "UPDATE columns SET archived = 0, position = ?1 WHERE id = ?2",
        params![position, id],
    ).map_err(to_string_err)?;

    tx.commit().map_err(to_string_err)?;
    Ok(())
}

#[tauri::command]
pub fn delete_card(id: i64, state: State<'_, DbState>) -> CmdResult<()> {
    let mut conn = state.conn.lock().unwrap();
    let tx = conn.transaction().map_err(to_string_err)?;
    tx.execute("DELETE FROM card_labels WHERE card_id = ?1", params![id]).map_err(to_string_err)?;
    tx.execute("DELETE FROM checklist_items WHERE card_id = ?1", params![id]).map_err(to_string_err)?;
    tx.execute("DELETE FROM cards WHERE id = ?1", params![id]).map_err(to_string_err)?;
    tx.commit().map_err(to_string_err)?;
    Ok(())
}

/// Deletes a column together with every card in it — including cards that were
/// archived separately, which would otherwise become unreachable orphans.
#[tauri::command]
pub fn delete_column(id: i64, state: State<'_, DbState>) -> CmdResult<()> {
    let mut conn = state.conn.lock().unwrap();
    let tx = conn.transaction().map_err(to_string_err)?;
    tx.execute(
        "DELETE FROM card_labels WHERE card_id IN (SELECT id FROM cards WHERE column_id = ?1)",
        params![id],
    ).map_err(to_string_err)?;
    tx.execute(
        "DELETE FROM checklist_items WHERE card_id IN (SELECT id FROM cards WHERE column_id = ?1)",
        params![id],
    ).map_err(to_string_err)?;
    tx.execute("DELETE FROM cards WHERE column_id = ?1", params![id]).map_err(to_string_err)?;
    tx.execute("DELETE FROM columns WHERE id = ?1", params![id]).map_err(to_string_err)?;
    tx.commit().map_err(to_string_err)?;
    Ok(())
}

/// Deletes a board and everything hanging off it: cards, columns, labels and
/// its entries in the recently-viewed list.
#[tauri::command]
pub fn delete_board(id: i64, state: State<'_, DbState>) -> CmdResult<()> {
    let mut conn = state.conn.lock().unwrap();

    // The hidden Inbox board backs the Inbox screen; deleting it would break
    // that screen for the whole workspace.
    let is_system: i64 = conn.query_row(
        "SELECT is_system FROM boards WHERE id = ?1",
        params![id],
        |row| row.get(0),
    ).map_err(|e| format!("Доска не найдена: {}", e))?;
    if is_system != 0 {
        return Err("Служебную доску Inbox удалить нельзя".to_string());
    }

    let tx = conn.transaction().map_err(to_string_err)?;
    tx.execute(
        "DELETE FROM card_labels WHERE card_id IN (
            SELECT c.id FROM cards c
            INNER JOIN columns col ON col.id = c.column_id
            WHERE col.board_id = ?1
        )",
        params![id],
    ).map_err(to_string_err)?;
    tx.execute(
        "DELETE FROM checklist_items WHERE card_id IN (
            SELECT c.id FROM cards c
            INNER JOIN columns col ON col.id = c.column_id
            WHERE col.board_id = ?1
        )",
        params![id],
    ).map_err(to_string_err)?;
    tx.execute(
        "DELETE FROM cards WHERE column_id IN (SELECT id FROM columns WHERE board_id = ?1)",
        params![id],
    ).map_err(to_string_err)?;
    tx.execute("DELETE FROM columns WHERE board_id = ?1", params![id]).map_err(to_string_err)?;
    tx.execute("DELETE FROM labels WHERE board_id = ?1", params![id]).map_err(to_string_err)?;
    tx.execute("DELETE FROM board_recent_views WHERE board_id = ?1", params![id]).map_err(to_string_err)?;
    tx.execute("DELETE FROM boards WHERE id = ?1", params![id]).map_err(to_string_err)?;
    tx.commit().map_err(to_string_err)?;
    Ok(())
}

// ─── Backups ───

/// Lists the automatic backups written at startup, newest first.
#[tauri::command]
pub fn get_backups(state: State<'_, DbState>) -> CmdResult<Vec<BackupInfo>> {
    let backup_dir = state.app_dir.join(crate::db::BACKUP_DIR_NAME);
    let Ok(entries) = std::fs::read_dir(&backup_dir) else { return Ok(Vec::new()) };

    let mut backups: Vec<BackupInfo> = entries
        .filter_map(|e| e.ok())
        .filter_map(|entry| {
            let file_name = entry.file_name().to_string_lossy().into_owned();
            if !(file_name.starts_with("backup-") && file_name.ends_with(".db")) {
                return None;
            }
            let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
            Some(BackupInfo {
                created_at: parse_backup_stamp(&file_name),
                file_name,
                size_bytes,
            })
        })
        .collect();

    // File names embed a zero-padded timestamp, so sorting names sorts by age.
    backups.sort_by(|a, b| b.file_name.cmp(&a.file_name));
    Ok(backups)
}

/// Turns `backup-20260817-143012.db` into `17.08.2026 14:30:12`.
/// Falls back to the raw name if it does not match that shape.
fn parse_backup_stamp(file_name: &str) -> String {
    let stem = file_name
        .strip_prefix("backup-")
        .and_then(|s| s.strip_suffix(".db"))
        .unwrap_or("");
    let (date, time) = match stem.split_once('-') {
        Some(parts) => parts,
        None => return file_name.to_string(),
    };
    if date.len() != 8 || time.len() != 6 {
        return file_name.to_string();
    }
    format!(
        "{}.{}.{} {}:{}:{}",
        &date[6..8], &date[4..6], &date[0..4],
        &time[0..2], &time[2..4], &time[4..6]
    )
}

#[tauri::command]
pub fn get_backup_dir(state: State<'_, DbState>) -> CmdResult<String> {
    Ok(state.app_dir.join(crate::db::BACKUP_DIR_NAME).to_string_lossy().into_owned())
}

/// Opens the backups folder in the system file manager.
#[tauri::command]
pub fn open_backup_dir(state: State<'_, DbState>) -> CmdResult<()> {
    let dir = state.app_dir.join(crate::db::BACKUP_DIR_NAME);
    std::fs::create_dir_all(&dir).map_err(|e| format!("Папка недоступна: {}", e))?;

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&dir)
            // explorer.exe reports a non-zero exit code even on success, so the
            // handle is dropped without checking it.
            .spawn()
            .map_err(|e| format!("Не удалось открыть папку: {}", e))?;
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    Err("Открытие папки поддерживается только в Windows-сборке".to_string())
}

// ─── Full database export ───

/// Writes a consistent snapshot of the whole database to `path`.
///
/// Uses SQLite's own `VACUUM INTO` rather than copying `trello_clone.db` off
/// the filesystem. A plain file copy captures whatever bytes happen to be on
/// disk at that instant, which is not necessarily a valid database: pages the
/// open connection has not flushed yet, and any rollback/WAL journal, live
/// outside that one file. `VACUUM INTO` asks SQLite to write a fresh, complete,
/// self-contained copy — the same guarantee the automatic backups get from the
/// online backup API.
///
/// The snapshot goes to a `.part` file first and is opened and read back before
/// it replaces anything at `path`. That way a failure part-way through cannot
/// destroy a file the user already had there.
#[tauri::command]
pub fn export_database(path: String, state: State<'_, DbState>) -> CmdResult<DatabaseExport> {
    let conn = state.conn.lock().unwrap();
    export_database_to(&conn, std::path::Path::new(&path))
}

fn export_database_to(conn: &rusqlite::Connection, target: &std::path::Path) -> CmdResult<DatabaseExport> {
    let mut partial = target.as_os_str().to_os_string();
    partial.push(".part");
    let partial = std::path::PathBuf::from(partial);

    // A leftover from an interrupted export would make VACUUM INTO refuse to
    // run: it insists on creating the file itself.
    if partial.exists() {
        std::fs::remove_file(&partial)
            .map_err(|e| format!("Не удалось убрать остаток прошлого экспорта: {}", e))?;
    }

    let partial_str = partial.to_string_lossy().into_owned();
    conn.execute("VACUUM INTO ?1", params![partial_str])
        .map_err(|e| format!("Не удалось создать копию базы: {}", e))?;

    // Read the snapshot back before trusting it. Handing over a corrupt export
    // is worse than failing loudly, because it is only discovered on the day it
    // is needed.
    let counts = match inspect_export(&partial) {
        Ok(counts) => counts,
        Err(e) => {
            let _ = std::fs::remove_file(&partial);
            return Err(format!("Копия получилась нечитаемой, файл не сохранён: {}", e));
        }
    };

    let size_bytes = std::fs::metadata(&partial).map(|m| m.len()).unwrap_or(0);

    // Only now is anything at `target` touched. On Windows `rename` replaces an
    // existing file, which is what the user agreed to in the save dialog.
    std::fs::rename(&partial, target).map_err(|e| {
        let _ = std::fs::remove_file(&partial);
        format!("Не удалось сохранить файл: {}", e)
    })?;

    Ok(DatabaseExport {
        path: target.to_string_lossy().into_owned(),
        size_bytes,
        boards: counts.boards,
        boards_active: counts.boards_active,
        cards: counts.cards,
        cards_active: counts.cards_active,
        members: counts.members,
    })
}

/// What a finished snapshot turned out to contain.
struct ExportCounts {
    boards: i64,
    boards_active: i64,
    cards: i64,
    cards_active: i64,
    members: i64,
}

/// Opens a freshly written snapshot read-only and counts what is inside, both
/// as an integrity check and so the UI can report something concrete.
///
/// "Active" means what the user can actually see: `boards_active` matches the
/// filter `get_boards` uses for the hub, `cards_active` matches the one behind
/// the "Список" screen. The unqualified totals include archived rows and the
/// hidden Inbox boards, which is the honest measure of what the file holds.
fn inspect_export(path: &std::path::Path) -> Result<ExportCounts, String> {
    let copy = rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    ).map_err(to_string_err)?;

    let integrity: String = copy
        .query_row("PRAGMA quick_check(1)", [], |row| row.get(0))
        .map_err(to_string_err)?;
    if integrity != "ok" {
        return Err(format!("проверка целостности вернула «{}»", integrity));
    }

    let count = |sql: &str| -> Result<i64, String> {
        copy.query_row(sql, [], |row| row.get(0)).map_err(to_string_err)
    };

    Ok(ExportCounts {
        boards: count("SELECT COUNT(*) FROM boards")?,
        boards_active: count("SELECT COUNT(*) FROM boards WHERE archived = 0 AND is_system = 0")?,
        cards: count("SELECT COUNT(*) FROM cards")?,
        cards_active: count(
            "SELECT COUNT(*) FROM cards c
             INNER JOIN columns col ON col.id = c.column_id
             INNER JOIN boards b ON b.id = col.board_id
             WHERE c.archived = 0 AND col.archived = 0 AND b.archived = 0",
        )?,
        members: count("SELECT COUNT(*) FROM members")?,
    })
}

/// Default file name offered in the save dialog: `taskflow-2026-08-22.db`.
#[tauri::command]
pub fn suggest_export_name(state: State<'_, DbState>) -> CmdResult<String> {
    let conn = state.conn.lock().unwrap();
    let stamp: String = conn
        .query_row("SELECT strftime('%Y-%m-%d', 'now', 'localtime')", [], |row| row.get(0))
        .map_err(to_string_err)?;
    Ok(format!("taskflow-{}.db", stamp))
}

/// Single source of truth for the version shown in the About section — comes
/// from the bundle config, which in turn reads `package.json`.
#[tauri::command]
pub fn get_app_version(app: tauri::AppHandle) -> String {
    app.package_info().version.to_string()
}

// ─── Sidebar background, one per workspace ───
//
// The picture the user picks is never referenced where it lies. It is decoded,
// shrunk and re-encoded into the app's own `backgrounds` folder, because a
// reference to the original would quietly break the day that file is moved,
// renamed, or was sitting on a stick that is no longer plugged in.
//
// What the database keeps is a bare file name, not a path — see the migration
// in `db.rs` for why.

/// Longest side, in pixels, of the stored picture.
///
/// Was 800 while the picture was only ever seen through a 40 px blur. Two
/// things changed that: the board screen now shows the same file *sharp*, and
/// the layer covers the whole window rather than the sidebar. On a 1500 px
/// window the layer is stretched to roughly 1770 px (`inset: -20px` plus
/// `scale(1.15)`), so 800 px arrived visibly soft on the one screen where
/// softness is not wanted.
///
/// 1600 is the smallest round number that covers a normal laptop window at
/// 1:1. The limit is still there to keep a 12-megapixel phone photo out of the
/// app folder — see `measure_stored_background_size_on_real_photos` for what
/// real wallpapers actually weigh at this setting.
const BACKGROUND_MAX_SIDE: u32 = 1600;

/// JPEG quality of the stored picture. Generous for something this blurred; the
/// point is only to stay clear of visible blocking in large flat areas.
const BACKGROUND_JPEG_QUALITY: u8 = 82;

/// Sources above this are refused before being decoded. Decoding is where the
/// memory goes — a compressed file this size can expand to gigabytes — and no
/// wallpaper needs to be bigger.
const BACKGROUND_MAX_SOURCE_BYTES: u64 = 40 * 1024 * 1024;

#[tauri::command]
pub fn set_workspace_background(
    workspace_id: i64,
    source_path: String,
    state: State<'_, DbState>,
) -> CmdResult<String> {
    let conn = state.conn.lock().unwrap();
    let dir = state.app_dir.join(crate::db::BACKGROUNDS_DIR_NAME);
    store_background(&conn, &dir, workspace_id, std::path::Path::new(&source_path))
}

#[tauri::command]
pub fn clear_workspace_background(workspace_id: i64, state: State<'_, DbState>) -> CmdResult<()> {
    let conn = state.conn.lock().unwrap();
    let dir = state.app_dir.join(crate::db::BACKGROUNDS_DIR_NAME);
    clear_background(&conn, &dir, workspace_id)
}

/// The workspace's background as a `data:` URL, or `None` if it has none.
///
/// A data URL rather than a file URL: the WebView cannot read the app data
/// folder without turning on Tauri's asset protocol and scoping it, and the
/// picture is small enough after re-encoding that handing over the bytes is the
/// simpler contract. The front-end caches the result per workspace.
#[tauri::command]
pub fn get_workspace_background(
    workspace_id: i64,
    state: State<'_, DbState>,
) -> CmdResult<Option<String>> {
    let conn = state.conn.lock().unwrap();
    let dir = state.app_dir.join(crate::db::BACKGROUNDS_DIR_NAME);
    read_background(&conn, &dir, workspace_id)
}

/// File name currently recorded for the workspace, if any.
fn background_name(conn: &rusqlite::Connection, workspace_id: i64) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT background_image_path FROM workspaces WHERE id = ?1",
        params![workspace_id],
        |row| row.get::<_, Option<String>>(0),
    )
    .optional()
    .map_err(to_string_err)
    // Outer `None` is "no such workspace", inner is "no background"; neither is
    // an error to the caller.
    .map(|found| found.flatten())
}

/// Resolves a stored name to a file inside `dir`, or `None` if the name is not a
/// plain file name. The value comes from our own column, but it is joined onto
/// the folder rather than trusted as a path: nothing that ends up here should be
/// able to reach outside the backgrounds folder.
fn background_file_path(dir: &std::path::Path, name: &str) -> Option<std::path::PathBuf> {
    let mut parts = std::path::Path::new(name).components();
    let only = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    match only {
        std::path::Component::Normal(part) => Some(dir.join(part)),
        _ => None,
    }
}

/// Deletes one picture, treating "already gone" as success.
fn remove_background_file(dir: &std::path::Path, name: &str) {
    let Some(path) = background_file_path(dir, name) else { return };
    match std::fs::remove_file(&path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => log::warn!("Не удалось удалить фон {:?}: {}", path, e),
    }
}

/// Imports `source` as the workspace's background and returns the stored name.
fn store_background(
    conn: &rusqlite::Connection,
    dir: &std::path::Path,
    workspace_id: i64,
    source: &std::path::Path,
) -> Result<String, String> {
    let known: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM workspaces WHERE id = ?1)",
            params![workspace_id],
            |row| row.get(0),
        )
        .map_err(to_string_err)?;
    if !known {
        return Err("Пространство не найдено".to_string());
    }

    let size = std::fs::metadata(source)
        .map_err(|e| format!("Файл недоступен: {}", e))?
        .len();
    if size > BACKGROUND_MAX_SOURCE_BYTES {
        return Err(format!(
            "Файл слишком большой — {} МБ. Выберите изображение до {} МБ.",
            size / (1024 * 1024),
            BACKGROUND_MAX_SOURCE_BYTES / (1024 * 1024)
        ));
    }

    let raw = std::fs::read(source).map_err(|e| format!("Не удалось прочитать файл: {}", e))?;
    let jpeg = encode_background_image(&raw)?;

    std::fs::create_dir_all(dir).map_err(|e| format!("Папка для фонов недоступна: {}", e))?;

    // The name carries the workspace and the moment of upload, so a replacement
    // never reuses the name the WebView already has cached.
    let stamp: String = conn
        .query_row("SELECT strftime('%Y%m%d-%H%M%S', 'now', 'localtime')", [], |row| row.get(0))
        .map_err(to_string_err)?;
    let name = format!("{}_{}.jpg", workspace_id, stamp);

    // Written under a temporary name and renamed into place, so an interrupted
    // write cannot leave a truncated file under the name the database is about
    // to point at.
    let partial = dir.join(format!("{}.part", name));
    let target = dir.join(&name);
    std::fs::write(&partial, &jpeg).map_err(|e| format!("Не удалось сохранить изображение: {}", e))?;
    std::fs::rename(&partial, &target).map_err(|e| {
        let _ = std::fs::remove_file(&partial);
        format!("Не удалось сохранить изображение: {}", e)
    })?;

    let previous = background_name(conn, workspace_id)?;

    conn.execute(
        "UPDATE workspaces SET background_image_path = ?1 WHERE id = ?2",
        params![name, workspace_id],
    )
    .map_err(to_string_err)?;

    // Only now is the old picture expendable. In this order a failure anywhere
    // above leaves the workspace with the background it already had, and the
    // worst case here is one orphaned file rather than a missing background.
    if let Some(old) = previous {
        if old != name {
            remove_background_file(dir, &old);
        }
    }

    Ok(name)
}

fn clear_background(
    conn: &rusqlite::Connection,
    dir: &std::path::Path,
    workspace_id: i64,
) -> Result<(), String> {
    let Some(name) = background_name(conn, workspace_id)? else { return Ok(()) };

    conn.execute(
        "UPDATE workspaces SET background_image_path = NULL WHERE id = ?1",
        params![workspace_id],
    )
    .map_err(to_string_err)?;

    remove_background_file(dir, &name);
    Ok(())
}

fn read_background(
    conn: &rusqlite::Connection,
    dir: &std::path::Path,
    workspace_id: i64,
) -> Result<Option<String>, String> {
    let Some(name) = background_name(conn, workspace_id)? else { return Ok(None) };
    let Some(path) = background_file_path(dir, &name) else { return Ok(None) };

    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        // The folder is an ordinary directory the user can empty from outside
        // the app, and a restored backup can name a file that never came with
        // it. That is "no background", not a failure — and the row is cleared so
        // the settings screen stops offering to reset something already gone.
        Err(_) => {
            let _ = conn.execute(
                "UPDATE workspaces SET background_image_path = NULL WHERE id = ?1",
                params![workspace_id],
            );
            return Ok(None);
        }
    };

    Ok(Some(format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )))
}

/// Decodes whatever the user picked, shrinks it to fit `BACKGROUND_MAX_SIDE` and
/// re-encodes it as JPEG.
///
/// The format is decided by the file's own bytes rather than its extension: an
/// extension is not evidence of anything, and the picker's filter only limits
/// what is easy to choose, not what can be typed into the dialog.
fn encode_background_image(raw: &[u8]) -> Result<Vec<u8>, String> {
    let reader = image::ImageReader::new(std::io::Cursor::new(raw))
        .with_guessed_format()
        .map_err(|e| format!("Не удалось прочитать изображение: {}", e))?;

    match reader.format() {
        Some(image::ImageFormat::Png | image::ImageFormat::Jpeg | image::ImageFormat::WebP) => {}
        _ => return Err("Это не изображение PNG, JPEG или WebP".to_string()),
    }

    let picture = reader
        .decode()
        .map_err(|e| format!("Файл не удалось разобрать как изображение: {}", e))?;

    // `resize` fits the picture inside the box keeping its proportions — but it
    // also scales *up* when the source is smaller, which would store more bytes
    // than were given to us. Hence the explicit check.
    let picture = if picture.width() > BACKGROUND_MAX_SIDE || picture.height() > BACKGROUND_MAX_SIDE {
        picture.resize(
            BACKGROUND_MAX_SIDE,
            BACKGROUND_MAX_SIDE,
            image::imageops::FilterType::Triangle,
        )
    } else {
        picture
    };

    // JPEG carries no alpha channel. Dropping it is acceptable here and nowhere
    // else in the app: the picture is a background behind a dark overlay, never
    // an illustration, so what a transparent PNG loses has nowhere to show.
    let rgb = picture.to_rgb8();

    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, BACKGROUND_JPEG_QUALITY)
        .encode(
            rgb.as_raw(),
            rgb.width(),
            rgb.height(),
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|e| format!("Не удалось пересжать изображение: {}", e))?;

    Ok(out)
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
