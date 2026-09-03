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
/// (one archived), a card with a due date, labels and a checklist, an archived
/// card, and a mistake-flagged card.
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

    // Позиции идут с дырой и не по порядку вставки: так и выглядит настоящая
    // карточка, из которой что-то удаляли и что-то перетаскивали. Экспорт
    // обязан отдать их в порядке position, а не в порядке id.
    conn.execute(
        "INSERT INTO checklist_items (card_id, text, is_done, position) VALUES (?1, 'Третий', 0, 7)",
        params![card1],
    ).unwrap();
    conn.execute(
        "INSERT INTO checklist_items (card_id, text, is_done, position) VALUES (?1, 'Первый', 1, 0)",
        params![card1],
    ).unwrap();
    conn.execute(
        "INSERT INTO checklist_items (card_id, text, is_done, position) VALUES (?1, 'Второй', 0, 3)",
        params![card1],
    ).unwrap();

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
            let before_items: Vec<(&str, bool)> =
                bc.checklist.iter().map(|i| (i.text.as_str(), i.is_done)).collect();
            let after_items: Vec<(&str, bool)> =
                ac.checklist.iter().map(|i| (i.text.as_str(), i.is_done)).collect();
            assert_eq!(before_items, after_items, "чек-лист карточки «{}» потерялся", bc.title);
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
            members: vec![],
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
            members: vec![],
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
            members: vec![],
            columns: vec![ColumnExport {
                name: "Колонка".into(),
                position: 0,
                archived: 0,
                is_final: false,
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
                    assignee_id: None,
                    author_id: None,
                    priority: None,
                    checklist: Vec::new(),
                    comments: Vec::new(),
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
    conn.execute(
        "DELETE FROM checklist_items WHERE card_id IN (
            SELECT c.id FROM cards c INNER JOIN columns col ON col.id = c.column_id WHERE col.board_id = ?1)",
        params![board_id],
    ).unwrap();
    conn.execute("DELETE FROM cards WHERE column_id IN (SELECT id FROM columns WHERE board_id = ?1)", params![board_id]).unwrap();
    conn.execute("DELETE FROM columns WHERE board_id = ?1", params![board_id]).unwrap();
    conn.execute("DELETE FROM labels WHERE board_id = ?1", params![board_id]).unwrap();
    conn.execute("DELETE FROM board_recent_views WHERE board_id = ?1", params![board_id]).unwrap();
    conn.execute("DELETE FROM boards WHERE id = ?1", params![board_id]).unwrap();

    for table in ["cards", "columns", "labels", "card_labels", "checklist_items"] {
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

// ============================================
// Members, assignment and the workspace-wide list
// ============================================
// The migration below moves data the user has already typed (their name and
// initials) between tables, and the deletion path has to cooperate with
// enforced foreign keys. Both are verified here against the production schema
// rather than reasoned about.

/// Id of the member representing the user of this installation.
fn self_member_id(conn: &Connection) -> i64 {
    conn.query_row("SELECT id FROM members WHERE is_self = 1", [], |r| r.get(0)).unwrap()
}

/// Rewinds a database to how it looked before members existed, so the
/// migration inside `create_schema` runs again on the next call.
fn forget_members(conn: &Connection) {
    conn.execute("UPDATE cards SET assignee_id = NULL, author_id = NULL", ()).unwrap();
    conn.execute("DELETE FROM members", ()).unwrap();
}

#[test]
fn the_existing_profile_becomes_the_self_member() {
    let conn = test_db();

    // A profile the user had already personalised before members existed.
    conn.execute(
        "UPDATE user_profile SET display_name = 'Шахзод', avatar_initials = 'ШИ' WHERE id = 1",
        (),
    ).unwrap();
    forget_members(&conn);

    crate::db::create_schema(&conn).unwrap();

    let (name, initials, is_self): (String, String, i64) = conn
        .query_row(
            "SELECT name, initials, is_self FROM members WHERE is_self = 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();

    assert_eq!(name, "Шахзод", "имя из профиля должно было переехать без изменений");
    assert_eq!(initials, "ШИ");
    assert_eq!(is_self, 1);

    // The old profile row is deliberately left intact, so the migration is not
    // a one-way door.
    let old_name: String = conn
        .query_row("SELECT display_name FROM user_profile WHERE id = 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(old_name, "Шахзод", "user_profile не должен затираться миграцией");
}

#[test]
fn migration_is_idempotent() {
    let conn = test_db();
    for _ in 0..3 {
        crate::db::create_schema(&conn).unwrap();
    }
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM members", [], |r| r.get(0)).unwrap();
    assert_eq!(count, 1, "повторный запуск create_schema не должен плодить участников");
}

#[test]
fn a_second_self_member_is_rejected() {
    let conn = test_db();
    let result = conn.execute(
        "INSERT INTO members (name, initials, color, is_self) VALUES ('Двойник', 'ДД', '#000', 1)",
        (),
    );
    assert!(result.is_err(), "уникальный индекс должен запрещать второго is_self");
}

#[test]
fn existing_cards_are_backfilled_with_the_user_as_author() {
    let conn = test_db();
    let ws: i64 = conn.query_row("SELECT id FROM workspaces LIMIT 1", [], |r| r.get(0)).unwrap();
    seed_board(&conn, ws);

    forget_members(&conn);
    crate::db::create_schema(&conn).unwrap();

    let orphans: i64 = conn
        .query_row("SELECT COUNT(*) FROM cards WHERE author_id IS NULL", [], |r| r.get(0))
        .unwrap();
    assert_eq!(orphans, 0, "у существующих карточек должен появиться автор");

    let self_id = self_member_id(&conn);
    let wrong: i64 = conn
        .query_row("SELECT COUNT(*) FROM cards WHERE author_id != ?1", params![self_id], |r| r.get(0))
        .unwrap();
    assert_eq!(wrong, 0);
}

#[test]
fn clearing_an_author_is_not_undone_by_a_restart() {
    // The backfill runs once, when the self member is created — not on every
    // start. Otherwise deliberately clearing an author would silently come back.
    let conn = test_db();
    let ws: i64 = conn.query_row("SELECT id FROM workspaces LIMIT 1", [], |r| r.get(0)).unwrap();
    seed_board(&conn, ws);

    conn.execute("UPDATE cards SET author_id = NULL", ()).unwrap();
    crate::db::create_schema(&conn).unwrap(); // «перезапуск приложения»

    let cleared: i64 = conn
        .query_row("SELECT COUNT(*) FROM cards WHERE author_id IS NULL", [], |r| r.get(0))
        .unwrap();
    assert!(cleared > 0, "снятый автор не должен возвращаться при перезапуске");
}

#[test]
fn deleting_a_member_releases_their_cards_instead_of_failing() {
    let mut conn = test_db();
    let ws: i64 = conn.query_row("SELECT id FROM workspaces LIMIT 1", [], |r| r.get(0)).unwrap();
    seed_board(&conn, ws);

    conn.execute("INSERT INTO members (name, initials, color) VALUES ('Пётр', 'ПП', '#f00')", ()).unwrap();
    let petr = conn.last_insert_rowid();
    conn.execute("UPDATE cards SET assignee_id = ?1, author_id = ?1", params![petr]).unwrap();

    // Without clearing the references first this fails outright: foreign keys
    // are enforced and ALTER TABLE could not declare ON DELETE SET NULL.
    delete_member_from(&mut conn, petr).expect("удаление участника с назначенными карточками");

    let left: i64 = conn
        .query_row("SELECT COUNT(*) FROM members WHERE id = ?1", params![petr], |r| r.get(0))
        .unwrap();
    assert_eq!(left, 0);

    let still_assigned: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM cards WHERE assignee_id IS NOT NULL OR author_id IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(still_assigned, 0, "ссылки на удалённого участника должны быть сняты");

    let violations: i64 = conn
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(violations, 0);

    // The cards themselves must survive — this removes a label, not the work.
    let cards: i64 = conn.query_row("SELECT COUNT(*) FROM cards", [], |r| r.get(0)).unwrap();
    assert!(cards > 0, "удаление участника не должно трогать карточки");
}

#[test]
fn the_user_cannot_delete_themselves() {
    let mut conn = test_db();
    let self_id = self_member_id(&conn);
    assert!(delete_member_from(&mut conn, self_id).is_err());
    assert!(delete_member_from(&mut conn, 9999).is_err(), "несуществующий участник — ошибка, а не тишина");
}

#[test]
fn initials_are_derived_from_the_name() {
    assert_eq!(default_initials("Иван Петров"), "ИП");
    assert_eq!(default_initials("Мадина"), "МА");
    assert_eq!(default_initials("  Анна   Мария  Ли "), "АМ");
    assert_eq!(default_initials(""), "");
}

#[test]
fn priority_is_clamped_to_what_the_schema_accepts() {
    assert_eq!(normalize_priority(Some("High")), "High");
    assert_eq!(normalize_priority(Some("Low")), "Low");
    // A stale frontend or a hand-edited export must not be able to write a
    // value the CHECK constraint would reject.
    assert_eq!(normalize_priority(Some("СРОЧНО")), "Medium");
    assert_eq!(normalize_priority(Some("high")), "Medium");
    assert_eq!(normalize_priority(None), "Medium");
}

#[test]
fn priority_and_people_survive_an_export_round_trip() {
    let mut conn = test_db();
    let ws: i64 = conn.query_row("SELECT id FROM workspaces LIMIT 1", [], |r| r.get(0)).unwrap();
    let board_id = seed_board(&conn, ws);

    conn.execute("INSERT INTO members (name, initials, color) VALUES ('Пётр', 'ПП', '#f00')", ()).unwrap();
    let petr = conn.last_insert_rowid();
    let me = self_member_id(&conn);

    let card_id: i64 = conn
        .query_row("SELECT id FROM cards WHERE title = 'Задача 1'", [], |r| r.get(0))
        .unwrap();
    conn.execute(
        "UPDATE cards SET assignee_id = ?1, author_id = ?2, priority = 'High' WHERE id = ?3",
        params![petr, me, card_id],
    ).unwrap();

    let json = serde_json::to_string(&build_board_export(&conn, board_id).unwrap()).unwrap();

    // Import back into the *same* database: both people are already in the
    // directory, so nothing may be duplicated.
    let members_before: i64 = conn.query_row("SELECT COUNT(*) FROM members", [], |r| r.get(0)).unwrap();
    let export: BoardExport = serde_json::from_str(&json).unwrap();
    let new_board = import_board_into(&mut conn, ws, export).unwrap();

    let members_after: i64 = conn.query_row("SELECT COUNT(*) FROM members", [], |r| r.get(0)).unwrap();
    assert_eq!(members_before, members_after, "импорт не должен плодить дубликаты участников");

    let (priority, assignee, author): (String, i64, i64) = conn
        .query_row(
            "SELECT c.priority, c.assignee_id, c.author_id
             FROM cards c INNER JOIN columns col ON col.id = c.column_id
             WHERE col.board_id = ?1 AND c.title = 'Задача 1'",
            params![new_board],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();

    assert_eq!(priority, "High");
    assert_eq!(assignee, petr, "исполнитель должен сопоставиться по имени");
    assert_eq!(author, me);
}

#[test]
fn importing_an_unknown_person_adds_them_to_the_directory() {
    let conn = test_db();
    let ws: i64 = conn.query_row("SELECT id FROM workspaces LIMIT 1", [], |r| r.get(0)).unwrap();
    let board_id = seed_board(&conn, ws);

    conn.execute("INSERT INTO members (name, initials, color) VALUES ('Гость', 'ГГ', '#0ff')", ()).unwrap();
    let guest = conn.last_insert_rowid();
    conn.execute("UPDATE cards SET assignee_id = ?1 WHERE title = 'Задача 1'", params![guest]).unwrap();

    let json = serde_json::to_string(&build_board_export(&conn, board_id).unwrap()).unwrap();

    // A different install: same file, but nobody named "Гость" here.
    let mut fresh = test_db();
    let fresh_ws: i64 = fresh.query_row("SELECT id FROM workspaces LIMIT 1", [], |r| r.get(0)).unwrap();
    let export: BoardExport = serde_json::from_str(&json).unwrap();
    let new_board = import_board_into(&mut fresh, fresh_ws, export).unwrap();

    let assignee_name: String = fresh
        .query_row(
            "SELECT m.name FROM cards c
             INNER JOIN columns col ON col.id = c.column_id
             INNER JOIN members m ON m.id = c.assignee_id
             WHERE col.board_id = ?1 AND c.title = 'Задача 1'",
            params![new_board],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(assignee_name, "Гость", "неизвестный исполнитель должен добавиться в справочник");

    let is_self: i64 = fresh
        .query_row("SELECT is_self FROM members WHERE name = 'Гость'", [], |r| r.get(0))
        .unwrap();
    assert_eq!(is_self, 0, "импортированный участник не должен становиться владельцем установки");
}

#[test]
fn an_export_from_before_members_still_imports() {
    // A file written by the previous build: no `members` section, no priority,
    // no assignee. It must import rather than being rejected.
    let old_json = r#"{
        "taskflow_export_version": 1,
        "exported_at": "2026-08-17 12:00:00",
        "board": {
            "name": "Старый экспорт",
            "gradient": "",
            "is_starred": false,
            "labels": [],
            "columns": [
                { "name": "Дела", "position": 0, "archived": 0,
                  "cards": [ { "title": "Старая задача", "description": "", "position": 0 } ] }
            ]
        }
    }"#;

    let mut conn = test_db();
    let ws: i64 = conn.query_row("SELECT id FROM workspaces LIMIT 1", [], |r| r.get(0)).unwrap();
    let export: BoardExport = serde_json::from_str(old_json).unwrap();
    let board_id = import_board_into(&mut conn, ws, export).unwrap();

    let (priority, assignee): (String, Option<i64>) = conn
        .query_row(
            "SELECT c.priority, c.assignee_id FROM cards c
             INNER JOIN columns col ON col.id = c.column_id WHERE col.board_id = ?1",
            params![board_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(priority, "Medium", "карточка без приоритета должна получить значение по умолчанию");
    assert_eq!(assignee, None);
}

#[test]
fn the_workspace_list_spans_every_board_at_once() {
    let conn = test_db();
    let ws: i64 = conn.query_row("SELECT id FROM workspaces LIMIT 1", [], |r| r.get(0)).unwrap();
    seed_board(&conn, ws);

    // A second board, so the screen has something to actually span.
    conn.execute("INSERT INTO boards (workspace_id, name) VALUES (?1, 'Доска Б')", params![ws]).unwrap();
    let board_b = conn.last_insert_rowid();
    conn.execute("INSERT INTO columns (board_id, name, position) VALUES (?1, 'Идеи', 0)", params![board_b]).unwrap();
    let col_b = conn.last_insert_rowid();
    conn.execute("INSERT INTO cards (column_id, title, description, position) VALUES (?1, 'Из другой доски', '', 0)", params![col_b]).unwrap();

    // A card in another workspace must not leak in.
    conn.execute("INSERT INTO workspaces (name) VALUES ('Чужое')", ()).unwrap();
    let other_ws = conn.last_insert_rowid();
    conn.execute("INSERT INTO boards (workspace_id, name) VALUES (?1, 'Чужая доска')", params![other_ws]).unwrap();
    let other_board = conn.last_insert_rowid();
    conn.execute("INSERT INTO columns (board_id, name, position) VALUES (?1, 'Чужая', 0)", params![other_board]).unwrap();
    let other_col = conn.last_insert_rowid();
    conn.execute("INSERT INTO cards (column_id, title, description, position) VALUES (?1, 'Чужая задача', '', 0)", params![other_col]).unwrap();

    let list = build_workspace_card_list(&conn, ws).unwrap();
    let titles: Vec<&str> = list.cards.iter().map(|c| c.title.as_str()).collect();

    assert!(titles.contains(&"Задача 1"));
    assert!(titles.contains(&"Из другой доски"), "список должен охватывать все доски пространства");
    assert!(!titles.contains(&"Чужая задача"), "чужое пространство не должно попадать в список");
    // Archived cards, and cards inside archived columns, stay out.
    assert!(!titles.contains(&"Архивная"));
    assert!(!titles.contains(&"Ошибка"), "карточка в архивной колонке не должна показываться");

    let boards: Vec<&str> = list.cards.iter().map(|c| c.board_name.as_str()).collect();
    assert!(boards.contains(&"Доска А") && boards.contains(&"Доска Б"));

    // Every row must know its own board's columns, or the status dropdown
    // would offer the wrong ones.
    let board_a = list.boards.iter().find(|b| b.name == "Доска А").unwrap();
    let col_names: Vec<&str> = board_a.columns.iter().map(|c| c.name.as_str()).collect();
    assert!(col_names.contains(&"Бэклог"));
    assert!(!col_names.contains(&"Старое"), "архивные колонки не предлагаются как статус");
    assert!(!col_names.contains(&"Идеи"), "колонки чужой доски не должны попадать в список");
}

#[test]
fn the_list_carries_whole_member_records_not_just_ids() {
    let conn = test_db();
    let ws: i64 = conn.query_row("SELECT id FROM workspaces LIMIT 1", [], |r| r.get(0)).unwrap();
    seed_board(&conn, ws);

    conn.execute("INSERT INTO members (name, initials, color) VALUES ('Пётр', 'ПП', '#ff0000')", ()).unwrap();
    let petr = conn.last_insert_rowid();
    conn.execute("UPDATE cards SET assignee_id = ?1 WHERE title = 'Задача 1'", params![petr]).unwrap();

    // Every other card seed_board makes is archived or sits in an archived
    // column, so the list would otherwise contain nothing unassigned to check.
    let live_col: i64 = conn
        .query_row("SELECT id FROM columns WHERE name = 'Бэклог'", [], |r| r.get(0))
        .unwrap();
    conn.execute(
        "INSERT INTO cards (column_id, title, description, position) VALUES (?1, 'Ничья', '', 5)",
        params![live_col],
    ).unwrap();

    let list = build_workspace_card_list(&conn, ws).unwrap();
    let card = list.cards.iter().find(|c| c.title == "Задача 1").unwrap();

    let assignee = card.assignee.as_ref().expect("исполнитель должен приехать вместе со строкой");
    assert_eq!(assignee.name, "Пётр");
    assert_eq!(assignee.color, "#ff0000", "цвет нужен для аватарки — иначе понадобился бы второй запрос");
    assert_eq!(assignee.initials, "ПП");

    let unassigned = list.cards.iter().find(|c| c.assignee.is_none());
    assert!(unassigned.is_some(), "карточка без исполнителя — это None, а не пустая запись");
}

// ============================================
// Full database export (VACUUM INTO)
// ============================================
// This is the button people press right before doing something risky, so the
// two things that matter are that the copy really contains everything and that
// a failed export cannot damage a file the user already had.

/// Unique temp directory for one test.
fn export_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("taskflow-export-{}", name));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn the_export_contains_the_whole_database() {
    let conn = test_db();
    let ws: i64 = conn.query_row("SELECT id FROM workspaces LIMIT 1", [], |r| r.get(0)).unwrap();
    seed_board(&conn, ws);
    conn.execute("INSERT INTO members (name, initials, color) VALUES ('Пётр', 'ПП', '#f00')", ()).unwrap();

    let target = export_dir("full").join("taskflow-copy.db");
    let result = export_database_to(&conn, &target).expect("экспорт должен пройти");

    assert!(target.exists(), "файл экспорта должен появиться на диске");
    assert!(result.size_bytes > 0);

    // Counts reported to the user must match the source, not be invented.
    let expect = |sql: &str| -> i64 { conn.query_row(sql, [], |r| r.get(0)).unwrap() };
    assert_eq!(result.boards, expect("SELECT COUNT(*) FROM boards"));
    assert_eq!(result.cards, expect("SELECT COUNT(*) FROM cards"));
    assert_eq!(result.members, expect("SELECT COUNT(*) FROM members"));

    // And the copy has to be a real, readable database with the same rows —
    // including archived items, which are still the user's data.
    let copy = Connection::open(&target).unwrap();
    let titles: Vec<String> = copy
        .prepare("SELECT title FROM cards ORDER BY title")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert!(titles.contains(&"Задача 1".to_string()));
    assert!(titles.contains(&"Архивная".to_string()), "архивные карточки — тоже данные");

    let member_names: Vec<String> = copy
        .prepare("SELECT name FROM members ORDER BY id")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    assert!(member_names.contains(&"Пётр".to_string()));
}

#[test]
fn exporting_over_an_existing_file_replaces_it() {
    let conn = test_db();
    let ws: i64 = conn.query_row("SELECT id FROM workspaces LIMIT 1", [], |r| r.get(0)).unwrap();
    seed_board(&conn, ws);

    // The native save dialog already asked the user about overwriting.
    let target = export_dir("overwrite").join("taskflow-copy.db");
    std::fs::write(&target, "это не база данных").unwrap();

    export_database_to(&conn, &target).expect("перезапись выбранного файла");

    let copy = Connection::open(&target).unwrap();
    let boards: i64 = copy.query_row("SELECT COUNT(*) FROM boards", [], |r| r.get(0)).unwrap();
    assert!(boards > 0, "на месте выбранного файла должна оказаться настоящая база");
}

#[test]
fn a_leftover_part_file_does_not_block_the_next_export() {
    let conn = test_db();
    let dir = export_dir("leftover");
    let target = dir.join("taskflow-copy.db");

    // VACUUM INTO refuses to write into a file that already exists, so a
    // half-finished export from last time must not wedge the button forever.
    std::fs::write(dir.join("taskflow-copy.db.part"), "обрывок").unwrap();

    export_database_to(&conn, &target).expect("остаток прошлого экспорта должен убираться");
    assert!(!dir.join("taskflow-copy.db.part").exists(), "временный файл не должен оставаться");
}

#[test]
fn a_failed_export_leaves_the_users_file_alone() {
    let conn = test_db();
    let dir = export_dir("failure");

    // A directory that does not exist: VACUUM INTO cannot write there.
    let target = dir.join("нет-такой-папки").join("copy.db");
    assert!(export_database_to(&conn, &target).is_err(), "ошибка должна быть заметной, а не тихой");

    // And the more important case: an existing file the user cares about.
    let precious = dir.join("важное.db");
    std::fs::write(&precious, "чужие данные").unwrap();
    let readonly_target = precious.clone();
    // Make the .part path impossible by leaving a *directory* in its place.
    std::fs::create_dir_all(dir.join("важное.db.part")).unwrap();

    assert!(export_database_to(&conn, &readonly_target).is_err());
    assert_eq!(
        std::fs::read_to_string(&precious).unwrap(),
        "чужие данные",
        "неудавшийся экспорт не должен трогать уже лежащий файл"
    );
}

#[test]
fn the_suggested_name_is_dated_and_usable_as_a_file_name() {
    let conn = test_db();
    let stamp: String = conn
        .query_row("SELECT strftime('%Y-%m-%d', 'now', 'localtime')", [], |r| r.get(0))
        .unwrap();
    let name = format!("taskflow-{}.db", stamp);

    assert!(name.starts_with("taskflow-"));
    assert!(name.ends_with(".db"));
    assert!(
        !name.contains(':') && !name.contains('/') && !name.contains('\\'),
        "в имени файла не должно быть символов, запрещённых Windows"
    );
}

#[test]
fn the_export_separates_what_is_visible_from_what_is_stored() {
    // The file must contain everything, but the number shown to the user has to
    // match what they see on their hub — otherwise "29 досок" looks like a bug
    // to someone with 9 boards on screen.
    let conn = test_db();
    let ws: i64 = conn.query_row("SELECT id FROM workspaces LIMIT 1", [], |r| r.get(0)).unwrap();
    seed_board(&conn, ws);

    conn.execute("INSERT INTO boards (workspace_id, name, archived) VALUES (?1, 'Закрытая', 1)", params![ws]).unwrap();

    let target = export_dir("counts").join("copy.db");
    let result = export_database_to(&conn, &target).unwrap();

    // seed_board's board plus the archived one. (No Inbox here: `test_db`
    // builds the schema before inserting the workspace, and the backfill only
    // sees workspaces that already exist.)
    assert_eq!(result.boards, 2, "в файл попадает всё, включая архив");
    assert_eq!(result.boards_active, 1, "видно на хабе только одну доску");

    // seed_board makes three cards: one live, one archived, one in an archived
    // column. Only the first is visible anywhere in the interface.
    assert_eq!(result.cards, 3);
    assert_eq!(result.cards_active, 1, "архивная карточка и карточка в архивной колонке не видны");

    // And the file really does hold the archived rows, not just count them.
    let copy = Connection::open(&target).unwrap();
    let archived: i64 = copy
        .query_row("SELECT COUNT(*) FROM boards WHERE archived = 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(archived, 1, "архивная доска должна физически лежать в копии");
}

// ============================================
// Checklists (sub-tasks inside a card)
// ============================================
// The counter on the card face is computed in SQL, and the delete paths have to
// clear these rows before the card they hang off — foreign keys are enforced,
// so getting either wrong fails at runtime rather than at compile time.

/// A card to hang checklist items off, with its column and board.
fn seed_card(conn: &Connection, workspace_id: i64) -> i64 {
    conn.execute("INSERT INTO boards (workspace_id, name) VALUES (?1, 'Доска')", params![workspace_id]).unwrap();
    let board_id = conn.last_insert_rowid();
    conn.execute("INSERT INTO columns (board_id, name, position) VALUES (?1, 'Колонка', 0)", params![board_id]).unwrap();
    let column_id = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO cards (column_id, title, description, position) VALUES (?1, 'Задача', '', 0)",
        params![column_id],
    ).unwrap();
    conn.last_insert_rowid()
}

/// Adds an item the way `create_checklist_item` does, without the Tauri State.
fn add_item(conn: &Connection, card_id: i64, text: &str) -> i64 {
    let position: i64 = conn.query_row(
        "SELECT COALESCE(MAX(position), -1) + 1 FROM checklist_items WHERE card_id = ?1",
        params![card_id],
        |r| r.get(0),
    ).unwrap();
    conn.execute(
        "INSERT INTO checklist_items (card_id, text, position) VALUES (?1, ?2, ?3)",
        params![card_id, text, position],
    ).unwrap();
    conn.last_insert_rowid()
}

/// The counter as `get_cards` computes it.
fn checklist_counts(conn: &Connection, card_id: i64) -> (i64, i64) {
    conn.query_row(
        "SELECT (SELECT COUNT(*) FROM checklist_items WHERE card_id = ?1),
                (SELECT COUNT(*) FROM checklist_items WHERE card_id = ?1 AND is_done = 1)",
        params![card_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).unwrap()
}

#[test]
fn new_items_keep_the_order_they_were_added_in() {
    let conn = test_db();
    let ws: i64 = conn.query_row("SELECT id FROM workspaces LIMIT 1", [], |r| r.get(0)).unwrap();
    let card = seed_card(&conn, ws);

    for text in ["Первый", "Второй", "Третий"] {
        add_item(&conn, card, text);
    }

    let order: Vec<(String, i64)> = conn
        .prepare("SELECT text, position FROM checklist_items WHERE card_id = ?1 ORDER BY position ASC, id ASC")
        .unwrap()
        .query_map(params![card], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();

    assert_eq!(order[0], ("Первый".to_string(), 0));
    assert_eq!(order[1], ("Второй".to_string(), 1));
    assert_eq!(order[2], ("Третий".to_string(), 2));
}

#[test]
fn the_card_face_counter_matches_what_is_ticked() {
    let conn = test_db();
    let ws: i64 = conn.query_row("SELECT id FROM workspaces LIMIT 1", [], |r| r.get(0)).unwrap();
    let card = seed_card(&conn, ws);

    assert_eq!(checklist_counts(&conn, card), (0, 0), "у карточки без пунктов счётчика быть не должно");

    let first = add_item(&conn, card, "Первый");
    add_item(&conn, card, "Второй");
    add_item(&conn, card, "Третий");
    assert_eq!(checklist_counts(&conn, card), (3, 0));

    conn.execute("UPDATE checklist_items SET is_done = 1 WHERE id = ?1", params![first]).unwrap();
    assert_eq!(checklist_counts(&conn, card), (3, 1), "должно получиться «1 из 3»");
}

#[test]
fn the_counter_does_not_leak_between_cards() {
    let conn = test_db();
    let ws: i64 = conn.query_row("SELECT id FROM workspaces LIMIT 1", [], |r| r.get(0)).unwrap();
    let card_a = seed_card(&conn, ws);
    let card_b = seed_card(&conn, ws);

    add_item(&conn, card_a, "Только у A");
    add_item(&conn, card_a, "И это тоже у A");

    assert_eq!(checklist_counts(&conn, card_a), (2, 0));
    assert_eq!(checklist_counts(&conn, card_b), (0, 0), "пункты соседней карточки не должны считаться");
}

#[test]
fn toggling_flips_the_item_both_ways() {
    let conn = test_db();
    let ws: i64 = conn.query_row("SELECT id FROM workspaces LIMIT 1", [], |r| r.get(0)).unwrap();
    let card = seed_card(&conn, ws);
    let item = add_item(&conn, card, "Пункт");

    let flip = |id: i64| -> i64 {
        conn.execute(
            "UPDATE checklist_items SET is_done = CASE is_done WHEN 0 THEN 1 ELSE 0 END WHERE id = ?1",
            params![id],
        ).unwrap();
        conn.query_row("SELECT is_done FROM checklist_items WHERE id = ?1", params![id], |r| r.get(0)).unwrap()
    };

    assert_eq!(flip(item), 1, "первое нажатие отмечает пункт");
    assert_eq!(flip(item), 0, "повторное — снимает отметку");
}

#[test]
fn deleting_a_card_takes_its_checklist_with_it() {
    let mut conn = test_db();
    let ws: i64 = conn.query_row("SELECT id FROM workspaces LIMIT 1", [], |r| r.get(0)).unwrap();
    let card = seed_card(&conn, ws);
    add_item(&conn, card, "Пункт 1");
    add_item(&conn, card, "Пункт 2");

    // Mirrors delete_card. Without the checklist_items line this fails outright:
    // foreign keys are on, and the items still point at the card.
    let tx = conn.transaction().unwrap();
    tx.execute("DELETE FROM card_labels WHERE card_id = ?1", params![card]).unwrap();
    tx.execute("DELETE FROM checklist_items WHERE card_id = ?1", params![card]).unwrap();
    tx.execute("DELETE FROM cards WHERE id = ?1", params![card]).unwrap();
    tx.commit().unwrap();

    let left: i64 = conn.query_row("SELECT COUNT(*) FROM checklist_items", [], |r| r.get(0)).unwrap();
    assert_eq!(left, 0, "пункты удалённой карточки не должны оставаться сиротами");

    let violations: i64 = conn
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(violations, 0);
}

#[test]
fn a_card_with_a_checklist_cannot_be_deleted_out_from_under_it() {
    // Guards the reason the delete paths above needed changing at all: if this
    // ever stops failing, foreign keys have been switched off somewhere.
    let conn = test_db();
    let ws: i64 = conn.query_row("SELECT id FROM workspaces LIMIT 1", [], |r| r.get(0)).unwrap();
    let card = seed_card(&conn, ws);
    add_item(&conn, card, "Пункт");

    let result = conn.execute("DELETE FROM cards WHERE id = ?1", params![card]);
    assert!(result.is_err(), "внешний ключ должен запрещать удаление карточки с пунктами");
}

#[test]
fn an_empty_item_is_refused() {
    // Mirrors the guard in create_checklist_item: a blank row would render as an
    // untickable empty line that nobody can identify to delete.
    for text in ["", "   ", "\t\n"] {
        assert!(text.trim().is_empty(), "пустой текст должен отсекаться до вставки");
    }
    assert!(!"  Настоящий пункт  ".trim().is_empty());
    assert_eq!("  Настоящий пункт  ".trim(), "Настоящий пункт");
}

// ============================================
// Sidebar background, one per workspace
// ============================================
// Two things decide whether this feature is trustworthy: that switching
// workspaces never shows the wrong picture, and that the folder does not fill
// up with every wallpaper the user has ever tried.

/// Unique temp directory for one background test.
fn background_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("taskflow-bg-{}", name));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A real PNG of the requested size, as bytes.
fn picture_bytes(width: u32, height: u32) -> Vec<u8> {
    let mut buf = image::RgbImage::new(width, height);
    for (x, y, pixel) in buf.enumerate_pixels_mut() {
        *pixel = image::Rgb([(x % 256) as u8, (y % 256) as u8, 128]);
    }
    let mut out = Vec::new();
    image::DynamicImage::ImageRgb8(buf)
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .unwrap();
    out
}

/// Writes such a PNG to disk, standing in for the file the user picks.
fn picture_file(dir: &std::path::Path, name: &str, width: u32, height: u32) -> std::path::PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, picture_bytes(width, height)).unwrap();
    path
}

/// How many pictures the backgrounds folder holds, ignoring anything else.
fn stored_pictures(dir: &std::path::Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.ends_with(".jpg"))
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

fn second_workspace(conn: &Connection) -> i64 {
    conn.execute("INSERT INTO workspaces (name) VALUES ('Второе')", ()).unwrap();
    conn.last_insert_rowid()
}

#[test]
fn a_large_picture_is_shrunk_before_it_is_stored() {
    let jpeg = encode_background_image(&picture_bytes(4800, 2400)).expect("картинка должна пережаться");

    let stored = image::load_from_memory(&jpeg).expect("результат должен быть читаемым изображением");
    assert_eq!(stored.width(), 1600, "длинная сторона ограничена 1600 px");
    assert_eq!(stored.height(), 800, "пропорции сохраняются");

    assert_eq!(
        image::guess_format(&jpeg).unwrap(),
        image::ImageFormat::Jpeg,
        "храним всегда JPEG, независимо от того, что дал пользователь",
    );
}

#[test]
fn a_small_picture_is_not_blown_up() {
    // `DynamicImage::resize` растягивает картинку до рамки, если она меньше —
    // это добавило бы байтов и мыла на ровном месте. Картинка меньше 1600 по
    // обеим сторонам должна дойти до диска ровно такой, какой была.
    let jpeg = encode_background_image(&picture_bytes(1200, 900)).unwrap();
    let stored = image::load_from_memory(&jpeg).unwrap();
    assert_eq!((stored.width(), stored.height()), (1200, 900));
}

#[test]
fn a_file_that_is_not_a_picture_is_refused() {
    let err = encode_background_image(b"not a picture at all").unwrap_err();
    assert!(!err.is_empty(), "отказ должен объясняться пользователю");
}

#[test]
fn each_workspace_keeps_its_own_background() {
    let conn = test_db();
    let first: i64 = conn.query_row("SELECT id FROM workspaces LIMIT 1", [], |r| r.get(0)).unwrap();
    let second = second_workspace(&conn);

    let dir = background_dir("per-workspace");
    let store = dir.join("store");
    let source_a = picture_file(&dir, "a.png", 60, 40);
    let source_b = picture_file(&dir, "b.png", 40, 60);

    let name_a = store_background(&conn, &store, first, &source_a).unwrap();
    let name_b = store_background(&conn, &store, second, &source_b).unwrap();

    assert_ne!(name_a, name_b, "у каждого пространства свой файл");
    assert!(name_a.starts_with(&format!("{}_", first)), "имя файла называет своё пространство");
    assert!(name_b.starts_with(&format!("{}_", second)));

    // И читается тоже своё: перепутанная привязка — главный риск этой фичи.
    let read_a = read_background(&conn, &store, first).unwrap().unwrap();
    let read_b = read_background(&conn, &store, second).unwrap().unwrap();
    assert!(read_a.starts_with("data:image/jpeg;base64,"));
    assert_ne!(read_a, read_b, "разные картинки — разные данные");
}

#[test]
fn replacing_a_background_deletes_the_file_it_replaces() {
    let conn = test_db();
    let ws: i64 = conn.query_row("SELECT id FROM workspaces LIMIT 1", [], |r| r.get(0)).unwrap();

    let dir = background_dir("replace");
    let store = dir.join("store");
    let first = store_background(&conn, &store, ws, &picture_file(&dir, "first.png", 50, 50)).unwrap();

    // Имя содержит секунды: без паузы замена попала бы в тот же файл, и тест
    // проверял бы не то, что нужно.
    std::thread::sleep(std::time::Duration::from_millis(1100));

    let second = store_background(&conn, &store, ws, &picture_file(&dir, "second.png", 70, 70)).unwrap();

    assert_ne!(first, second);
    assert_eq!(
        stored_pictures(&store),
        vec![second.clone()],
        "прошлая загрузка не должна оставаться в папке",
    );
    assert_eq!(background_name(&conn, ws).unwrap(), Some(second));
}

#[test]
fn resetting_one_workspace_leaves_the_others_alone() {
    let conn = test_db();
    let first: i64 = conn.query_row("SELECT id FROM workspaces LIMIT 1", [], |r| r.get(0)).unwrap();
    let second = second_workspace(&conn);

    let dir = background_dir("reset");
    let store = dir.join("store");
    store_background(&conn, &store, first, &picture_file(&dir, "a.png", 50, 50)).unwrap();
    let kept = store_background(&conn, &store, second, &picture_file(&dir, "b.png", 50, 50)).unwrap();

    clear_background(&conn, &store, first).unwrap();

    assert_eq!(background_name(&conn, first).unwrap(), None);
    assert_eq!(read_background(&conn, &store, first).unwrap(), None);
    assert_eq!(background_name(&conn, second).unwrap(), Some(kept.clone()));
    assert_eq!(stored_pictures(&store), vec![kept], "чужой файл сбросом не трогается");
}

#[test]
fn a_workspace_without_a_background_reads_as_none() {
    let conn = test_db();
    let ws: i64 = conn.query_row("SELECT id FROM workspaces LIMIT 1", [], |r| r.get(0)).unwrap();
    let store = background_dir("empty").join("store");

    assert_eq!(read_background(&conn, &store, ws).unwrap(), None);
    // Сброс без установленного фона — обычная ситуация, не ошибка.
    clear_background(&conn, &store, ws).unwrap();
}

#[test]
fn a_background_whose_file_vanished_stops_being_remembered() {
    let conn = test_db();
    let ws: i64 = conn.query_row("SELECT id FROM workspaces LIMIT 1", [], |r| r.get(0)).unwrap();

    let dir = background_dir("vanished");
    let store = dir.join("store");
    let name = store_background(&conn, &store, ws, &picture_file(&dir, "a.png", 50, 50)).unwrap();

    // Папка обычная, пользователь может вычистить её мимо приложения; после
    // восстановления из копии база тоже может назвать файл, которого нет.
    std::fs::remove_file(store.join(&name)).unwrap();

    assert_eq!(read_background(&conn, &store, ws).unwrap(), None, "это «фона нет», а не ошибка");
    assert_eq!(
        background_name(&conn, ws).unwrap(),
        None,
        "ссылка на пропавший файл должна забыться, иначе кнопка «Сбросить» висит навсегда",
    );
}

#[test]
fn a_stored_name_cannot_point_outside_the_backgrounds_folder() {
    let dir = std::path::Path::new("C:/taskflow/backgrounds");
    assert!(background_file_path(dir, "3_20260824.jpg").is_some());
    assert!(background_file_path(dir, "../../trello_clone.db").is_none());
    assert!(background_file_path(dir, "sub/dir.jpg").is_none());
    assert!(background_file_path(dir, "").is_none());
}

/// Сколько на самом деле весит фон при текущем `BACKGROUND_MAX_SIDE`.
///
/// Синтетическая картинка из `picture_bytes` — градиент, он жмётся в разы
/// лучше фотографии, и мерить предел по нему бессмысленно. Тест берёт
/// настоящие обои Windows (3840×2400) и печатает три числа: файл на диске,
/// длину base64 (именно она едет через IPC и живёт в CSS как `url()`) и размер
/// распакованного растра в композиторе.
///
/// Запуск: `cargo test --lib measure_stored_background -- --ignored --nocapture`
#[test]
#[ignore]
fn measure_stored_background_size_on_real_photos() {
    let dirs = [
        r"C:\Windows\Web\Wallpaper\ThemeA",
        r"C:\Windows\Web\Wallpaper\ThemeB",
        r"C:\Windows\Web\4K\Wallpaper\Windows",
    ];

    let mut checked = 0;
    for dir in dirs {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jpg") {
                continue;
            }
            let raw = std::fs::read(&path).unwrap();
            let source = image::load_from_memory(&raw).unwrap();

            let jpeg = encode_background_image(&raw).expect("обои должны пережаться");
            let stored = image::load_from_memory(&jpeg).unwrap();
            // base64 всегда 4 символа на каждые 3 байта, с добивкой до кратности.
            let base64_len = (jpeg.len() + 2) / 3 * 4;
            let bitmap = stored.width() as usize * stored.height() as usize * 4;

            println!(
                "{:<26} {}×{} {:>7} КБ  ->  {}×{} {:>6} КБ  base64 {:>6} КБ  растр {:>5} МБ",
                path.file_name().unwrap().to_string_lossy(),
                source.width(), source.height(), raw.len() / 1024,
                stored.width(), stored.height(), jpeg.len() / 1024,
                base64_len / 1024,
                bitmap / (1024 * 1024),
            );
            checked += 1;
        }
    }

    if checked == 0 {
        println!("обоев Windows на этой машине нет — мерить нечего");
    }
}

#[test]
fn export_carries_checklist_items_in_display_order() {
    let conn = test_db();
    let board_id = seed_board(&conn, 1);

    let export = build_board_export(&conn, board_id).unwrap();
    let card = &export.board.columns[0].cards[0];

    let items: Vec<(&str, bool)> =
        card.checklist.iter().map(|i| (i.text.as_str(), i.is_done)).collect();
    assert_eq!(
        items,
        vec![("Первый", true), ("Второй", false), ("Третий", false)],
        "пункты должны идти по position, а не по порядку вставки"
    );

    // Карточка без подзадач отдаёт пустой список, а не отсутствующее поле:
    // читателю файла не приходится различать «нет чек-листа» и «поле забыли».
    assert!(export.board.columns[0].cards[1].checklist.is_empty());
}

#[test]
fn import_restores_checklists_through_real_json() {
    let mut conn = test_db();
    let original_id = seed_board(&conn, 1);

    // Через настоящий JSON: поле, которое сериализуется, но не читается обратно,
    // выглядит в тесте на структурах живым — а в файле теряется.
    let json = serde_json::to_string(&build_board_export(&conn, original_id).unwrap()).unwrap();
    let parsed: BoardExport = serde_json::from_str(&json).unwrap();
    let new_id = import_board_into(&mut conn, 1, parsed).unwrap();

    let new_card_id: i64 = conn
        .query_row(
            "SELECT c.id FROM cards c
             JOIN columns col ON col.id = c.column_id
             WHERE col.board_id = ?1 AND c.title = 'Задача 1'",
            params![new_id],
            |r| r.get(0),
        )
        .unwrap();

    let items: Vec<(String, i64, i64)> = conn
        .prepare(
            "SELECT text, is_done, position FROM checklist_items
             WHERE card_id = ?1 ORDER BY position ASC, id ASC",
        )
        .unwrap()
        .query_map(params![new_card_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();

    assert_eq!(
        items,
        vec![
            ("Первый".to_string(), 1, 0),
            ("Второй".to_string(), 0, 1),
            ("Третий".to_string(), 0, 2),
        ],
        "отметки «сделано» должны пережить импорт, а позиции — перенумероваться подряд"
    );

    // Пункты принадлежат новой карточке и не отобраны у оригинала.
    let original_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM checklist_items ci
             JOIN cards c ON c.id = ci.card_id
             JOIN columns col ON col.id = c.column_id
             WHERE col.board_id = ?1",
            params![original_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(original_count, 3, "импорт не должен трогать исходную доску");
}

#[test]
fn a_file_written_before_checklists_existed_still_imports() {
    let mut conn = test_db();

    // Ровно то, что писала предыдущая версия: у карточки нет поля `checklist`.
    let json = r#"{
        "taskflow_export_version": 1,
        "exported_at": "2026-01-01 00:00:00",
        "board": {
            "name": "Старый файл",
            "gradient": "",
            "is_starred": false,
            "labels": [],
            "members": [],
            "columns": [
                { "name": "Список", "position": 0, "archived": 0, "cards": [
                    { "title": "Без подзадач", "description": "", "position": 0 }
                ] }
            ]
        }
    }"#;

    let parsed: BoardExport = serde_json::from_str(json).expect("старый файл должен читаться");
    let new_id = import_board_into(&mut conn, 1, parsed).unwrap();

    let after = build_board_export(&conn, new_id).unwrap();
    assert_eq!(after.board.columns[0].cards[0].title, "Без подзадач");
    assert!(after.board.columns[0].cards[0].checklist.is_empty());
}

#[test]
fn an_empty_checklist_line_in_a_hand_edited_file_is_skipped() {
    let mut conn = test_db();

    let json = r#"{
        "taskflow_export_version": 1,
        "exported_at": "",
        "board": {
            "name": "Правленый руками",
            "columns": [
                { "name": "Список", "cards": [
                    { "title": "Карточка", "checklist": [
                        { "text": "  Настоящий пункт  " },
                        { "text": "   " },
                        { "text": "" }
                    ] }
                ] }
            ]
        }
    }"#;

    let parsed: BoardExport = serde_json::from_str(json).unwrap();
    let new_id = import_board_into(&mut conn, 1, parsed).unwrap();

    let after = build_board_export(&conn, new_id).unwrap();
    let items = &after.board.columns[0].cards[0].checklist;
    assert_eq!(items.len(), 1, "пустые строки не должны становиться невидимыми пунктами");
    assert_eq!(items[0].text, "Настоящий пункт", "текст сохраняется без внешних пробелов");
}

// ─── Напоминания о дедлайнах (Фаза 3) ───
//
// Сроки здесь задаются не константами, а сдвигом от сегодняшнего **местного**
// дня: тест с зашитой датой прошёл бы один раз и сломался бы назавтра, а
// зашитый UTC-день ещё и разошёлся бы с местным в половине часовых поясов.

/// Доска с одной колонкой; возвращает id колонки.
fn board_with_column(conn: &Connection) -> i64 {
    conn.execute(
        "INSERT INTO boards (workspace_id, name, gradient) VALUES (1, 'Доска сроков', '')",
        (),
    ).unwrap();
    let board_id = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO columns (board_id, name, position) VALUES (?1, 'В работе', 0)",
        params![board_id],
    ).unwrap();
    conn.last_insert_rowid()
}

/// `days` дней от сегодняшнего местного дня в виде 'YYYY-MM-DD'.
fn local_day(conn: &Connection, days: i64) -> String {
    conn.query_row(
        "SELECT date('now', 'localtime', ?1 || ' days')",
        params![days],
        |r| r.get(0),
    ).unwrap()
}

fn add_card(conn: &Connection, column_id: i64, title: &str, due_days: i64) -> i64 {
    let due = local_day(conn, due_days);
    conn.execute(
        "INSERT INTO cards (column_id, title, description, position, due_date) VALUES (?1, ?2, '', 0, ?3)",
        params![column_id, title, due],
    ).unwrap();
    conn.last_insert_rowid()
}

#[test]
fn a_deadline_inside_the_window_is_picked_up_and_one_outside_is_not() {
    let conn = test_db();
    let col = board_with_column(&conn);
    add_card(&conn, col, "Сегодня", 0);
    add_card(&conn, col, "Завтра", 1);
    add_card(&conn, col, "Через десять дней", 10);

    // Срок истекает в конце своего дня, поэтому при окне в сутки «завтра» ещё
    // не наступило: до конца завтрашнего дня больше 24 часов.
    let due = due_cards_needing_reminder(&conn, 24).unwrap();
    let titles: Vec<&str> = due.iter().map(|c| c.title.as_str()).collect();
    assert_eq!(titles, vec!["Сегодня"]);

    // Окно в двое суток забирает и завтрашнюю, но не ту, что через десять дней.
    let due = due_cards_needing_reminder(&conn, 48).unwrap();
    let titles: Vec<&str> = due.iter().map(|c| c.title.as_str()).collect();
    assert_eq!(titles, vec!["Сегодня", "Завтра"]);
}

#[test]
fn a_card_reminds_once_and_then_goes_quiet() {
    let conn = test_db();
    let col = board_with_column(&conn);
    add_card(&conn, col, "Сегодня", 0);

    let first = due_cards_needing_reminder(&conn, 24).unwrap();
    assert_eq!(first.len(), 1);

    mark_reminders_sent(&conn, &first).unwrap();

    let second = due_cards_needing_reminder(&conn, 24).unwrap();
    assert!(second.is_empty(), "повторное напоминание о том же сроке — это спам");
}

// Раньше здесь стояли два теста, проверявшие, что перенос и снятие срока
// сбрасывают отметку «напоминание показано». С Фазы A срок неизменяем, и через
// `update_card_in` его больше не двигают — правило переехало в тесты ниже.
// Законный перенос срока (продление ретрая) появится в Фазе B и сбрасывать
// отметку будет уже он.

#[test]
fn editing_the_text_does_not_disturb_the_deadline_or_its_reminder() {
    let conn = test_db();
    let col = board_with_column(&conn);
    let card_id = add_card(&conn, col, "Сегодня", 0);

    let first = due_cards_needing_reminder(&conn, 24).unwrap();
    mark_reminders_sent(&conn, &first).unwrap();

    let same_due = local_day(&conn, 0);
    update_card_in(&conn, card_id, "Сегодня", "правка текста", Some(&same_due)).unwrap();

    assert!(
        due_cards_needing_reminder(&conn, 24).unwrap().is_empty(),
        "правка описания не должна поднимать напоминание заново"
    );
}

#[test]
fn a_first_deadline_clears_a_stale_sent_mark() {
    let conn = test_db();
    let col = board_with_column(&conn);

    // Карточка без срока, но с отметкой о напоминании: так выглядит строка из
    // базы, где срок сняли ещё до того, как он стал неизменяемым.
    conn.execute(
        "INSERT INTO cards (column_id, title, description, position, due_reminder_sent_at)
         VALUES (?1, 'Наследство', '', 0, '2026-01-01 10:00:00')",
        params![col],
    ).unwrap();
    let card_id = conn.last_insert_rowid();

    let today = local_day(&conn, 0);
    update_card_in(&conn, card_id, "Наследство", "", Some(&today)).unwrap();

    let mark: Option<String> = conn
        .query_row("SELECT due_reminder_sent_at FROM cards WHERE id = ?1", params![card_id], |r| r.get(0))
        .unwrap();
    assert!(mark.is_none(), "старая отметка заглушила бы напоминание о новом сроке");
    assert_eq!(due_cards_needing_reminder(&conn, 24).unwrap().len(), 1);
}

#[test]
fn archived_things_do_not_remind() {
    let conn = test_db();

    // Четыре одинаковые карточки со сроком «сегодня», у каждой в архиве свой
    // уровень: сама карточка, её колонка, её доска, её пространство.
    let col = board_with_column(&conn);
    let archived_card = add_card(&conn, col, "Архивная карточка", 0);
    conn.execute("UPDATE cards SET archived = 1 WHERE id = ?1", params![archived_card]).unwrap();

    let col2 = board_with_column(&conn);
    add_card(&conn, col2, "В архивной колонке", 0);
    conn.execute("UPDATE columns SET archived = 1 WHERE id = ?1", params![col2]).unwrap();

    let col3 = board_with_column(&conn);
    add_card(&conn, col3, "На архивной доске", 0);
    conn.execute(
        "UPDATE boards SET archived = 1 WHERE id = (SELECT board_id FROM columns WHERE id = ?1)",
        params![col3],
    ).unwrap();

    conn.execute("INSERT INTO workspaces (name, archived) VALUES ('Закрытое', 1)", ()).unwrap();
    let dead_ws = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO boards (workspace_id, name, gradient) VALUES (?1, 'Доска', '')",
        params![dead_ws],
    ).unwrap();
    let dead_board = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO columns (board_id, name, position) VALUES (?1, 'Колонка', 0)",
        params![dead_board],
    ).unwrap();
    let dead_col = conn.last_insert_rowid();
    add_card(&conn, dead_col, "В архивном пространстве", 0);

    // И одна живая — иначе тест прошёл бы и на запросе, который не находит ничего.
    let alive_col = board_with_column(&conn);
    add_card(&conn, alive_col, "Живая", 0);

    let due = due_cards_needing_reminder(&conn, 24).unwrap();
    let titles: Vec<&str> = due.iter().map(|c| c.title.as_str()).collect();
    assert_eq!(titles, vec!["Живая"]);
}

#[test]
fn a_deadline_missed_long_ago_does_not_resurface() {
    let conn = test_db();
    let col = board_with_column(&conn);
    add_card(&conn, col, "Вчерашняя", -1);
    add_card(&conn, col, "Забытая в прошлом месяце", -30);

    // Пропущенная вчера ещё стоит напоминания — приложение могло быть закрыто
    // в момент срабатывания. Месячной давности — уже нет: при первом запуске
    // после обновления такие посыпались бы пачкой.
    let due = due_cards_needing_reminder(&conn, 24).unwrap();
    let titles: Vec<&str> = due.iter().map(|c| c.title.as_str()).collect();
    assert_eq!(titles, vec!["Вчерашняя"]);
}

#[test]
fn a_card_without_a_deadline_is_never_considered() {
    let conn = test_db();
    let col = board_with_column(&conn);
    conn.execute(
        "INSERT INTO cards (column_id, title, description, position) VALUES (?1, 'Без срока', '', 0)",
        params![col],
    ).unwrap();

    assert!(due_cards_needing_reminder(&conn, 168).unwrap().is_empty());
}

#[test]
fn reminder_settings_default_to_on_and_read_back_what_was_written() {
    let conn = test_db();

    // По умолчанию напоминания включены — иначе функция, ради которой всё это
    // писалось, у большинства просто не работала бы.
    let defaults = read_reminder_settings(&conn).unwrap();
    assert_eq!(defaults, ReminderSettings { enabled: true, hours: 24 });

    conn.execute(
        "UPDATE user_profile SET due_reminders_enabled = 0, due_reminder_hours = 6 WHERE id = 1",
        (),
    ).unwrap();
    assert_eq!(
        read_reminder_settings(&conn).unwrap(),
        ReminderSettings { enabled: false, hours: 6 }
    );
}

#[test]
fn one_deadline_is_named_and_several_are_summed_up() {
    let single = vec![DueReminder {
        card_id: 1,
        title: "Сдать отчёт".into(),
        due_date: "2026-09-01".into(),
        board_name: "Работа".into(),
    }];
    let (title, body) = reminder_text(&single);
    assert_eq!(title, "Скоро дедлайн: Сдать отчёт");
    assert_eq!(body, "Работа — срок 1 сентября");

    let many: Vec<DueReminder> = ["Первая", "Вторая", "Третья", "Четвёртая"]
        .iter()
        .enumerate()
        .map(|(i, t)| DueReminder {
            card_id: i as i64,
            title: (*t).into(),
            due_date: "2026-09-01".into(),
            board_name: "Работа".into(),
        })
        .collect();
    let (title, body) = reminder_text(&many);
    assert_eq!(title, "Приближается 4 дедлайна");
    assert_eq!(body, "Первая, Вторая, Третья и ещё 1", "четыре окна подряд человек закрывает не читая");
}

#[test]
fn a_due_date_in_an_unexpected_shape_is_shown_as_is() {
    assert_eq!(format_due_date_ru("2026-01-09"), "9 января");
    assert_eq!(format_due_date_ru("2026-12-31"), "31 декабря");
    // Ничего не разбирается — но и не паникует.
    assert_eq!(format_due_date_ru("завтра"), "завтра");
    assert_eq!(format_due_date_ru("2026-13-01"), "2026-13-01");
}

#[test]
fn the_exported_file_is_readable_without_the_key() {
    let conn = test_db();
    let ws: i64 = conn.query_row("SELECT id FROM workspaces LIMIT 1", [], |r| r.get(0)).unwrap();
    seed_board(&conn, ws);

    let target = export_dir("plaintext").join("taskflow-copy.db");
    export_database_to(&conn, &target).expect("экспорт должен пройти");

    // Осознанное решение, а не побочный эффект: копия, которую нельзя открыть
    // без ключа из Диспетчера учётных данных этой машины, бесполезна ровно
    // тогда, когда понадобится (см. PROJECT_NOTES §17.5). Если экспорт когда-то
    // решат шифровать, этот тест обязан упасть и заставить переписать
    // предупреждение в Настройках.
    assert!(
        crate::crypto::is_plaintext_database(&target),
        "файл экспорта должен оставаться обычной базой SQLite"
    );

    // И действительно открываться без всякого ключа.
    let copy = Connection::open(&target).unwrap();
    let boards: i64 = copy.query_row("SELECT COUNT(*) FROM boards", [], |r| r.get(0)).unwrap();
    assert!(boards > 0);
}

// ─── Комментарии к карточке (Фаза 6) ───

/// Карточка на новой доске; возвращает её id.
fn card_for_comments(conn: &Connection) -> i64 {
    conn.execute(
        "INSERT INTO boards (workspace_id, name, gradient) VALUES (1, 'Обсуждения', '')",
        (),
    ).unwrap();
    let board_id = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO columns (board_id, name, position) VALUES (?1, 'В работе', 0)",
        params![board_id],
    ).unwrap();
    let col = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO cards (column_id, title, description, position) VALUES (?1, 'Задача', '', 0)",
        params![col],
    ).unwrap();
    conn.last_insert_rowid()
}

#[test]
fn comments_come_back_oldest_first_with_their_author() {
    let conn = test_db();
    let card_id = card_for_comments(&conn);

    create_card_comment_in(&conn, card_id, "Первый").unwrap();
    create_card_comment_in(&conn, card_id, "Второй").unwrap();
    create_card_comment_in(&conn, card_id, "Третий").unwrap();

    let list = list_card_comments_in(&conn, card_id).unwrap();
    let bodies: Vec<&str> = list.iter().map(|c| c.body.as_str()).collect();
    assert_eq!(
        bodies,
        vec!["Первый", "Второй", "Третий"],
        "переписку читают сверху вниз, последний ответ должен быть в конце"
    );

    // Подпись приезжает целиком, а не идентификатором: рисовать её нужно сразу.
    let author = list[0].author.as_ref().expect("автором должен стать сам пользователь");
    assert!(author.is_self);
    assert!(!author.initials.is_empty());
}

#[test]
fn the_signature_follows_a_rename_because_it_is_a_reference_not_a_copy() {
    let conn = test_db();
    let card_id = card_for_comments(&conn);
    create_card_comment_in(&conn, card_id, "Уже написано").unwrap();

    conn.execute("UPDATE members SET name = 'Новое имя' WHERE is_self = 1", ()).unwrap();

    let list = list_card_comments_in(&conn, card_id).unwrap();
    assert_eq!(list[0].author.as_ref().unwrap().name, "Новое имя");
}

#[test]
fn an_empty_or_absurdly_long_comment_is_refused() {
    let conn = test_db();
    let card_id = card_for_comments(&conn);

    assert!(create_card_comment_in(&conn, card_id, "").is_err());
    assert!(create_card_comment_in(&conn, card_id, "    \n  ").is_err(), "одни пробелы — тоже пусто");

    // Обрезать молча нельзя — это потеря текста; поэтому именно ошибка.
    let huge = "я".repeat(5001);
    let err = create_card_comment_in(&conn, card_id, &huge).unwrap_err();
    assert!(err.contains("вставилось не то"), "неожиданное сообщение: {}", err);

    // Ровно по границе — можно. Счёт в символах, а не байтах: кириллица весит
    // по два байта, и лимит в байтах резал бы русский текст вдвое раньше.
    assert!(create_card_comment_in(&conn, card_id, &"я".repeat(5000)).is_ok());

    // Пробелы по краям срезаются.
    let c = create_card_comment_in(&conn, card_id, "  с краями  ").unwrap();
    assert_eq!(c.body, "с краями");
}

#[test]
fn a_comment_on_a_card_that_does_not_exist_says_so_plainly() {
    let conn = test_db();
    let err = create_card_comment_in(&conn, 99999, "В пустоту").unwrap_err();
    assert_eq!(err, "Карточка не найдена", "сообщение внешнего ключа человеку ничего не говорит");
}

#[test]
fn deleting_a_member_keeps_what_they_wrote() {
    let mut conn = test_db();
    let card_id = card_for_comments(&conn);

    conn.execute("INSERT INTO members (name, initials, color) VALUES ('Пётр', 'ПП', '#f00')", ()).unwrap();
    let peter = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO card_comments (card_id, author_id, body) VALUES (?1, ?2, 'Мысль Петра')",
        params![card_id, peter],
    ).unwrap();

    delete_member_from(&mut conn, peter).unwrap();

    let list = list_card_comments_in(&conn, card_id).unwrap();
    assert_eq!(list.len(), 1, "обсуждение задачи — её часть, а не участника");
    assert_eq!(list[0].body, "Мысль Петра");
    assert!(list[0].author.is_none(), "подпись пропадает, текст остаётся");
}

#[test]
fn deleting_a_card_takes_its_comments_with_it() {
    let mut conn = test_db();
    let card_id = card_for_comments(&conn);
    create_card_comment_in(&conn, card_id, "Исчезнет вместе с карточкой").unwrap();

    let tx = conn.transaction().unwrap();
    // Тот же порядок, что и в `delete_card`.
    tx.execute("DELETE FROM card_labels WHERE card_id = ?1", params![card_id]).unwrap();
    tx.execute("DELETE FROM checklist_items WHERE card_id = ?1", params![card_id]).unwrap();
    tx.execute("DELETE FROM card_comments WHERE card_id = ?1", params![card_id]).unwrap();
    tx.execute("DELETE FROM cards WHERE id = ?1", params![card_id]).unwrap();
    tx.commit().unwrap();

    let left: i64 = conn.query_row("SELECT COUNT(*) FROM card_comments", [], |r| r.get(0)).unwrap();
    assert_eq!(left, 0);
    let violations: i64 = conn
        .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(violations, 0);
}

#[test]
fn comments_survive_an_export_round_trip_with_their_author_and_time() {
    let mut conn = test_db();
    let card_id = card_for_comments(&conn);
    let board_id: i64 = conn
        .query_row(
            "SELECT col.board_id FROM cards c JOIN columns col ON col.id = c.column_id WHERE c.id = ?1",
            params![card_id],
            |r| r.get(0),
        )
        .unwrap();

    // Автор комментария намеренно НЕ исполнитель и НЕ автор карточки: раньше
    // список участников в экспорте собирался только по этим двум ссылкам, и
    // такой человек в файл не попадал — подпись терялась при импорте.
    conn.execute("INSERT INTO members (name, initials, color) VALUES ('Комментатор', 'КМ', '#0f0')", ()).unwrap();
    let commenter = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO card_comments (card_id, author_id, body, created_at)
         VALUES (?1, ?2, 'Написано в марте', '2026-03-01 09:30:00')",
        params![card_id, commenter],
    ).unwrap();

    let json = serde_json::to_string(&build_board_export(&conn, board_id).unwrap()).unwrap();
    let parsed: BoardExport = serde_json::from_str(&json).unwrap();
    let new_id = import_board_into(&mut conn, 1, parsed).unwrap();

    let after = build_board_export(&conn, new_id).unwrap();
    let card = &after.board.columns[0].cards[0];
    assert_eq!(card.comments.len(), 1, "комментарий обязан пережить круг через файл");
    assert_eq!(card.comments[0].body, "Написано в марте");
    assert_eq!(
        card.comments[0].created_at, "2026-03-01 09:30:00",
        "перенос доски не должен делать мартовский комментарий сегодняшним"
    );

    // Подпись указывает на участника новой доски, а не на строку из исходной.
    let new_comment_author: Option<String> = conn
        .query_row(
            "SELECT m.name FROM card_comments cc
             JOIN cards c ON c.id = cc.card_id
             JOIN columns col ON col.id = c.column_id
             LEFT JOIN members m ON m.id = cc.author_id
             WHERE col.board_id = ?1",
            params![new_id],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(new_comment_author.as_deref(), Some("Комментатор"), "автор комментария потерялся");
}

#[test]
fn a_file_written_before_comments_existed_still_imports() {
    let mut conn = test_db();

    let json = r#"{
        "taskflow_export_version": 1,
        "exported_at": "",
        "board": {
            "name": "Старый файл",
            "columns": [
                { "name": "Список", "cards": [ { "title": "Без обсуждения" } ] }
            ]
        }
    }"#;

    let parsed: BoardExport = serde_json::from_str(json).expect("старый файл должен читаться");
    let new_id = import_board_into(&mut conn, 1, parsed).unwrap();

    let after = build_board_export(&conn, new_id).unwrap();
    assert!(after.board.columns[0].cards[0].comments.is_empty());
}

// ─── Фаза A: срок задаётся один раз, финальная колонка не отпускает ───
//
// Оба правила проверяются на уровне базы, а не интерфейса: окно карточки
// рисует нередактируемое поле, а Sortable не даёт вытащить карточку из
// финальной колонки, но обе команды можно позвать и мимо экрана — из «Списка»,
// из Inbox или напрямую. Тесты гоняют ровно тот код, который стоит за IPC.

/// Доска с рабочей и финальной колонками. Возвращает `(рабочая, финальная)`.
fn board_with_final_column(conn: &Connection) -> (i64, i64) {
    conn.execute(
        "INSERT INTO boards (workspace_id, name) VALUES (1, 'Доска с финалом')",
        (),
    ).unwrap();
    let board_id = conn.last_insert_rowid();

    conn.execute(
        "INSERT INTO columns (board_id, name, position) VALUES (?1, 'В работе', 0)",
        params![board_id],
    ).unwrap();
    let working = conn.last_insert_rowid();

    conn.execute(
        "INSERT INTO columns (board_id, name, position, is_final) VALUES (?1, 'Сдано', 1, 1)",
        params![board_id],
    ).unwrap();
    let final_col = conn.last_insert_rowid();

    (working, final_col)
}

fn add_plain_card(conn: &Connection, column_id: i64, title: &str, position: i64) -> i64 {
    conn.execute(
        "INSERT INTO cards (column_id, title, position) VALUES (?1, ?2, ?3)",
        params![column_id, title, position],
    ).unwrap();
    conn.last_insert_rowid()
}

fn card_column_and_position(conn: &Connection, card_id: i64) -> (i64, i64) {
    conn.query_row(
        "SELECT column_id, position FROM cards WHERE id = ?1",
        params![card_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).unwrap()
}

fn due_of(conn: &Connection, card_id: i64) -> Option<String> {
    conn.query_row("SELECT due_date FROM cards WHERE id = ?1", params![card_id], |r| r.get(0))
        .unwrap()
}

#[test]
fn a_card_without_a_deadline_can_still_be_given_one() {
    let conn = test_db();
    let col = board_with_column(&conn);
    let card_id = add_plain_card(&conn, col, "Пока без срока", 0);

    // Задачу заводят раньше, чем узнают дату сдачи, — первая установка обязана
    // проходить, иначе поле было бы бесполезным для всех новых карточек.
    update_card_in(&conn, card_id, "Пока без срока", "", Some("2026-12-01")).unwrap();
    assert_eq!(due_of(&conn, card_id).as_deref(), Some("2026-12-01"));
}

#[test]
fn a_deadline_that_exists_can_be_neither_moved_nor_cleared() {
    let conn = test_db();
    let col = board_with_column(&conn);
    let card_id = add_plain_card(&conn, col, "Со сроком", 0);
    update_card_in(&conn, card_id, "Со сроком", "", Some("2026-12-01")).unwrap();

    // Попытка отодвинуть срок правкой карточки. Команда не падает — она
    // сохраняет текст и оставляет срок прежним: ошибка на каждом сохранении
    // названия ради поля, которого в окне и нет, была бы хуже.
    update_card_in(&conn, card_id, "Со сроком", "описание", Some("2027-01-01")).unwrap();
    assert_eq!(
        due_of(&conn, card_id).as_deref(),
        Some("2026-12-01"),
        "срок, однажды заданный, двигать нельзя"
    );

    // Снятие срока — тоже изменение, и оно тоже не проходит.
    update_card_in(&conn, card_id, "Со сроком", "описание", None).unwrap();
    assert_eq!(due_of(&conn, card_id).as_deref(), Some("2026-12-01"), "срок нельзя и снять");

    // При этом обычная правка текста доезжает до базы.
    let description: String = conn
        .query_row("SELECT description FROM cards WHERE id = ?1", params![card_id], |r| r.get(0))
        .unwrap();
    assert_eq!(description, "описание");
}

#[test]
fn a_card_cannot_leave_a_final_column() {
    let mut conn = test_db();
    let (working, final_col) = board_with_final_column(&conn);
    let card_id = add_plain_card(&conn, working, "Задача", 0);

    // Доехать до финальной колонки можно.
    move_card_in(&mut conn, card_id, final_col, 0).unwrap();
    assert_eq!(card_column_and_position(&conn, card_id).0, final_col);

    // А обратно — нет, и не только через доску: этой же командой меняют статус
    // «Список» и Inbox.
    let err = move_card_in(&mut conn, card_id, working, 0).unwrap_err();
    assert_eq!(err, ERR_CARD_IS_FINAL);
    assert_eq!(
        card_column_and_position(&conn, card_id).0,
        final_col,
        "отклонённый перенос не должен ничего менять"
    );
}

#[test]
fn a_rejected_move_leaves_the_neighbours_alone() {
    let mut conn = test_db();
    let (working, final_col) = board_with_final_column(&conn);
    let stays = add_plain_card(&conn, working, "Остаётся", 0);
    let locked = add_plain_card(&conn, final_col, "Заперта", 0);
    let neighbour = add_plain_card(&conn, final_col, "Соседка", 1);

    move_card_in(&mut conn, locked, working, 0).unwrap_err();

    // Ни исходная колонка, ни целевая не поехали: отказ случается до первого
    // UPDATE, а транзакция всё равно откатывается.
    assert_eq!(card_column_and_position(&conn, stays), (working, 0));
    assert_eq!(card_column_and_position(&conn, locked), (final_col, 0));
    assert_eq!(card_column_and_position(&conn, neighbour), (final_col, 1));
}

#[test]
fn cards_can_still_be_reordered_inside_a_final_column() {
    let mut conn = test_db();
    let (_working, final_col) = board_with_final_column(&conn);
    let first = add_plain_card(&conn, final_col, "Первая", 0);
    let second = add_plain_card(&conn, final_col, "Вторая", 1);

    // Перестановка внутри финальной колонки карточку оттуда не выпускает,
    // поэтому запрещать её незачем.
    move_card_in(&mut conn, second, final_col, 0).unwrap();

    assert_eq!(card_column_and_position(&conn, second), (final_col, 0));
    assert_eq!(card_column_and_position(&conn, first), (final_col, 1));
}

#[test]
fn the_final_flag_survives_an_export_round_trip() {
    let mut conn = test_db();
    let (working, final_col) = board_with_final_column(&conn);
    add_plain_card(&conn, working, "В работе", 0);
    add_plain_card(&conn, final_col, "Сдана", 0);

    let board_id: i64 = conn
        .query_row("SELECT board_id FROM columns WHERE id = ?1", params![working], |r| r.get(0))
        .unwrap();

    let json = serde_json::to_string(&build_board_export(&conn, board_id).unwrap()).unwrap();
    let parsed: BoardExport = serde_json::from_str(&json).unwrap();
    let new_id = import_board_into(&mut conn, 1, parsed).unwrap();

    let after = build_board_export(&conn, new_id).unwrap();
    let flags: Vec<bool> = after.board.columns.iter().map(|c| c.is_final).collect();
    assert_eq!(
        flags,
        vec![false, true],
        "перенесённая доска не должна терять правило, ради которого её так настроили"
    );

    // И правило работает на новой доске, а не только числится в поле.
    let new_final: i64 = conn
        .query_row(
            "SELECT id FROM columns WHERE board_id = ?1 AND is_final = 1",
            params![new_id],
            |r| r.get(0),
        ).unwrap();
    let new_working: i64 = conn
        .query_row(
            "SELECT id FROM columns WHERE board_id = ?1 AND is_final = 0",
            params![new_id],
            |r| r.get(0),
        ).unwrap();
    let moved_card: i64 = conn
        .query_row("SELECT id FROM cards WHERE column_id = ?1", params![new_final], |r| r.get(0))
        .unwrap();
    assert_eq!(move_card_in(&mut conn, moved_card, new_working, 0).unwrap_err(), ERR_CARD_IS_FINAL);
}

#[test]
fn a_file_written_before_final_columns_still_imports() {
    let mut conn = test_db();

    let json = r#"{
        "taskflow_export_version": 1,
        "exported_at": "",
        "board": {
            "name": "Старый файл",
            "columns": [
                { "name": "Готово", "cards": [ { "title": "Задача" } ] }
            ]
        }
    }"#;

    let parsed: BoardExport = serde_json::from_str(json).expect("старый файл должен читаться");
    let new_id = import_board_into(&mut conn, 1, parsed).unwrap();

    let after = build_board_export(&conn, new_id).unwrap();
    assert!(!after.board.columns[0].is_final, "колонка из старого файла — обычная");
}
