// ============================================
// TaskFlow — tests for the assistant's tools
// ============================================
// Инструменты гоняются на базе в памяти с настоящей схемой — теми же
// функциями, что и команды интерфейса. Главное здесь — не «вернули JSON», а
// что модель не может выйти за своё пространство и что мусор в аргументах
// превращается в понятный ей отказ, а не в панику.

use super::*;
use rusqlite::Connection;
use serde_json::{json, Value};

struct Fixture {
    conn: Connection,
    ws: i64,
    other_ws: i64,
    board: i64,
    open_col: i64,
    final_col: i64,
    other_board: i64,
    other_col: i64,
    me: i64,
}

fn board(conn: &Connection, ws: i64, name: &str) -> (i64, i64, i64) {
    conn.execute("INSERT INTO boards (workspace_id, name, gradient) VALUES (?1, ?2, 'g')", params![ws, name]).unwrap();
    let b = conn.last_insert_rowid();
    conn.execute("INSERT INTO columns (board_id, name, position, is_final) VALUES (?1, 'В работе', 0, 0)", params![b]).unwrap();
    let open = conn.last_insert_rowid();
    conn.execute("INSERT INTO columns (board_id, name, position, is_final) VALUES (?1, 'Закрыто', 1, 1)", params![b]).unwrap();
    (b, open, conn.last_insert_rowid())
}

fn card(conn: &Connection, col: i64, title: &str, due: Option<&str>, assignee: Option<i64>) -> i64 {
    conn.execute(
        "INSERT INTO cards (column_id, title, description, position, due_date, assignee_id, priority)
         VALUES (?1, ?2, 'описание', 0, ?3, ?4, 'High')",
        params![col, title, due, assignee],
    ).unwrap();
    conn.last_insert_rowid()
}

fn fixture() -> Fixture {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
    crate::db::create_schema(&conn).unwrap();
    conn.execute("INSERT INTO workspaces (name) VALUES ('Моё')", ()).unwrap();
    let ws = conn.last_insert_rowid();
    conn.execute("INSERT INTO workspaces (name) VALUES ('Чужое')", ()).unwrap();
    let other_ws = conn.last_insert_rowid();
    let (board_id, open_col, final_col) = board(&conn, ws, "Shady's tasks");
    let (other_board, other_col, _) = board(&conn, other_ws, "Секретная");
    let me: i64 = conn.query_row("SELECT id FROM members WHERE is_self = 1", [], |r| r.get(0)).unwrap();
    Fixture { conn, ws, other_ws, board: board_id, open_col, final_col, other_board, other_col, me }
}

fn call(f: &Fixture, name: &str, args: Value) -> Value {
    serde_json::from_str(&run(&f.conn, f.ws, name, &args)).unwrap()
}

fn error_of(v: &Value) -> &str {
    v["error"].as_str().unwrap_or_else(|| panic!("ждали ошибку, пришло {v}"))
}

fn date_offset(conn: &Connection, days: i64) -> String {
    conn.query_row("SELECT date('now', 'localtime', ?1)", params![format!("{days:+} days")], |r| r.get(0)).unwrap()
}

// ─── Описания ───

#[test]
fn every_defined_tool_has_a_handler_and_a_valid_schema() {
    let f = fixture();
    for def in definitions() {
        assert_eq!(def["type"], "function");
        let name = def["function"]["name"].as_str().unwrap();
        assert!(!def["function"]["description"].as_str().unwrap().is_empty());
        assert_eq!(def["function"]["parameters"]["type"], "object");
        if let ToolStep::Result(out) = step(&f.conn, f.ws, name, &json!({})) {
            assert!(!out.contains("Инструмента «"), "{name} не подключён: {out}");
        }
        assert_eq!(is_write_tool(name), WRITE_TOOLS.contains(&name));
    }
    assert_eq!(definitions().len(), 18);
    // Каждое изменение описано в схеме, иначе модель о нём не узнает.
    for w in WRITE_TOOLS {
        assert!(definitions().iter().any(|d| d["function"]["name"] == w), "{w}");
    }
}

#[test]
fn an_unknown_tool_lists_the_real_ones() {
    let f = fixture();
    let v = call(&f, "delete_everything", json!({}));
    assert!(error_of(&v).contains("get_boards"));
}

// ─── Аргументы ───

#[test]
fn ids_are_accepted_as_numbers_and_digit_strings_only() {
    assert_eq!(arg_id(&json!({"card_id": 7}), "card_id").unwrap(), 7);
    assert_eq!(arg_id(&json!({"card_id": " 7 "}), "card_id").unwrap(), 7);
    for bad in [json!({}), json!({"card_id": null}), json!({"card_id": "семь"}), json!({"card_id": 7.5}),
                json!({"card_id": -1}), json!({"card_id": 0}), json!({"card_id": true}), json!(null)] {
        assert!(arg_id(&bad, "card_id").is_err(), "{bad}");
    }
}

#[test]
fn dates_must_be_real_calendar_dates() {
    assert_eq!(arg_date(&json!({"from": "2026-10-05"}), "from").unwrap(), "2026-10-05");
    for bad in ["5 октября", "2026-02-30", "2026-10-5", "", "2026/10/05"] {
        assert!(arg_date(&json!({"from": bad}), "from").is_err(), "{bad}");
    }
}

// ─── Пространство ───

#[test]
fn boards_are_counted_with_open_and_done_cards() {
    let f = fixture();
    card(&f.conn, f.open_col, "Отчёт", None, None);
    card(&f.conn, f.open_col, "Созвон", None, None);
    card(&f.conn, f.final_col, "Готово", None, None);
    let v = call(&f, "get_boards", json!({}));
    assert_eq!(v["count"], 1, "чужое пространство не видно");
    assert_eq!(v["boards"][0]["name"], "Shady's tasks");
    assert_eq!(v["boards"][0]["open_cards"], 2);
    assert_eq!(v["boards"][0]["done_cards"], 1);
}

#[test]
fn another_workspace_is_out_of_reach_by_any_id() {
    let f = fixture();
    let secret = card(&f.conn, f.other_col, "Секрет", Some("2020-01-01"), None);

    assert!(error_of(&call(&f, "get_board_columns", json!({"board_id": f.other_board}))).contains("get_boards"));
    assert!(error_of(&call(&f, "get_cards_in_column", json!({"column_id": f.other_col}))).contains("get_board_columns"));
    assert!(error_of(&call(&f, "get_card_details", json!({"card_id": secret}))).contains("search_cards"));
    assert!(call(&f, "get_time_summary", json!({"board_id": f.other_board}))["error"].is_string());
    assert!(call(&f, "get_time_summary", json!({"card_id": secret}))["error"].is_string());
    assert_eq!(call(&f, "search_cards", json!({"query": "Секрет"}))["count"], 0);
    assert_eq!(call(&f, "get_overdue_cards", json!({}))["count"], 0);
    // И обратное: из чужого пространства своя карточка видна.
    let v: Value = serde_json::from_str(&run(&f.conn, f.other_ws, "search_cards", &json!({"query": "Секрет"}))).unwrap();
    assert_eq!(v["count"], 1);
}

#[test]
fn made_up_ids_are_refused_with_a_hint() {
    let f = fixture();
    let v = call(&f, "get_card_details", json!({"card_id": 99999}));
    assert!(error_of(&v).contains("99999"));
    let v = call(&f, "get_board_columns", json!({"board_id": "abc"}));
    assert!(error_of(&v).contains("board_id"));
}

#[test]
fn archived_boards_are_invisible() {
    let f = fixture();
    f.conn.execute("UPDATE boards SET archived = 1 WHERE id = ?1", params![f.board]).unwrap();
    assert_eq!(call(&f, "get_boards", json!({}))["count"], 0);
    assert!(call(&f, "get_board_columns", json!({"board_id": f.board}))["error"].is_string());
}

// ─── Чтение ───

#[test]
fn columns_come_in_order_with_counts_and_the_final_flag() {
    let f = fixture();
    card(&f.conn, f.open_col, "Отчёт", None, None);
    let v = call(&f, "get_board_columns", json!({"board_id": f.board}));
    assert_eq!(v["board"], "Shady's tasks");
    assert_eq!(v["columns"][0], json!({"id": f.open_col, "name": "В работе", "is_final": false, "cards": 1}));
    assert_eq!(v["columns"][1]["is_final"], true);
}

#[test]
fn cards_in_a_column_name_their_assignee() {
    let f = fixture();
    card(&f.conn, f.open_col, "Отчёт", Some("2026-10-10"), Some(f.me));
    let v = call(&f, "get_cards_in_column", json!({"column_id": f.open_col}));
    assert_eq!(v["count"], 1);
    assert_eq!(v["cards"][0]["title"], "Отчёт");
    assert_eq!(v["cards"][0]["due_date"], "2026-10-10");
    assert_eq!(v["cards"][0]["priority"], "High");
    assert!(v["cards"][0]["assignee"].is_string());
}

#[test]
fn search_ignores_case_and_finds_parts_of_titles() {
    let f = fixture();
    card(&f.conn, f.open_col, "Подготовить ОТЧЁТ", None, None);
    card(&f.conn, f.open_col, "Созвон", None, None);
    let v = call(&f, "search_cards", json!({"query": "отчёт"}));
    assert_eq!(v["count"], 1);
    assert_eq!(v["cards"][0]["title"], "Подготовить ОТЧЁТ");
    assert!(call(&f, "search_cards", json!({"query": "  "}))["error"].is_string());
}

#[test]
fn card_details_gather_checklist_time_and_dependencies() {
    let f = fixture();
    let c = card(&f.conn, f.open_col, "Отчёт", Some("2026-10-10"), Some(f.me));
    let blocker = card(&f.conn, f.open_col, "Данные", None, None);
    f.conn.execute("INSERT INTO checklist_items (card_id, text, is_done, position) VALUES (?1, 'Сбор', 1, 0)", params![c]).unwrap();
    f.conn.execute("INSERT INTO checklist_items (card_id, text, is_done, position) VALUES (?1, 'Текст', 0, 1)", params![c]).unwrap();
    f.conn.execute(
        "INSERT INTO time_entries (card_id, member_id, started_at, ended_at, duration_seconds)
         VALUES (?1, ?2, datetime('now', '-2 hours'), datetime('now'), 7500)",
        params![c, f.me],
    ).unwrap();
    f.conn.execute("INSERT INTO card_dependencies (blocker_card_id, blocked_card_id) VALUES (?1, ?2)", params![blocker, c]).unwrap();

    let v = call(&f, "get_card_details", json!({"card_id": c}));
    assert_eq!(v["title"], "Отчёт");
    assert_eq!(v["description"], "описание");
    let items = v["checklist"].as_array().unwrap();
    assert_eq!(items.iter().map(|i| (i["text"].as_str().unwrap(), i["done"].as_bool().unwrap())).collect::<Vec<_>>(), [("Сбор", true), ("Текст", false)]);
    assert!(items.iter().all(|i| i["id"].is_i64()), "id нужен, чтобы отметить пункт");
    assert_eq!(v["time_spent"], "2 ч 05 мин");
    assert_eq!(v["blocked_by"][0]["title"], "Данные");
    assert_eq!(v["done"], false);
}

#[test]
fn overdue_means_past_due_and_not_in_the_final_column() {
    let f = fixture();
    let yesterday = date_offset(&f.conn, -1);
    let today = date_offset(&f.conn, 0);
    card(&f.conn, f.open_col, "Просрочена", Some(&yesterday), None);
    card(&f.conn, f.final_col, "Сделана поздно", Some(&yesterday), None);
    card(&f.conn, f.open_col, "Сегодня", Some(&today), None);
    card(&f.conn, f.open_col, "Без срока", None, None);
    let v = call(&f, "get_overdue_cards", json!({}));
    assert_eq!(v["count"], 1);
    assert_eq!(v["cards"][0]["title"], "Просрочена");
    assert_eq!(v["today"], today);
}

#[test]
fn deadline_range_is_inclusive_sorted_and_split_into_open_and_done() {
    let f = fixture();
    card(&f.conn, f.open_col, "Пятое", Some("2026-10-05"), None);
    card(&f.conn, f.open_col, "Первое", Some("2026-10-01"), None);
    card(&f.conn, f.final_col, "Третье готово", Some("2026-10-03"), None);
    card(&f.conn, f.open_col, "Шестое", Some("2026-10-06"), None);
    let v = call(&f, "get_cards_by_deadline_range", json!({"from": "2026-10-01", "to": "2026-10-05"}));
    assert_eq!(v["count"], 3);
    assert_eq!(v["open_count"], 2);
    assert_eq!(v["done_count"], 1);
    let titles: Vec<&str> = v["cards"].as_array().unwrap().iter().map(|c| c["title"].as_str().unwrap()).collect();
    assert_eq!(titles, ["Первое", "Третье готово", "Пятое"]);

    assert!(call(&f, "get_cards_by_deadline_range", json!({"from": "2026-10-05", "to": "2026-10-01"}))["error"].is_string());
    assert!(call(&f, "get_cards_by_deadline_range", json!({"from": "5 октября", "to": "2026-10-10"}))["error"].is_string());
}

#[test]
fn time_is_summed_per_card_or_per_board_but_not_both() {
    let f = fixture();
    let a = card(&f.conn, f.open_col, "А", None, None);
    let b = card(&f.conn, f.final_col, "Б", None, None);
    for (c, secs) in [(a, 3600), (b, 1800)] {
        f.conn.execute(
            "INSERT INTO time_entries (card_id, member_id, started_at, ended_at, duration_seconds)
             VALUES (?1, ?2, datetime('now', '-3 hours'), datetime('now', '-2 hours'), ?3)",
            params![c, f.me, secs],
        ).unwrap();
    }
    assert_eq!(call(&f, "get_time_summary", json!({"card_id": a}))["seconds"], 3600);
    let v = call(&f, "get_time_summary", json!({"board_id": f.board}));
    assert_eq!(v["seconds"], 5400);
    assert_eq!(v["time_spent"], "1 ч 30 мин");
    assert!(call(&f, "get_time_summary", json!({}))["error"].is_string());
    assert!(call(&f, "get_time_summary", json!({"card_id": a, "board_id": f.board}))["error"].is_string());
}

#[test]
fn workload_lists_members_with_open_tasks() {
    let f = fixture();
    card(&f.conn, f.open_col, "Отчёт", None, Some(f.me));
    card(&f.conn, f.final_col, "Готово", None, Some(f.me));
    let v = call(&f, "get_workload", json!({}));
    let me = v["members"].as_array().unwrap().iter().find(|m| m["is_me"] == true).unwrap();
    assert_eq!(me["open_tasks"], 1);
    assert_eq!(me["high"], 1);
}

#[test]
fn long_lists_are_cut_but_counted() {
    let f = fixture();
    for i in 0..(MAX_CARDS + 5) {
        card(&f.conn, f.open_col, &format!("Задача {i}"), None, None);
    }
    let v = call(&f, "search_cards", json!({"query": "задача"}));
    assert_eq!(v["count"], MAX_CARDS + 5);
    assert_eq!(v["cards"].as_array().unwrap().len(), MAX_CARDS);
    assert!(v["note"].is_string());
}

#[test]
fn durations_read_like_a_person_would_say_them() {
    assert_eq!(format_duration(0), "0 мин");
    assert_eq!(format_duration(59), "0 мин");
    assert_eq!(format_duration(45 * 60), "45 мин");
    assert_eq!(format_duration(3600 + 5 * 60), "1 ч 05 мин");
    assert_eq!(format_duration(-10), "0 мин");
}

// ─── Изменения: проверка до «Да» ───

fn prepared(f: &Fixture, name: &str, args: Value) -> Prepared {
    match step(&f.conn, f.ws, name, &args) {
        ToolStep::Confirm(p) => p,
        ToolStep::Result(r) => panic!("ждали превью {name}, пришло {r}"),
    }
}

fn refused(f: &Fixture, name: &str, args: Value) -> String {
    match step(&f.conn, f.ws, name, &args) {
        ToolStep::Result(r) => {
            let v: Value = serde_json::from_str(&r).unwrap();
            v["error"].as_str().unwrap_or_else(|| panic!("ждали отказ, пришло {r}")).to_string()
        }
        ToolStep::Confirm(p) => panic!("ждали отказ {name}, пришло превью «{}»", p.description),
    }
}

fn count(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |r| r.get(0)).unwrap()
}

#[test]
fn reading_tools_run_at_once_and_changes_wait() {
    let f = fixture();
    assert!(matches!(step(&f.conn, f.ws, "get_boards", &json!({})), ToolStep::Result(_)));
    let before = count(&f.conn, "SELECT COUNT(*) FROM cards");
    prepared(&f, "create_card", json!({"board_id": f.board, "title": "Отчёт"}));
    assert_eq!(count(&f.conn, "SELECT COUNT(*) FROM cards"), before, "превью ничего не пишет");
}

#[test]
fn create_card_preview_names_everything_and_uses_defaults() {
    let f = fixture();
    let due = date_offset(&f.conn, 14);
    let p = prepared(&f, "create_card", json!({
        "board_id": f.board, "title": " Подготовить отчёт ", "due_date": due, "priority": "высокий",
    }));
    assert_eq!(p.description, format!(
        "Создать задачу «Подготовить отчёт» на доске «Shady's tasks» в колонке «В работе»: срок {}, приоритет высокий, исполнитель — Пользователь (вы)",
        date_label(&due)));
    assert_eq!(p.args["column_id"], f.open_col, "первая рабочая колонка, не финальная");
    assert_eq!(p.args["priority"], "High");
    assert_eq!(p.args["assignee_id"], f.me, "без исполнителя — сам пользователь");
    assert_eq!(p.warnings.len(), 1, "про срок, который потом не поправить");

    let p = prepared(&f, "create_card", json!({"board_id": f.board, "title": "Без всего"}));
    assert!(p.description.contains("без срока, приоритет средний"), "{}", p.description);
    assert!(p.warnings.is_empty());
}

#[test]
fn create_card_refuses_bad_input() {
    let f = fixture();
    let yesterday = date_offset(&f.conn, -1);
    assert!(refused(&f, "create_card", json!({"board_id": f.board, "title": "Х", "due_date": yesterday})).contains("уже прошёл"));
    assert!(refused(&f, "create_card", json!({"board_id": f.board, "title": "Х", "due_date": "10 октября"})).contains("ГГГГ-ММ-ДД"));
    assert!(refused(&f, "create_card", json!({"board_id": f.board, "title": "Х", "priority": "срочно"})).contains("неизвестен"));
    assert!(refused(&f, "create_card", json!({"board_id": f.board, "title": "   "})).contains("title"));
    assert!(refused(&f, "create_card", json!({"board_id": f.other_board, "title": "Х"})).contains("get_boards"));
    assert!(refused(&f, "create_card", json!({"board_id": f.board, "title": "Х", "assignee_id": 9999})).contains("get_workload"));
    assert!(refused(&f, "create_card", json!({"board_id": f.board, "title": "Х".repeat(201)})).contains("200"));
}

#[test]
fn a_board_with_only_a_final_column_takes_no_new_cards() {
    let f = fixture();
    f.conn.execute("UPDATE columns SET archived = 1 WHERE id = ?1", params![f.open_col]).unwrap();
    assert!(refused(&f, "create_card", json!({"board_id": f.board, "title": "Х"})).contains("рабочей колонки"));
}

#[test]
fn confirmed_create_card_lands_with_every_field() {
    let mut f = fixture();
    let due = date_offset(&f.conn, 14);
    let p = prepared(&f, "create_card", json!({
        "board_id": f.board, "title": "Подготовить отчёт", "due_date": due, "priority": "High", "description": "к пятнице",
    }));
    let result = execute(&mut f.conn, f.ws, &p).unwrap();
    let id = result["card_id"].as_i64().unwrap();
    let (col, title, desc, d, prio, who, author): (i64, String, String, Option<String>, String, Option<i64>, Option<i64>) = f.conn.query_row(
        "SELECT column_id, title, description, due_date, priority, assignee_id, author_id FROM cards WHERE id = ?1",
        params![id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)),
    ).unwrap();
    assert_eq!((col, title.as_str(), desc.as_str(), d.as_deref(), prio.as_str()), (f.open_col, "Подготовить отчёт", "к пятнице", Some(due.as_str()), "High"));
    assert_eq!((who, author), (Some(f.me), Some(f.me)));
}

#[test]
fn nothing_happens_if_the_data_changed_while_waiting() {
    let mut f = fixture();
    let c = card(&f.conn, f.open_col, "Отчёт", None, None);
    let p = prepared(&f, "update_card_priority", json!({"card_id": c, "priority": "Low"}));
    // Пока человек думал, карточку убрали в архив.
    f.conn.execute("UPDATE cards SET archived = 1 WHERE id = ?1", params![c]).unwrap();
    assert!(execute(&mut f.conn, f.ws, &p).is_err());
    let prio: String = f.conn.query_row("SELECT priority FROM cards WHERE id = ?1", params![c], |r| r.get(0)).unwrap();
    assert_eq!(prio, "High");
}

#[test]
fn a_changed_description_is_not_executed() {
    let mut f = fixture();
    let c = card(&f.conn, f.open_col, "Отчёт", None, None);
    let p = prepared(&f, "move_card", json!({"card_id": c, "column_id": f.final_col}));
    // Карточку переименовали — превью описывало уже не её.
    f.conn.execute("UPDATE cards SET title = 'Другое' WHERE id = ?1", params![c]).unwrap();
    let err = execute(&mut f.conn, f.ws, &p).unwrap_err();
    assert!(err.contains("данные изменились"), "{err}");
    let col: i64 = f.conn.query_row("SELECT column_id FROM cards WHERE id = ?1", params![c], |r| r.get(0)).unwrap();
    assert_eq!(col, f.open_col);
}

#[test]
fn moving_into_the_final_column_warns_and_out_of_it_is_refused() {
    let mut f = fixture();
    let c = card(&f.conn, f.open_col, "Отчёт", None, None);
    let blocker = card(&f.conn, f.open_col, "Данные", None, None);
    f.conn.execute("INSERT INTO card_dependencies (blocker_card_id, blocked_card_id) VALUES (?1, ?2)", params![blocker, c]).unwrap();

    let p = prepared(&f, "move_card", json!({"card_id": c, "column_id": f.final_col}));
    assert_eq!(p.description, "Перенести задачу «Отчёт» из «В работе» в «Закрыто»");
    assert_eq!(p.warnings.len(), 2, "{:?}", p.warnings);
    assert!(p.warnings[1].contains("«Данные»"));
    execute(&mut f.conn, f.ws, &p).unwrap();

    assert!(refused(&f, "move_card", json!({"card_id": c, "column_id": f.open_col})).contains("финальной"));
    assert!(refused(&f, "move_card", json!({"card_id": blocker, "column_id": f.open_col})).contains("уже в колонке"));
    assert!(refused(&f, "move_card", json!({"card_id": blocker, "column_id": f.other_col})).contains("get_board_columns"));
}

#[test]
fn moving_between_boards_is_refused() {
    let f = fixture();
    let (_, second_open, _) = board(&f.conn, f.ws, "Вторая");
    let c = card(&f.conn, f.open_col, "Отчёт", None, None);
    assert!(refused(&f, "move_card", json!({"card_id": c, "column_id": second_open})).contains("другой доске"));
}

#[test]
fn priority_and_assignee_changes_describe_before_and_after() {
    let mut f = fixture();
    let c = card(&f.conn, f.open_col, "Отчёт", None, None);
    let p = prepared(&f, "update_card_priority", json!({"card_id": c, "priority": "low"}));
    assert_eq!(p.description, "Сменить приоритет задачи «Отчёт»: высокий → низкий");
    execute(&mut f.conn, f.ws, &p).unwrap();
    assert!(refused(&f, "update_card_priority", json!({"card_id": c, "priority": "Low"})).contains("уже"));

    let colleague: i64 = {
        f.conn.execute("INSERT INTO members (name) VALUES ('Коллега')", ()).unwrap();
        f.conn.last_insert_rowid()
    };
    let p = prepared(&f, "update_card_assignee", json!({"card_id": c, "member_id": colleague}));
    assert_eq!(p.description, "Назначить исполнителем задачи «Отчёт»: Коллега вместо никого");
    execute(&mut f.conn, f.ws, &p).unwrap();
    let who: i64 = f.conn.query_row("SELECT assignee_id FROM cards WHERE id = ?1", params![c], |r| r.get(0)).unwrap();
    assert_eq!(who, colleague);
}

#[test]
fn checklist_items_are_added_and_toggled_only_in_this_workspace() {
    let mut f = fixture();
    let c = card(&f.conn, f.open_col, "Отчёт", None, None);
    let p = prepared(&f, "add_checklist_item", json!({"card_id": c, "text": "Собрать данные"}));
    let item = execute(&mut f.conn, f.ws, &p).unwrap()["item_id"].as_i64().unwrap();

    let p = prepared(&f, "toggle_checklist_item", json!({"item_id": item}));
    assert_eq!(p.description, "Отметить выполненным пункт «Собрать данные» в задаче «Отчёт»");
    assert_eq!(execute(&mut f.conn, f.ws, &p).unwrap()["done"], true);
    let p = prepared(&f, "toggle_checklist_item", json!({"item_id": item}));
    assert!(p.description.starts_with("Снять отметку"));

    let secret = card(&f.conn, f.other_col, "Секрет", None, None);
    f.conn.execute("INSERT INTO checklist_items (card_id, text, position) VALUES (?1, 'чужой', 0)", params![secret]).unwrap();
    let foreign = f.conn.last_insert_rowid();
    assert!(refused(&f, "toggle_checklist_item", json!({"item_id": foreign})).contains("этом пространстве"));
}

#[test]
fn timers_start_and_stop_for_the_user_by_default() {
    let mut f = fixture();
    let a = card(&f.conn, f.open_col, "А", None, None);
    let b = card(&f.conn, f.open_col, "Б", None, None);
    assert!(refused(&f, "stop_timer", json!({})).contains("не идёт"));

    let p = prepared(&f, "start_timer", json!({"card_id": a}));
    assert_eq!(p.description, "Запустить таймер на задаче «А»");
    execute(&mut f.conn, f.ws, &p).unwrap();
    assert!(refused(&f, "start_timer", json!({"card_id": a})).contains("уже идёт"));

    let p = prepared(&f, "start_timer", json!({"card_id": b}));
    assert_eq!(p.warnings, vec!["Идущий таймер на «А» остановится.".to_string()]);
    execute(&mut f.conn, f.ws, &p).unwrap();

    let p = prepared(&f, "stop_timer", json!({}));
    assert_eq!(p.description, "Остановить таймер на задаче «Б»");
    execute(&mut f.conn, f.ws, &p).unwrap();
    assert_eq!(count(&f.conn, "SELECT COUNT(*) FROM time_entries WHERE ended_at IS NULL"), 0);
    assert_eq!(count(&f.conn, "SELECT COUNT(*) FROM time_entries"), 2);
}


// ─── Сводка продуктивности ───

#[test]
fn the_report_splits_period_events_from_the_current_state() {
    let f = fixture();
    let (second, second_open, _) = board(&f.conn, f.ws, "Учёба");
    let yesterday = date_offset(&f.conn, -1);

    // Созданы в периоде: три открытые + одна завершённая сегодня.
    let a = card(&f.conn, f.open_col, "А", Some(&yesterday), None);        // просрочена
    card(&f.conn, f.open_col, "Б", None, None);
    card(&f.conn, second_open, "В", None, None);
    let done = card(&f.conn, f.final_col, "Готово", None, None);
    f.conn.execute("UPDATE cards SET completed_at = datetime('now', '-1 day') WHERE id = ?1", params![done]).unwrap();
    // Создана и закрыта давно — не в периоде.
    let old = card(&f.conn, f.final_col, "Старое", None, None);
    f.conn.execute("UPDATE cards SET created_at = datetime('now', '-60 days'), completed_at = datetime('now', '-50 days') WHERE id = ?1", params![old]).unwrap();
    // Закрыта до появления даты завершения.
    card(&f.conn, f.final_col, "Без даты", None, None);
    // «Требует внимания», попытка и время.
    f.conn.execute("UPDATE cards SET is_mistake = 1 WHERE id = ?1", params![a]).unwrap();
    f.conn.execute("INSERT INTO card_retries (card_id) VALUES (?1)", params![a]).unwrap();
    f.conn.execute("INSERT INTO card_retries (card_id, requested_at) VALUES (?1, datetime('now', '-20 days'))", params![a]).unwrap();
    f.conn.execute(
        "INSERT INTO time_entries (card_id, member_id, started_at, ended_at, duration_seconds)
         VALUES (?1, ?2, datetime('now', '-3 hours'), datetime('now', '-1 hours'), 7200)",
        params![a, f.me],
    ).unwrap();
    // Чужое пространство не считается.
    card(&f.conn, f.other_col, "Чужая", Some(&yesterday), None);

    let v = call(&f, "get_productivity_report", json!({"period_days": 7}));
    let p = &v["during_period"];
    assert_eq!(p["created"], 5, "{v}"); // А, Б, В, Готово, Без даты
    assert_eq!(p["completed"], 1);
    assert_eq!(p["retries_requested"], 1);
    assert_eq!(p["time_tracked"], "2 ч 00 мин");
    let now = &v["right_now"];
    assert_eq!(now["active"], 3);
    assert_eq!(now["overdue"], 1);
    assert_eq!(now["needs_attention"], 1);
    assert_eq!(v["top_boards_by_active_cards"], json!([
        {"board": "Shady's tasks", "active_cards": 2},
        {"board": "Учёба", "active_cards": 1},
    ]));
    assert!(v["note"].as_str().unwrap().contains("Ещё 1 завершённых"), "{v}");
    let _ = second;

    // За 30 дней попадает и вторая попытка.
    let v = call(&f, "get_productivity_report", json!({"period_days": 30}));
    assert_eq!(v["during_period"]["retries_requested"], 2);
}

#[test]
fn the_report_period_is_bounded() {
    let f = fixture();
    for bad in [json!({}), json!({"period_days": 0}), json!({"period_days": 366}), json!({"period_days": "неделя"})] {
        assert!(call(&f, "get_productivity_report", bad.clone())["error"].is_string(), "{bad}");
    }
    assert!(call(&f, "get_productivity_report", json!({"period_days": "30"}))["error"].is_null());
}

#[test]
fn an_empty_workspace_gets_zeros_not_errors() {
    let f = fixture();
    let v = call(&f, "get_productivity_report", json!({"period_days": 7}));
    assert_eq!(v["during_period"]["created"], 0);
    assert_eq!(v["right_now"]["active"], 0);
    assert_eq!(v["top_boards_by_active_cards"], json!([]));
    assert!(v.get("note").is_none());
}


#[test]
fn needs_attention_matches_the_attention_section() {
    let f = fixture();
    let a = card(&f.conn, f.open_col, "А", None, None);
    // Колонка ушла в архив, а карточка осталась помеченной — раздел
    // «Требуют внимания» её показывает, значит и сводка считает.
    f.conn.execute("INSERT INTO columns (board_id, name, position, archived) VALUES (?1, 'Старая', 5, 1)", params![f.board]).unwrap();
    let old_col = f.conn.last_insert_rowid();
    let b = card(&f.conn, old_col, "Б", None, None);
    f.conn.execute("UPDATE cards SET is_mistake = 1 WHERE id IN (?1, ?2)", params![a, b]).unwrap();
    let section = mistake_cards_in(&f.conn, f.ws).unwrap().len();
    assert_eq!(section, 2);
    let v = call(&f, "get_productivity_report", json!({"period_days": 7}));
    assert_eq!(v["right_now"]["needs_attention"], section);
}


#[test]
fn closing_a_recurring_card_through_the_assistant_warns_and_reports_the_next_one() {
    let mut f = fixture();
    let due = date_offset(&f.conn, 2);
    let c = card(&f.conn, f.open_col, "Недельный отчёт", Some(&due), None);
    f.conn.execute("UPDATE cards SET recurrence_rule = 'weekly' WHERE id = ?1", params![c]).unwrap();

    let details = call(&f, "get_card_details", json!({"card_id": c}));
    assert_eq!(details["recurrence"], "weekly");

    let p = prepared(&f, "move_card", json!({"card_id": c, "column_id": f.final_col}));
    let next = date_label(&date_offset(&f.conn, 9));
    assert!(p.warnings.iter().any(|w| w.contains("повторяется") && w.contains(&next)), "{:?}", p.warnings);

    let result = execute(&mut f.conn, f.ws, &p).unwrap();
    let spawned = &result["next_recurring_card"];
    assert_eq!(spawned["title"], "Недельный отчёт");
    assert_eq!(spawned["due_date"], date_offset(&f.conn, 9));
    assert_eq!(spawned["column"], "В работе");
}

// ─── Живая проверка с настоящей моделью ───

fn live_settings() -> AiSettings {
    AiSettings {
        ollama_url: crate::ollama::DEFAULT_OLLAMA_URL.into(),
        model: std::env::var("TASKFLOW_AI_MODEL").unwrap_or_else(|_| "qwen2.5:7b-instruct-q4_K_M".into()),
        context_length: 20,
        timeout_seconds: 300,
    }
}

/// Вопросы из «Проверки» Фазы 2 — настоящей модели через тот же `advance`,
/// что у `ollama_chat`, но на базе в памяти.
///
/// `cargo test --lib ai_tools_live -- --ignored --nocapture --test-threads=1`
/// Модель — из `TASKFLOW_AI_MODEL`, по умолчанию qwen2.5:7b-instruct-q4_K_M.
#[test]
#[ignore]
fn ai_tools_live_questions_from_the_phase_check() {
    let f = fixture();
    board(&f.conn, f.ws, "Учёба");
    let yesterday = date_offset(&f.conn, -1);
    card(&f.conn, f.open_col, "Сдать отчёт по проекту", Some(&yesterday), Some(f.me));
    card(&f.conn, f.open_col, "Позвонить подрядчику", None, None);
    card(&f.conn, f.final_col, "Старая готовая задача", Some("2020-01-01"), None);
    let settings = live_settings();
    let system = system_prompt(&f.conn, f.ws).unwrap();

    for (question, expected_tool) in [
        ("Сколько у меня досок?", "get_boards"),
        ("Какие задачи просрочены?", "get_overdue_cards"),
    ] {
        let mut turn = Turn::new(question.into(), build_chat_request(&system, &[], question));
        let started = std::time::Instant::now();
        let answer = match tauri::async_runtime::block_on(advance(&settings, &mut turn, |name, args| {
            println!("    → {name}({args})");
            Ok(step(&f.conn, f.ws, name, args))
        })).unwrap() {
            TurnStep::Answer(a) => a,
            TurnStep::Confirm(p) => panic!("на вопрос о данных модель предложила изменение: {}", p.description),
        };
        println!("ВОПРОС: {question}\nИНСТРУМЕНТЫ: {:?}\nОТВЕТ ({:.1} с): {answer}\n", turn.tools_used, started.elapsed().as_secs_f32());
        assert!(turn.tools_used.iter().any(|t| t == expected_tool), "ждали {expected_tool}, вызваны {:?}", turn.tools_used);
    }
}

/// «Проверка» Фазы 3: модель должна предложить создание с правильными полями,
/// а после «Да» карточка — появиться на доске.
///
/// `cargo test --lib ai_tools_live_create -- --ignored --nocapture`
#[test]
#[ignore]
fn ai_tools_live_create_card_from_the_phase_check() {
    let mut f = fixture();
    let settings = live_settings();
    let system = system_prompt(&f.conn, f.ws).unwrap();
    let question = "Создай задачу 'Подготовить отчёт' на доске Shady's tasks с дедлайном 10 октября, приоритет высокий";
    let mut turn = Turn::new(question.into(), build_chat_request(&system, &[], question));
    let started = std::time::Instant::now();

    let mut confirmations = 0;
    let answer = loop {
        let step_result = {
            let conn = &f.conn;
            tauri::async_runtime::block_on(advance(&settings, &mut turn, |name, args| {
                println!("    → {name}({args})");
                Ok(step(conn, f.ws, name, args))
            })).unwrap()
        };
        match step_result {
            TurnStep::Answer(a) => break a,
            TurnStep::Confirm(p) => {
                confirmations += 1;
                println!("ПРЕВЬЮ: {}\n  предупреждения: {:?}", p.description, p.warnings);
                assert_eq!(p.tool, "create_card", "ждали create_card");
                let outcome = execute(&mut f.conn, f.ws, &p);
                resolve_action(&mut turn, &p, outcome, true);
            }
        }
    };
    println!("ОТВЕТ ({:.1} с): {answer}\nДЕЙСТВИЯ: {:?}", started.elapsed().as_secs_f32(), turn.actions);
    assert_eq!(confirmations, 1);

    let (title, due, prio, col): (String, Option<String>, String, i64) = f.conn.query_row(
        "SELECT title, due_date, priority, column_id FROM cards ORDER BY id DESC LIMIT 1", [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    ).unwrap();
    println!("В БАЗЕ: {title} | {due:?} | {prio} | колонка {col}");
    assert_eq!(title, "Подготовить отчёт");
    assert_eq!(due.as_deref().map(|d| &d[5..]), Some("10-10"));
    assert_eq!(prio, "High");
    assert_eq!(col, f.open_col);
}

// ─── Подтверждение: что уходит модели и что остаётся в истории ───

fn sample_prepared() -> Prepared {
    Prepared { tool: "update_card_priority".into(), args: json!({}), description: "Сменить приоритет".into(), warnings: vec![] }
}

#[test]
fn a_cancelled_action_tells_the_model_and_is_logged() {
    let mut turn = Turn::new("q".into(), vec![]);
    resolve_action(&mut turn, &sample_prepared(), Ok(Value::Null), false);
    let last = turn.messages.last().unwrap();
    assert_eq!(last.role, "tool");
    assert!(last.content.contains("Пользователь отменил действие"));
    assert_eq!(turn.actions[0].status, "cancelled");
    assert!(!turn.executed_anything());
}

#[test]
fn done_and_failed_actions_are_told_apart() {
    let mut turn = Turn::new("q".into(), vec![]);
    resolve_action(&mut turn, &sample_prepared(), Err("данные изменились".into()), true);
    assert_eq!(turn.actions[0].status, "failed");
    assert_eq!(turn.actions[0].error.as_deref(), Some("данные изменились"));
    assert!(!turn.executed_anything());
    resolve_action(&mut turn, &sample_prepared(), Ok(json!({"ok": true})), true);
    assert_eq!(turn.actions[1].status, "done");
    assert!(turn.executed_anything());
    assert_eq!(turn.tools_used, ["update_card_priority", "update_card_priority"]);
}

#[test]
fn actions_are_kept_with_the_answer_in_history() {
    let mut f = fixture();
    let log = vec![AiActionLog { description: "Создать задачу «Отчёт»".into(), status: "done".into(), error: None }];
    let saved = save_chat_exchange(&mut f.conn, f.ws, "Создай", "Готово", &["create_card".to_string()], &log).unwrap();
    assert!(saved[0].actions.is_empty());
    assert_eq!(saved[1].actions, log);
    assert_eq!(read_chat_history(&f.conn, f.ws, None).unwrap()[1].actions, log);
}

/// «Проверка» Фазы 4: приветствие по сводке с реальными числами и вопрос о
/// продуктивности за месяц — модель должна вызвать отчёт за 30 дней.
///
/// `cargo test --lib ai_tools_live_report -- --ignored --nocapture`
#[test]
#[ignore]
fn ai_tools_live_report_greeting_and_month_question() {
    let f = fixture();
    let yesterday = date_offset(&f.conn, -1);
    let a = card(&f.conn, f.open_col, "Сдать отчёт по проекту", Some(&yesterday), Some(f.me));
    card(&f.conn, f.open_col, "Позвонить подрядчику", Some(&yesterday), None);
    card(&f.conn, f.open_col, "Обновить сайт", None, None);
    let done = card(&f.conn, f.final_col, "Выпустить релиз", None, None);
    f.conn.execute("UPDATE cards SET completed_at = datetime('now', '-2 days') WHERE id = ?1", params![done]).unwrap();
    f.conn.execute("UPDATE cards SET is_mistake = 1 WHERE id = ?1", params![a]).unwrap();
    let settings = live_settings();
    let system = system_prompt(&f.conn, f.ws).unwrap();

    let report = productivity_report(&f.conn, f.ws, 7).unwrap();
    println!("СВОДКА: {report}");
    let started = std::time::Instant::now();
    let greeting = tauri::async_runtime::block_on(async {
        let (settings, request) = (settings.clone(), greeting_request(&system, &report));
        run_blocking(move || crate::ollama::chat(&settings.ollama_url, &settings.model, &request, &[], settings.timeout_seconds)).await
    }).unwrap().content;
    println!("ПРИВЕТСТВИЕ ({:.1} с):\n{greeting}\n", started.elapsed().as_secs_f32());
    assert!(greeting.contains('2'), "в приветствии должны быть реальные числа (2 просроченные)");

    let question = "Как у меня дела с продуктивностью за месяц?";
    let greeting_msg = ChatMessage {
        id: 1, workspace_id: f.ws, role: "assistant".into(), content: greeting, created_at: String::new(),
        tools_used: vec![], actions: vec![],
    };
    let mut turn = Turn::new(question.into(), build_chat_request(&system, &[greeting_msg], question));
    let started = std::time::Instant::now();
    let answer = match tauri::async_runtime::block_on(advance(&settings, &mut turn, |name, args| {
        println!("    → {name}({args})");
        Ok(step(&f.conn, f.ws, name, args))
    })).unwrap() {
        TurnStep::Answer(a) => a,
        TurnStep::Confirm(p) => panic!("на вопрос о продуктивности модель предложила изменение: {}", p.description),
    };
    println!("ВОПРОС: {question}\nИНСТРУМЕНТЫ: {:?}\nОТВЕТ ({:.1} с): {answer}", turn.tools_used, started.elapsed().as_secs_f32());
    assert!(turn.tools_used.iter().any(|t| t == "get_productivity_report"), "{:?}", turn.tools_used);
}
