// ============================================
// TaskFlow — tests for export/import and deletion
// ============================================
// These drive the real code path (`build_board_export`, `import_board_into`)
// against an in-memory database built from the production schema, rather than
// re-implementing the SQL. A round trip that silently loses data is exactly the
// kind of bug that would otherwise only surface on a user's real board — after
// they had already deleted the original.

use super::*;
use rusqlite::Connection;

/// In-memory database with the production schema and one workspace.
fn test_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
    crate::db::create_schema(&conn).unwrap();
    conn.execute("INSERT INTO workspaces (name) VALUES ('Тест')", ()).unwrap();
    conn
}

/// A board exercising every field the export format carries: several columns
/// (one archived), a card with a due date and labels, an archived card, and a
/// mistake-flagged card.
fn seed_board(conn: &Connection, workspace_id: i64) -> i64 {
    conn.execute(
        "INSERT INTO boards (workspace_id, name, gradient, is_starred) VALUES (?1, 'Доска А', 'linear-gradient(1)', 1)",
        params![workspace_id],
    ).unwrap();
    let board_id = conn.last_insert_rowid();

    conn.execute("INSERT INTO labels (board_id, name, color) VALUES (?1, 'Баг', '#f00')", params![board_id]).unwrap();
    let label_bug = conn.last_insert_rowid();
    conn.execute("INSERT INTO labels (board_id, name, color) VALUES (?1, 'Срочно', '#0f0')", params![board_id]).unwrap();
    let label_urgent = conn.last_insert_rowid();

    conn.execute("INSERT INTO columns (board_id, name, position) VALUES (?1, 'Бэклог', 0)", params![board_id]).unwrap();
    let col_backlog = conn.last_insert_rowid();
    conn.execute("INSERT INTO columns (board_id, name, position, archived) VALUES (?1, 'Старое', 1, 1)", params![board_id]).unwrap();
    let col_archived = conn.last_insert_rowid();

    conn.execute(
        "INSERT INTO cards (column_id, title, description, position, due_date) VALUES (?1, 'Задача 1', 'Описание', 0, '2026-09-01')",
        params![col_backlog],
    ).unwrap();
    let card1 = conn.last_insert_rowid();

    conn.execute(
        "INSERT INTO cards (column_id, title, description, position, archived) VALUES (?1, 'Архивная', '', 1, 1)",
        params![col_backlog],
    ).unwrap();

    conn.execute(
        "INSERT INTO cards (column_id, title, description, position, is_mistake, mistake_marked_at)
         VALUES (?1, 'Ошибка', '', 0, 1, '2026-08-01 10:00:00')",
        params![col_archived],
    ).unwrap();

    conn.execute("INSERT INTO card_labels (card_id, label_id) VALUES (?1, ?2)", params![card1, label_bug]).unwrap();
    conn.execute("INSERT INTO card_labels (card_id, label_id) VALUES (?1, ?2)", params![card1, label_urgent]).unwrap();

    board_id
}

#[test]
fn export_captures_the_whole_board_including_archived_items() {
    let conn = test_db();
    let board_id = seed_board(&conn, 1);

    let export = build_board_export(&conn, board_id).unwrap();

    assert_eq!(export.taskflow_export_version, EXPORT_FORMAT_VERSION);
    assert_eq!(export.board.name, "Доска А");
    assert!(export.board.is_starred);
    assert_eq!(export.board.labels.len(), 2);
    // The archived column is part of the snapshot, not filtered out.
    assert_eq!(export.board.columns.len(), 2);

    let backlog = &export.board.columns[0];
    assert_eq!(backlog.name, "Бэклог");
    assert_eq!(backlog.cards.len(), 2, "архивная карточка тоже должна попасть в экспорт");
    assert_eq!(backlog.cards[0].title, "Задача 1");
    assert_eq!(backlog.cards[0].due_date.as_deref(), Some("2026-09-01"));
    assert_eq!(backlog.cards[0].label_ids.len(), 2);
    assert_eq!(backlog.cards[1].archived, 1);

    let archived_col = &export.board.columns[1];
    assert_eq!(archived_col.archived, 1);
    assert!(archived_col.cards[0].is_mistake);
}

#[test]
fn import_round_trip_preserves_the_board() {
    let mut conn = test_db();
    let original_id = seed_board(&conn, 1);

    // Goes through real JSON, the same way the file on disk does.
    let json = serde_json::to_string(&build_board_export(&conn, original_id).unwrap()).unwrap();
    let parsed: BoardExport = serde_json::from_str(&json).unwrap();
    let new_id = import_board_into(&mut conn, 1, parsed).unwrap();

    assert_ne!(new_id, original_id, "импорт должен создавать новую доску, а не перезаписывать");

    let before = build_board_export(&conn, original_id).unwrap();
    let after = build_board_export(&conn, new_id).unwrap();

    assert_eq!(before.board.name, after.board.name);
    assert_eq!(before.board.gradient, after.board.gradient);
    assert_eq!(before.board.is_starred, after.board.is_starred);
    assert_eq!(before.board.labels.len(), after.board.labels.len());
    assert_eq!(before.board.columns.len(), after.board.columns.len());

    for (b, a) in before.board.columns.iter().zip(after.board.columns.iter()) {
        assert_eq!(b.name, a.name);
        assert_eq!(b.archived, a.archived);
        assert_eq!(b.cards.len(), a.cards.len());
        for (bc, ac) in b.cards.iter().zip(a.cards.iter()) {
            assert_eq!(bc.title, ac.title);
            assert_eq!(bc.description, ac.description);
            assert_eq!(bc.due_date, ac.due_date);
            assert_eq!(bc.archived, ac.archived);
            assert_eq!(bc.is_mistake, ac.is_mistake);
            assert_eq!(bc.label_ids.len(), ac.label_ids.len(), "связи с метками должны сохраниться");
        }
    }
}

#[test]
fn imported_labels_point_at_the_new_boards_own_labels() {
    let mut conn = test_db();
    let original_id = seed_board(&conn, 1);

    let export = build_board_export(&conn, original_id).unwrap();
    let new_id = import_board_into(&mut conn, 1, export).unwrap();

    // Every label id referenced by an imported card must belong to the new
    // board — a naive import would leave them pointing at the original's rows.
    let new_label_ids: Vec<i64> = conn
        .prepare("SELECT id FROM labels WHERE board_id = ?1").unwrap()
        .query_map(params![new_id], |r| r.get(0)).unwrap()
        .collect::<rusqlite::Result<Vec<_>>>().unwrap();

    let after = build_board_export(&conn, new_id).unwrap();
    let referenced: Vec<i64> = after.board.columns.iter()
        .flat_map(|c| c.cards.iter())
        .flat_map(|c| c.label_ids.iter().copied())
        .collect();

    assert!(!referenced.is_empty(), "тестовые данные должны содержать связи с метками");
    for id in referenced {
        assert!(new_label_ids.contains(&id), "метка {} не принадлежит импортированной доске", id);
    }
}

#[test]
fn import_rejects_a_newer_format_and_writes_nothing() {
    let mut conn = test_db();
    let boards_before: i64 = conn.query_row("SELECT COUNT(*) FROM boards", [], |r| r.get(0)).unwrap();

    let export = BoardExport {
        taskflow_export_version: EXPORT_FORMAT_VERSION + 1,
        exported_at: String::new(),
        board: BoardExportBody {
            name: "Из будущего".into(),
            gradient: String::new(),
            is_starred: false,
            labels: vec![],
            columns: vec![],
        },
    };

    assert!(import_board_into(&mut conn, 1, export).is_err());

    let boards_after: i64 = conn.query_row("SELECT COUNT(*) FROM boards", [], |r| r.get(0)).unwrap();
    assert_eq!(boards_before, boards_after, "отклонённый импорт не должен оставлять доску");
}

#[test]
fn import_rejects_a_board_without_a_name() {
    let mut conn = test_db();
    let export = BoardExport {
        taskflow_export_version: EXPORT_FORMAT_VERSION,
        exported_at: String::new(),
        board: BoardExportBody {
            name: "   ".into(),
            gradient: String::new(),
            is_starred: false,
            labels: vec![],
            columns: vec![],
        },
    };
    assert!(import_board_into(&mut conn, 1, export).is_err());
}

#[test]
fn import_skips_label_links_the_file_does_not_define() {
    let mut conn = test_db();

    // A card referencing label id 999, which has no entry in the file's label
    // list — a hand-edited or truncated export.
    let export = BoardExport {
        taskflow_export_version: EXPORT_FORMAT_VERSION,
        exported_at: String::new(),
        board: BoardExportBody {
            name: "Битый".into(),
            gradient: String::new(),
            is_starred: false,
            labels: vec![],
            columns: vec![ColumnExport {
                name: "Колонка".into(),
                position: 0,
                archived: 0,
                cards: vec![CardExport {
                    title: "Карточка".into(),
                    description: String::new(),
                    position: 0,
                    due_date: None,
                    archived: 0,
                    is_mistake: false,
                    mistake_marked_at: None,
                    mistake_resolved_at: None,
                    label_ids: vec![999],
                }],
            }],
        },
    };

    let new_id = import_board_into(&mut conn, 1, export)
        .expect("недостающая метка не должна ронять весь импорт");

    let after = build_board_export(&conn, new_id).unwrap();
    assert_eq!(after.board.columns[0].cards[0].title, "Карточка");
    assert!(after.board.columns[0].cards[0].label_ids.is_empty());
}

#[test]
fn delete_board_leaves_no_orphan_rows() {
    let conn = test_db();
    let board_id = seed_board(&conn, 1);

    // Same statement order as delete_board — that order is what has to hold up
    // with foreign keys enforced.
    conn.execute(
        "DELETE FROM card_labels WHERE card_id IN (
            SELECT c.id FROM cards c INNER JOIN columns col ON col.id = c.column_id WHERE col.board_id = ?1)",
        params![board_id],
    ).unwrap();
    conn.execute("DELETE FROM cards WHERE column_id IN (SELECT id FROM columns WHERE board_id = ?1)", params![board_id]).unwrap();
    conn.execute("DELETE FROM columns WHERE board_id = ?1", params![board_id]).unwrap();
    conn.execute("DELETE FROM labels WHERE board_id = ?1", params![board_id]).unwrap();
    conn.execute("DELETE FROM board_recent_views WHERE board_id = ?1", params![board_id]).unwrap();
    conn.execute("DELETE FROM boards WHERE id = ?1", params![board_id]).unwrap();

    for table in ["cards", "columns", "labels", "card_labels"] {
        let n: i64 = conn.query_row(&format!("SELECT COUNT(*) FROM {}", table), [], |r| r.get(0)).unwrap();
        assert_eq!(n, 0, "в таблице {} остались строки удалённой доски", table);
    }

    let violations: i64 = conn
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(violations, 0);
}

#[test]
fn backup_file_names_render_as_readable_dates() {
    assert_eq!(parse_backup_stamp("backup-20260817-143012.db"), "17.08.2026 14:30:12");
    // Anything unexpected falls back to the raw name rather than panicking.
    assert_eq!(parse_backup_stamp("что-то-другое.db"), "что-то-другое.db");
    assert_eq!(parse_backup_stamp("backup-123.db"), "backup-123.db");
}
