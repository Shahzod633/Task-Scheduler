// ============================================
// TaskFlow — тесты трекера привычек
// ============================================
// Стрик проверяется на чистой функции с номерами дней, всё остальное — на
// базе в памяти с настоящей схемой (`db::create_schema`), как в
// commands_tests.rs.

use super::*;
use rusqlite::Connection;

fn db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
    crate::db::create_schema(&conn).unwrap();
    conn.execute("INSERT INTO workspaces (name) VALUES ('Тест')", ()).unwrap();
    conn
}

/// Местная дата со сдвигом от сегодня, «ГГГГ-ММ-ДД».
fn date_offset(conn: &Connection, days: i64) -> String {
    conn.query_row("SELECT date('now', 'localtime', ?1)", params![format!("{days:+} days")], |r| r.get(0))
        .unwrap()
}

fn input(title: &str, frequency: &str) -> HabitInput {
    HabitInput {
        title: title.into(),
        description: None,
        frequency_type: frequency.into(),
        weekdays: Vec::new(),
        target_per_week: None,
        time_of_day: "morning".into(),
        effort_level: 3,
        fallback_title: None,
    }
}

/// Привычка, заведённая `days_ago` дней назад.
fn habit_since(conn: &Connection, title: &str, days_ago: i64) -> Habit {
    let habit = create_habit_in(conn, &input(title, "daily")).unwrap();
    conn.execute(
        "UPDATE habits SET created_at = datetime('now', ?1) WHERE id = ?2",
        params![format!("-{} days", days_ago), habit.id],
    )
    .unwrap();
    habit
}

fn logs(pairs: &[(i64, &str)]) -> HashMap<i64, String> {
    pairs.iter().map(|(d, s)| (*d, s.to_string())).collect()
}

/// Понедельник: 1970-01-05 — день 4.
const MONDAY: i64 = 4 + 7 * 2900;

fn streaks(freq: &Frequency, start: i64, today: i64, pairs: &[(i64, &str)]) -> (i64, i64) {
    streaks_from_units(&streak_units(freq, start, today, &logs(pairs)))
}

// ─── Даты ───

#[test]
fn weekday_and_week_start_follow_the_calendar() {
    // 26 ноября 2026 — четверг.
    let day = day_number("2026-11-26").unwrap();
    assert_eq!(weekday_of(day), 3);
    assert_eq!(day_string(week_start(day)), "2026-11-23");
    assert_eq!(weekday_of(MONDAY), 0);
    assert_eq!(day_string(day_number("2024-02-29").unwrap()), "2024-02-29");
}

// ─── Стрик: дни ───

#[test]
fn daily_streak_counts_consecutive_days_and_today_is_pending() {
    let s = 100;
    let pairs = [(s, "completed"), (s + 1, "completed"), (s + 2, "fallback"), (s + 3, "completed")];
    assert_eq!(streaks(&Frequency::Daily, s, s + 4, &pairs), (4, 4));
}

#[test]
fn failed_resets_the_current_streak_but_keeps_the_longest() {
    let s = 100;
    let pairs = [(s, "completed"), (s + 1, "completed"), (s + 2, "failed"), (s + 3, "completed")];
    assert_eq!(streaks(&Frequency::Daily, s, s + 3, &pairs), (1, 2));
}

#[test]
fn skipped_freezes_the_streak() {
    let s = 100;
    let pairs = [(s, "completed"), (s + 1, "skipped"), (s + 2, "completed")];
    assert_eq!(streaks(&Frequency::Daily, s, s + 2, &pairs), (2, 2));
}

#[test]
fn a_past_day_without_a_mark_breaks_the_streak() {
    let s = 100;
    let pairs = [(s, "completed"), (s + 2, "completed")];
    assert_eq!(streaks(&Frequency::Daily, s, s + 3, &pairs), (1, 1));
}

#[test]
fn weekly_days_ignore_days_off_the_schedule() {
    let mut mask = [false; 7];
    mask[0] = true; // пн
    mask[2] = true; // ср
    mask[4] = true; // пт
    let freq = Frequency::WeeklyDays(mask);
    let pairs = [(MONDAY, "completed"), (MONDAY + 2, "completed"), (MONDAY + 4, "completed")];
    // Вторник, четверг и выходные без отметок ничего не рвут; следующий
    // понедельник — сегодня, ещё не отмечен.
    assert_eq!(streaks(&freq, MONDAY, MONDAY + 7, &pairs), (3, 3));
    // А пропущенная среда рвёт.
    let pairs = [(MONDAY, "completed"), (MONDAY + 4, "completed")];
    assert_eq!(streaks(&freq, MONDAY, MONDAY + 5, &pairs), (1, 1));
}

// ─── Стрик: недели ───

#[test]
fn x_per_week_counts_weeks_that_met_the_target() {
    let freq = Frequency::XPerWeek(2);
    let pairs = [
        (MONDAY, "completed"),
        (MONDAY + 3, "completed"), // неделя 1: 2 из 2
        (MONDAY + 8, "completed"), // неделя 2: 1 из 2 — рвёт
        (MONDAY + 14, "completed"),
        (MONDAY + 15, "fallback"), // неделя 3: 2 из 2
    ];
    // Неделя 4 идёт (сегодня вторник), выполнений ещё нет — она ничего не решает.
    assert_eq!(streaks(&freq, MONDAY, MONDAY + 22, &pairs), (1, 1));
}

#[test]
fn skips_lower_the_weekly_target_and_an_all_skipped_week_is_frozen() {
    let freq = Frequency::XPerWeek(3);
    let pairs = [(MONDAY, "completed"), (MONDAY + 1, "completed"), (MONDAY + 2, "skipped")];
    assert_eq!(streaks(&freq, MONDAY, MONDAY + 6, &pairs), (1, 1));

    let freq = Frequency::XPerWeek(2);
    let pairs = [
        (MONDAY, "completed"),
        (MONDAY + 1, "completed"),
        (MONDAY + 7, "skipped"),
        (MONDAY + 8, "skipped"),
        (MONDAY + 14, "completed"),
        (MONDAY + 15, "completed"),
    ];
    assert_eq!(streaks(&freq, MONDAY, MONDAY + 20, &pairs), (2, 2));
}

#[test]
fn a_short_first_week_needs_only_the_days_it_has() {
    // Завели в субботу с нормой 3 — в неделе осталось два дня.
    let freq = Frequency::XPerWeek(3);
    let pairs = [(MONDAY + 5, "completed"), (MONDAY + 6, "completed")];
    assert_eq!(streaks(&freq, MONDAY + 5, MONDAY + 7, &pairs), (1, 1));
}

// ─── Форма ───

#[test]
fn habit_form_is_validated() {
    let conn = db();
    let mut bad = input("  ", "daily");
    assert_eq!(create_habit_in(&conn, &bad).unwrap_err(), ERR_HABIT_TITLE_EMPTY);

    bad = input("Чтение", "monthly");
    assert_eq!(create_habit_in(&conn, &bad).unwrap_err(), ERR_HABIT_FREQUENCY);

    bad = input("Чтение", "weekly_days");
    assert_eq!(create_habit_in(&conn, &bad).unwrap_err(), ERR_HABIT_WEEKDAYS);
    bad.weekdays = vec!["mon".into(), "funday".into()];
    assert_eq!(create_habit_in(&conn, &bad).unwrap_err(), ERR_HABIT_WEEKDAY_UNKNOWN);

    bad = input("Чтение", "x_per_week");
    assert_eq!(create_habit_in(&conn, &bad).unwrap_err(), ERR_HABIT_TARGET);
    bad.target_per_week = Some(8);
    assert_eq!(create_habit_in(&conn, &bad).unwrap_err(), ERR_HABIT_TARGET);

    bad = input("Чтение", "daily");
    bad.time_of_day = "night".into();
    assert_eq!(create_habit_in(&conn, &bad).unwrap_err(), ERR_HABIT_TIME);

    bad = input("Чтение", "daily");
    bad.effort_level = 0;
    assert_eq!(create_habit_in(&conn, &bad).unwrap_err(), ERR_HABIT_EFFORT);

    bad = input(&"я".repeat(121), "daily");
    assert_eq!(create_habit_in(&conn, &bad).unwrap_err(), ERR_HABIT_TITLE_LONG);

    let count: i64 = conn.query_row("SELECT COUNT(*) FROM habits", [], |r| r.get(0)).unwrap();
    assert_eq!(count, 0);
}

#[test]
fn schedule_is_normalized_and_round_trips() {
    let conn = db();
    let mut form = input("  Зарядка  ", "weekly_days");
    form.weekdays = vec!["fri".into(), "mon".into(), "mon".into()];
    form.description = Some("   ".into());
    form.fallback_title = Some(" 5 приседаний ".into());
    let habit = create_habit_in(&conn, &form).unwrap();
    assert_eq!(habit.title, "Зарядка");
    assert_eq!(habit.weekdays, vec!["mon", "fri"]);
    assert_eq!(habit.description, "");
    assert_eq!(habit.fallback_title.as_deref(), Some("5 приседаний"));
    assert_eq!(habit.streak_unit, "day");
    let stored: String = conn
        .query_row("SELECT frequency_config FROM habits WHERE id = ?1", params![habit.id], |r| r.get(0))
        .unwrap();
    assert_eq!(stored, r#"["mon","fri"]"#);

    let mut form = input("Зарядка", "x_per_week");
    form.target_per_week = Some(3);
    let habit = update_habit_in(&conn, habit.id, &form).unwrap();
    assert_eq!(habit.target_per_week, Some(3));
    assert!(habit.weekdays.is_empty());
    assert_eq!(habit.streak_unit, "week");
    assert!(habit.due_today, "норма недели ещё не набрана");

    assert_eq!(update_habit_in(&conn, 999, &form).unwrap_err(), ERR_HABIT_NOT_FOUND);
}

// ─── Отметки ───

#[test]
fn a_second_mark_for_the_same_day_replaces_the_first() {
    let conn = db();
    let habit = create_habit_in(&conn, &input("Медитация", "daily")).unwrap();
    let today = date_offset(&conn, 0);

    let after = log_habit_in(&conn, habit.id, &today, "completed", Some("лишняя причина".into())).unwrap();
    assert_eq!(after.current_streak, 1);
    let log = after.today.unwrap();
    assert_eq!(log.status, "completed");
    assert_eq!(log.skip_reason, None, "у выполненной причины нет");
    assert!(log.completed_at.is_some());

    let after = log_habit_in(&conn, habit.id, &today, "skipped", Some("  заболел ".into())).unwrap();
    let log = after.today.unwrap();
    assert_eq!(log.status, "skipped");
    assert_eq!(log.skip_reason.as_deref(), Some("заболел"));
    assert_eq!(log.completed_at, None);
    assert_eq!(after.current_streak, 0);

    let rows: i64 = conn.query_row("SELECT COUNT(*) FROM habit_logs", [], |r| r.get(0)).unwrap();
    assert_eq!(rows, 1);

    let cleared = clear_habit_log_in(&conn, habit.id, &today).unwrap();
    assert!(cleared.today.is_none());
}

#[test]
fn bad_marks_are_rejected() {
    let conn = db();
    let habit = create_habit_in(&conn, &input("Медитация", "daily")).unwrap();
    let today = date_offset(&conn, 0);
    let tomorrow = date_offset(&conn, 1);

    assert_eq!(log_habit_in(&conn, habit.id, &tomorrow, "completed", None).unwrap_err(), ERR_HABIT_DATE_FUTURE);
    assert_eq!(log_habit_in(&conn, habit.id, "26.11.2026", "completed", None).unwrap_err(), ERR_HABIT_DATE);
    assert_eq!(log_habit_in(&conn, habit.id, &today, "done", None).unwrap_err(), ERR_HABIT_STATUS);
    assert_eq!(log_habit_in(&conn, habit.id, &today, "fallback", None).unwrap_err(), ERR_HABIT_NO_FALLBACK);
    assert_eq!(
        log_habit_in(&conn, habit.id, &today, "failed", Some("я".repeat(301))).unwrap_err(),
        ERR_SKIP_REASON_LONG
    );
    assert_eq!(log_habit_in(&conn, 999, &today, "completed", None).unwrap_err(), ERR_HABIT_NOT_FOUND);

    set_habit_archived_in(&conn, habit.id, true).unwrap();
    assert_eq!(log_habit_in(&conn, habit.id, &today, "completed", None).unwrap_err(), ERR_HABIT_ARCHIVED);
}

#[test]
fn fallback_needs_a_light_version_and_keeps_the_streak() {
    let conn = db();
    let habit = habit_since(&conn, "Пробежка", 1);
    let mut form = input("Пробежка", "daily");
    form.fallback_title = Some("Прогулка 10 минут".into());
    update_habit_in(&conn, habit.id, &form).unwrap();

    log_habit_in(&conn, habit.id, &date_offset(&conn, -1), "completed", None).unwrap();
    let after = log_habit_in(&conn, habit.id, &date_offset(&conn, 0), "fallback", None).unwrap();
    assert_eq!(after.current_streak, 2);
}

#[test]
fn deleting_a_habit_removes_its_marks() {
    let conn = db();
    let habit = create_habit_in(&conn, &input("Медитация", "daily")).unwrap();
    log_habit_in(&conn, habit.id, &date_offset(&conn, 0), "completed", None).unwrap();
    delete_habit_in(&conn, habit.id).unwrap();
    let rows: i64 = conn.query_row("SELECT COUNT(*) FROM habit_logs", [], |r| r.get(0)).unwrap();
    assert_eq!(rows, 0);
    assert_eq!(delete_habit_in(&conn, habit.id).unwrap_err(), ERR_HABIT_NOT_FOUND);
}

#[test]
fn reading_the_list_breaks_a_streak_left_alone() {
    let conn = db();
    let habit = habit_since(&conn, "Медитация", 5);
    for days_ago in 2..=5 {
        log_habit_in(&conn, habit.id, &date_offset(&conn, -days_ago), "completed", None).unwrap();
    }
    // Вчера отметки нет: стрик сломался, хотя никто ничего не отмечал.
    let list = list_habits_in(&conn, false).unwrap();
    assert_eq!((list[0].current_streak, list[0].longest_streak), (0, 4));
    let stored: i64 = conn
        .query_row("SELECT current_streak FROM habits WHERE id = ?1", params![habit.id], |r| r.get(0))
        .unwrap();
    assert_eq!(stored, 0, "пересчитанный стрик записан в базу");
}

#[test]
fn archived_habits_keep_their_streak_and_hide_from_the_list() {
    let conn = db();
    let habit = habit_since(&conn, "Медитация", 3);
    log_habit_in(&conn, habit.id, &date_offset(&conn, -3), "completed", None).unwrap();
    log_habit_in(&conn, habit.id, &date_offset(&conn, -2), "completed", None).unwrap();
    // Архивируем, пока стрик ещё не прочитан как сломанный.
    conn.execute("UPDATE habits SET current_streak = 2, is_archived = 1 WHERE id = ?1", params![habit.id]).unwrap();
    assert!(list_habits_in(&conn, false).unwrap().is_empty());
    let all = list_habits_in(&conn, true).unwrap();
    assert_eq!(all[0].current_streak, 2);
}

#[test]
fn backfilled_marks_before_creation_count() {
    let conn = db();
    let habit = create_habit_in(&conn, &input("Чтение", "daily")).unwrap();
    log_habit_in(&conn, habit.id, &date_offset(&conn, -1), "completed", None).unwrap();
    let after = log_habit_in(&conn, habit.id, &date_offset(&conn, 0), "completed", None).unwrap();
    assert_eq!(after.current_streak, 2);
}

// ─── Тепловая карта ───

#[test]
fn heatmap_counts_marks_against_the_schedule() {
    let conn = db();
    let a = habit_since(&conn, "А", 3);
    let b = habit_since(&conn, "Б", 3);
    let yesterday = date_offset(&conn, -1);
    log_habit_in(&conn, a.id, &yesterday, "completed", None).unwrap();
    log_habit_in(&conn, b.id, &yesterday, "failed", Some("аврал".into())).unwrap();

    let cells = heatmap_in(&conn, None, 7).unwrap();
    assert_eq!(cells.len(), 7);
    assert_eq!(cells.last().unwrap().date, date_offset(&conn, 0));
    let y = cells.iter().find(|c| c.date == yesterday).unwrap();
    assert_eq!((y.due, y.completed, y.failed), (2, 1, 1));
    // Пять дней назад привычек ещё не было.
    let early = cells.iter().find(|c| c.date == date_offset(&conn, -5)).unwrap();
    assert_eq!(early.due, 0);

    let only_a = heatmap_in(&conn, Some(a.id), 7).unwrap();
    let y = only_a.iter().find(|c| c.date == yesterday).unwrap();
    assert_eq!((y.due, y.completed, y.failed), (1, 1, 0));
    assert_eq!(heatmap_in(&conn, Some(999), 7).unwrap_err(), ERR_HABIT_NOT_FOUND);
}

// ─── Сводка «задачи + привычки» ───

fn board_with_final_column(conn: &Connection) -> (i64, i64) {
    conn.execute("INSERT INTO boards (workspace_id, name) VALUES (1, 'Работа')", ()).unwrap();
    let board = conn.last_insert_rowid();
    conn.execute("INSERT INTO columns (board_id, name, position) VALUES (?1, 'Новые', 0)", params![board]).unwrap();
    let open = conn.last_insert_rowid();
    conn.execute(
        "INSERT INTO columns (board_id, name, position, is_final) VALUES (?1, 'Закрыто', 1, 1)",
        params![board],
    )
    .unwrap();
    (open, conn.last_insert_rowid())
}

fn card(conn: &Connection, column: i64, due: &str, completed_days_ago: Option<i64>) {
    let completed = completed_days_ago.map(|d| format!("-{} days", d));
    conn.execute(
        "INSERT INTO cards (column_id, title, position, due_date, completed_at)
         VALUES (?1, 'Задача', 0, ?2, CASE WHEN ?3 IS NULL THEN NULL ELSE datetime('now', ?3) END)",
        params![column, due, completed],
    )
    .unwrap();
}

#[test]
fn daily_load_joins_tasks_and_habits_by_day() {
    let conn = db();
    let (open, done) = board_with_final_column(&conn);
    let d1 = date_offset(&conn, -1);
    let d2 = date_offset(&conn, -2);
    let today = date_offset(&conn, 0);

    card(&conn, open, &d1, None); // вчера не закрыта — просрочена
    card(&conn, done, &d1, Some(1)); // закрыта в свой день
    card(&conn, done, &d2, Some(1)); // закрыта на день позже срока
    card(&conn, open, &today, None); // сегодня ещё не просрочка
    card(&conn, done, &d2, None); // закрыта когда-то, дата неизвестна — не в счёт

    let habit = habit_since(&conn, "Медитация", 2);
    log_habit_in(&conn, habit.id, &d1, "failed", Some("дедлайн".into())).unwrap();

    let loads = daily_load_in(&conn, ANALYSIS_DAYS).unwrap();
    assert_eq!(loads.len(), ANALYSIS_DAYS as usize);
    assert_eq!(loads.last().unwrap().date, today);

    let day = |date: &str| loads.iter().find(|l| l.date == date).unwrap().clone();
    let y = day(&d1);
    assert_eq!((y.tasks_due, y.tasks_completed, y.tasks_overdue), (2, 2, 1));
    assert_eq!((y.habits_due, y.habits_failed, y.habits_missed), (1, 1, 0));
    assert_eq!(y.marks.len(), 1);
    assert_eq!(y.marks[0].reason.as_deref(), Some("дедлайн"));

    let b = day(&d2);
    assert_eq!((b.tasks_due, b.tasks_overdue), (2, 1));
    assert_eq!((b.habits_due, b.habits_missed), (1, 1));
    assert_eq!(b.marks[0].status, "missed");

    let t = day(&today);
    assert_eq!((t.tasks_due, t.tasks_overdue), (1, 0));
    assert_eq!((t.habits_due, t.habits_missed), (1, 0), "сегодня ещё не пропуск");
    assert!(t.marks.is_empty());
}

#[test]
fn archived_boards_and_habits_stay_out_of_the_summary() {
    let conn = db();
    let (open, _) = board_with_final_column(&conn);
    let d1 = date_offset(&conn, -1);
    card(&conn, open, &d1, None);
    conn.execute("UPDATE boards SET archived = 1", ()).unwrap();
    let habit = habit_since(&conn, "Медитация", 2);
    log_habit_in(&conn, habit.id, &d1, "completed", None).unwrap();
    set_habit_archived_in(&conn, habit.id, true).unwrap();

    let loads = daily_load_in(&conn, 3).unwrap();
    let y = loads.iter().find(|l| l.date == d1).unwrap();
    assert_eq!((y.tasks_due, y.tasks_overdue, y.habits_completed, y.habits_due), (0, 0, 0, 0));
}

// ─── ИИ-анализ: вход и разбор ответа ───

#[test]
fn analysis_needs_habits_and_at_least_one_mark() {
    let conn = db();
    let (input, _, _) = analysis_input(&conn).unwrap();
    assert_eq!(check_analysis_input(&input).unwrap_err(), ERR_ANALYSIS_NO_HABITS);

    let habit = habit_since(&conn, "Медитация", 3);
    let (input, _, _) = analysis_input(&conn).unwrap();
    assert_eq!(check_analysis_input(&input).unwrap_err(), ERR_ANALYSIS_NO_LOGS, "одни missed — не данные");

    log_habit_in(&conn, habit.id, &date_offset(&conn, -1), "completed", None).unwrap();
    let (input, from, to) = analysis_input(&conn).unwrap();
    check_analysis_input(&input).unwrap();
    assert_eq!(from, date_offset(&conn, -13));
    assert_eq!(to, date_offset(&conn, 0));
    assert_eq!(input["days"].as_array().unwrap().len(), 14);
    assert!(!input.to_string().contains("habit_id"), "id модели не нужны");

    let request = analysis_request(&input);
    assert_eq!(request[0].role, "system");
    assert!(request[1].content.contains("Медитация"));
}

#[test]
fn model_answer_is_parsed_and_checked() {
    let titles = vec!["Медитация".to_string(), "Пробежка".to_string()];
    let raw = r#"Вот анализ:
```json
{
  "workload_verdict": "  В дни с 4+ задачами срывается вечерняя пробежка.  ",
  "correlations": [
    {"pattern": "Пробежка срывается после просрочек", "evidence": "2026-11-20: 3 просрочки, failed", "confidence": "HIGH"},
    {"pattern": "Нужно больше верить в себя", "evidence": "мотивация важна", "confidence": "medium"},
    {"pattern": "Медитация держится", "evidence": "13 из 14 дней", "confidence": "высокая"},
    {"pattern": "Что-то", "evidence": "1 эпизод", "confidence": "maybe"}
  ],
  "actionable_adjustments": [
    {"habit": "пробежка", "change": "Перенести на утро", "reason": "срывы 18 и 20 ноября"},
    {"habit": "Йога", "change": "Делать по вторникам", "reason": "—"},
    {"habit": "", "change": "Включать лайт-версии в дни с >3 задачами", "reason": "4 из 5 срывов"},
    {"habit": "Медитация", "change": "   ", "reason": "пусто"}
  ]
}
```"#;
    let analysis = check_analysis(raw, &titles).unwrap();
    assert_eq!(analysis.workload_verdict, "В дни с 4+ задачами срывается вечерняя пробежка.");

    let confidences: Vec<&str> = analysis.correlations.iter().map(|c| c.confidence.as_str()).collect();
    assert_eq!(confidences, vec!["high", "high", "low"], "без цифр — отброшено, неизвестное — low");

    let habits: Vec<&str> = analysis.actionable_adjustments.iter().map(|a| a.habit.as_str()).collect();
    assert_eq!(habits, vec!["Пробежка", ""], "чужая привычка и пустой совет отброшены, название — точное");

    assert!(check_analysis(r#"{"workload_verdict": "  ", "correlations": [], "actionable_adjustments": []}"#, &titles).is_err());
    assert!(check_analysis("не JSON", &titles).is_err());
    assert!(check_analysis(r#"{"correlations": []}"#, &titles).is_err());
    // Пустые массивы допустимы — закономерностей может и не быть.
    let empty = check_analysis(r#"{"workload_verdict": "Связи не найдены."}"#, &titles).unwrap();
    assert!(empty.correlations.is_empty() && empty.actionable_adjustments.is_empty());
}

#[test]
fn cached_analysis_goes_stale_when_the_data_changes() {
    let conn = db();
    assert!(cached_analysis_in(&conn).unwrap().is_none());

    let habit = habit_since(&conn, "Медитация", 3);
    log_habit_in(&conn, habit.id, &date_offset(&conn, -1), "completed", None).unwrap();
    let (input, from, to) = analysis_input(&conn).unwrap();
    let analysis = HabitAnalysis {
        workload_verdict: "Связи не найдены.".into(),
        correlations: Vec::new(),
        actionable_adjustments: Vec::new(),
    };
    save_analysis(&conn, "qwen", &from, &to, &input, &analysis).unwrap();

    let cached = cached_analysis_in(&conn).unwrap().unwrap();
    assert_eq!(cached.analysis, analysis);
    assert_eq!(cached.model, "qwen");
    assert!(!cached.stale);

    log_habit_in(&conn, habit.id, &date_offset(&conn, 0), "completed", None).unwrap();
    assert!(cached_analysis_in(&conn).unwrap().unwrap().stale);

    // Второй анализ заменяет первый — строка в кэше одна.
    save_analysis(&conn, "qwen", &from, &to, &input, &analysis).unwrap();
    let rows: i64 = conn.query_row("SELECT COUNT(*) FROM habit_ai_reports", [], |r| r.get(0)).unwrap();
    assert_eq!(rows, 1);
}

#[test]
fn schema_lists_every_field_the_parser_reads() {
    let schema = analysis_schema();
    assert_eq!(schema["required"], json!(["workload_verdict", "correlations", "actionable_adjustments"]));
    assert_eq!(
        schema["properties"]["correlations"]["items"]["properties"]["confidence"]["enum"],
        json!(["high", "medium", "low"])
    );
}

// ─── Живой тест ───

/// Две недели с явной закономерностью: в дни с тремя и больше задачами со
/// сроком вечерняя пробежка срывается, утренняя медитация держится.
///
/// `cargo test --lib habits_live -- --ignored --nocapture`
/// Модель — из `TASKFLOW_AI_MODEL`, по умолчанию qwen2.5:7b-instruct-q4_K_M.
/// Ollama должна быть запущена.
#[test]
#[ignore]
fn habits_live_analysis_finds_the_pattern() {
    let conn = db();
    let (open, done) = board_with_final_column(&conn);
    let meditation = habit_since(&conn, "Медитация", 14);
    let run = habit_since(&conn, "Пробежка", 14);
    let mut form = input("Пробежка", "daily");
    form.time_of_day = "evening".into();
    form.effort_level = 4;
    update_habit_in(&conn, run.id, &form).unwrap();

    for days_ago in 1..=13 {
        let day = date_offset(&conn, -days_ago);
        let heavy = days_ago % 3 == 0;
        let tasks = if heavy { 4 } else { 1 };
        for i in 0..tasks {
            if i == 0 && heavy {
                card(&conn, open, &day, None);
            } else {
                card(&conn, done, &day, Some(days_ago));
            }
        }
        log_habit_in(&conn, meditation.id, &day, "completed", None).unwrap();
        if heavy {
            log_habit_in(&conn, run.id, &day, "failed", Some("засиделся с задачами".into())).unwrap();
        } else {
            log_habit_in(&conn, run.id, &day, "completed", None).unwrap();
        }
    }

    let (input, _, _) = analysis_input(&conn).unwrap();
    check_analysis_input(&input).unwrap();
    let titles = vec!["Медитация".to_string(), "Пробежка".to_string()];
    let settings = AiSettings {
        ollama_url: crate::ollama::DEFAULT_OLLAMA_URL.into(),
        model: std::env::var("TASKFLOW_AI_MODEL").unwrap_or_else(|_| "qwen2.5:7b-instruct-q4_K_M".into()),
        context_length: 20,
        timeout_seconds: 300,
    };
    let started = std::time::Instant::now();
    let analysis = tauri::async_runtime::block_on(ask_analysis(&settings, &input, &titles)).unwrap();
    println!("{:.1} с\n{}", started.elapsed().as_secs_f64(), serde_json::to_string_pretty(&analysis).unwrap());
    assert!(!analysis.workload_verdict.is_empty());
    assert!(!super::super::ai_tools::needs_language_retry(&analysis_text(&analysis)));
}
