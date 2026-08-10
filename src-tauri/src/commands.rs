use rusqlite::params;
use std::sync::Mutex;
use tauri::State;
use crate::db::DbState;
use crate::models::{Workspace, Board, Column, Card, Label};
use serde_json::Value;

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

#[tauri::command]
pub fn get_boards(workspace_id: i64, state: State<'_, DbState>) -> CmdResult<Vec<Board>> {
    let conn = state.conn.lock().unwrap();
    let mut stmt = conn.prepare("SELECT id, workspace_id, name, gradient, is_starred, created_at, archived FROM boards WHERE workspace_id = ?1 AND archived = 0 ORDER BY is_starred DESC, id DESC").map_err(to_string_err)?;
    
    let iter = stmt.query_map(params![workspace_id], |row| {
        Ok(Board {
            id: row.get(0)?,
            workspace_id: row.get(1)?,
            name: row.get(2)?,
            gradient: row.get(3)?,
            is_starred: {
                let s: i8 = row.get(4)?;
                s != 0
            },
            created_at: row.get(5)?,
            archived: row.get(6)?,
        })
    }).map_err(to_string_err)?;
    
    let mut res = Vec::new();
    for i in iter { res.push(i.map_err(to_string_err)?); }
    Ok(res)
}

#[tauri::command]
pub fn get_board(id: i64, state: State<'_, DbState>) -> CmdResult<Board> {
    let conn = state.conn.lock().unwrap();
    let mut stmt = conn.prepare("SELECT id, workspace_id, name, gradient, is_starred, created_at, archived FROM boards WHERE id = ?1").map_err(to_string_err)?;
    
    let board = stmt.query_row(params![id], |row| {
        Ok(Board {
            id: row.get(0)?,
            workspace_id: row.get(1)?,
            name: row.get(2)?,
            gradient: row.get(3)?,
            is_starred: { let s: i8 = row.get(4)?; s != 0 },
            created_at: row.get(5)?,
            archived: row.get(6)?,
        })
    }).map_err(to_string_err)?;
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
        id, workspace_id, name, gradient, is_starred: false, created_at: "".into(), archived: 0
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
    let mut stmt = conn.prepare("SELECT id, workspace_id, name, gradient, is_starred, created_at, archived FROM boards WHERE workspace_id = ?1 AND archived = 1").map_err(to_string_err)?;
    
    let iter = stmt.query_map(params![workspace_id], |row| {
        Ok(Board {
            id: row.get(0)?,
            workspace_id: row.get(1)?,
            name: row.get(2)?,
            gradient: row.get(3)?,
            is_starred: { let s: i8 = row.get(4)?; s != 0 },
            created_at: row.get(5)?,
            archived: row.get(6)?,
        })
    }).map_err(to_string_err)?;
    
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

#[tauri::command]
pub fn get_cards(column_id: i64, state: State<'_, DbState>) -> CmdResult<Vec<Card>> {
    let conn = state.conn.lock().unwrap();
    let mut stmt = conn.prepare("SELECT id, column_id, title, description, position, due_date, created_at, archived FROM cards WHERE column_id = ?1 AND archived = 0 ORDER BY position ASC").map_err(to_string_err)?;
    
    let iter = stmt.query_map(params![column_id], |row| {
        Ok(Card {
            id: row.get(0)?,
            column_id: row.get(1)?,
            title: row.get(2)?,
            description: row.get(3)?,
            position: row.get(4)?,
            due_date: row.get(5)?,
            created_at: row.get(6)?,
            archived: row.get(7)?,
            labels: Vec::new(), // Populate labels later if needed
        })
    }).map_err(to_string_err)?;
    
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
    Ok(Card { id, column_id, title, description, position: pos, due_date: None, created_at: "".into(), archived: 0, labels: vec![] })
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

#[tauri::command]
pub fn move_card(id: i64, new_column_id: i64, new_position: i64, state: State<'_, DbState>) -> CmdResult<()> {
    let mut conn = state.conn.lock().unwrap();
    let tx = conn.transaction().map_err(to_string_err)?;
    
    // Shift cards in new column down to make room
    tx.execute(
        "UPDATE cards SET position = position + 1 WHERE column_id = ?1 AND position >= ?2",
        params![new_column_id, new_position],
    ).map_err(to_string_err)?;
    
    // Update card's column and position
    tx.execute(
        "UPDATE cards SET column_id = ?1, position = ?2 WHERE id = ?3",
        params![new_column_id, new_position, id],
    ).map_err(to_string_err)?;
    
    // Normalize positions in old column could be done, but simplified for now
    
    tx.commit().map_err(to_string_err)?;
    Ok(())
}

#[tauri::command]
pub fn export_board(board_id: i64, state: State<'_, DbState>) -> CmdResult<String> {
    // Just a stub for exporting, returning some JSON
    Ok(r#"{"status": "exported"}"#.to_string())
}
