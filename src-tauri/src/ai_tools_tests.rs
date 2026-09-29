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
        let out = run(&f.conn, f.ws, name, &json!({}));
        assert!(!out.contains("нет. Доступны"), "{name} не подключён к run(): {out}");
    }
    assert_eq!(definitions().len(), 9);
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
    assert_eq!(v["checklist"], json!([{"text": "Сбор", "done": true}, {"text": "Текст", "done": false}]));
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

// ─── Живая проверка с настоящей моделью ───

/// Вопросы из «Проверки» Фазы 2 — настоящей модели через тот же цикл
/// `converse`, что у `ollama_chat`, но на базе в памяти.
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

    let model = std::env::var("TASKFLOW_AI_MODEL").unwrap_or_else(|_| "qwen2.5:7b-instruct-q4_K_M".into());
    let settings = AiSettings {
        ollama_url: crate::ollama::DEFAULT_OLLAMA_URL.into(),
        model,
        context_length: 20,
        timeout_seconds: 300,
    };
    let system = system_prompt(&f.conn, f.ws).unwrap();

    for (question, expected_tool) in [
        ("Сколько у меня досок?", "get_boards"),
        ("Какие задачи просрочены?", "get_overdue_cards"),
    ] {
        let messages = build_chat_request(&system, &[], question);
        let started = std::time::Instant::now();
        let (answer, used) = tauri::async_runtime::block_on(converse(&settings, messages, |name, args| {
            println!("    → {name}({args})");
            Ok(run(&f.conn, f.ws, name, args))
        }))
        .unwrap();
        println!("ВОПРОС: {question}\nИНСТРУМЕНТЫ: {used:?}\nОТВЕТ ({:.1} с): {answer}\n", started.elapsed().as_secs_f32());
        assert!(used.iter().any(|t| t == expected_tool), "ждали {expected_tool}, вызваны {used:?}");
    }
}
