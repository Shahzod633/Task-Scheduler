// ============================================
// TaskFlow — тесты сверки ответа ассистента с данными
// ============================================
// Ответы — настоящие, из живых прогонов 26.11.2026 на jarvis:latest
// (qwen2.5 7B): и неверные, которые надо ловить, и верные, которые трогать
// нельзя.

use super::*;
use serde_json::json;

fn boards_facts() -> Facts {
    let mut f = Facts::default();
    f.add_tool_result(
        "get_boards",
        &json!({ "count": 2, "boards": [
            { "id": 1, "name": "Учёба", "starred": false, "open_cards": 0, "done_cards": 0 },
            { "id": 2, "name": "Shady's tasks", "starred": false, "open_cards": 2, "done_cards": 1 },
        ]})
        .to_string(),
    );
    f
}

fn report(days: i64, retries: i64) -> Value {
    json!({
        "period_days": days,
        "today": "2026-11-26",
        "during_period": { "created": 4, "completed": 1, "retries_requested": retries,
                           "time_tracked": "0 мин", "time_tracked_seconds": 0 },
        "right_now": { "active": 3, "overdue": 2, "needs_attention": 1 },
        "top_boards_by_active_cards": [{ "board": "Shady's tasks", "active_cards": 3 }],
    })
}

fn report_facts(days: i64, retries: i64) -> Facts {
    let mut f = Facts::default();
    f.add_report(&report(days, retries));
    f
}

#[test]
fn no_open_tasks_while_the_tool_says_two_is_caught() {
    let f = boards_facts();
    let answer = "У вас есть 2 доски: \"Учёба\" и \"Shady's tasks\". В настоящее время на них нет открытых задач.";
    let problems = check(answer, &f);
    assert_eq!(problems, vec![Problem::Number {
        clause: "В настоящее время на них нет открытых задач".into(),
        kind: FactKind::Open,
        said: 0,
    }]);
    let nudge_text = nudge(&problems, &f);
    assert!(nudge_text.contains("открытых задач: 2"), "{nudge_text}");
    let fixed = correct(answer, &problems, &f);
    assert!(fixed.ends_with("⚠ Приложение: по данным открытых задач — 2."), "{fixed}");
}

#[test]
fn invented_done_count_is_reported_as_the_board_total() {
    let f = boards_facts();
    let answer = "У вас две доски: \"Учёба\" и \"Shady's tasks\". В общей сложности на них нет открытых задач, \
                  но есть 2 завершённые задачи.";
    let problems = check(answer, &f);
    assert_eq!(problems.len(), 2, "{problems:?}");
    let fixed = correct(answer, &problems, &f);
    assert!(fixed.ends_with("по данным открытых задач — 2; завершено задач — 1."), "{fixed}");
}

#[test]
fn a_correct_per_board_list_passes() {
    let f = boards_facts();
    let answer = "Вот текущая информация о досках:\n\n- **Доска \"Учёба\"**: 0 открытых задач, 0 завершённых задач.\n\
                  - **Доска \"Shady's tasks\"**: 2 открытые задачи, 1 завершённая задача.";
    assert_eq!(check(answer, &f), vec![]);
}

#[test]
fn a_named_board_may_legitimately_have_none() {
    let f = boards_facts();
    assert!(check("На доске «Учёба» нет открытых задач.", &f).is_empty());
    assert!(check("На доске «Shady's tasks» 2 открытые задачи.", &f).is_empty());
}

#[test]
fn today_about_a_weekly_report_is_caught_and_corrected() {
    let f = report_facts(7, 0);
    let answer = "Ваше пространство содержит 2 просроченных задачи и 1 задача, требующая внимания. \
                  Сегодня вы создали 4 новые задачи и завершили 1. Чем могу помочь?";
    let problems = check(answer, &f);
    assert_eq!(problems, vec![Problem::Period {
        clause: "Сегодня вы создали 4 новые задачи и завершили 1".into(),
        days: 7,
    }]);
    let fixed = correct(answer, &problems, &f);
    assert!(fixed.contains("За последние 7 дней вы создали 4 новые задачи и завершили 1."), "{fixed}");
    assert!(!fixed.contains('⚠'), "период поправлен в тексте, приписка не нужна: {fixed}");
}

#[test]
fn a_one_day_report_may_say_today() {
    let f = report_facts(1, 0);
    assert!(check("Сегодня вы создали 4 задачи и завершили 1.", &f).is_empty());
}

#[test]
fn invented_retries_are_caught() {
    let f = report_facts(30, 0);
    let answer = "За последние 30 дней вы создали 4 задачи и завершили 1. Всего было 4 попытки запрошенные, \
                  но ни на одну задачу не было отслежено время.";
    let problems = check(answer, &f);
    assert_eq!(problems, vec![Problem::Number {
        clause: "Всего было 4 попытки запрошенные".into(),
        kind: FactKind::Retries,
        said: 4,
    }]);
    let fixed = correct(answer, &problems, &f);
    assert!(fixed.contains("запрошено попыток — 0"), "{fixed}");
}

#[test]
fn correct_answers_from_the_live_runs_pass() {
    let f = report_facts(30, 0);
    for answer in [
        "За последние 30 дней вы создали 4 задачи и завершили 1. Всего задач на доске \"Shady's tasks\" сейчас 3, \
         одна из которых требует вашего внимания, а также есть 2 просроченные задачи.",
        "На данный момент у вас активно 3 задачи, одна требует внимания и две просрочены. \
         Топ-1 доска по количеству активных задач — \"Shady's tasks\".",
        "Ваше пространство содержит 2 просроченных задачи и 1 задача, требующая внимания.",
    ] {
        assert!(check(answer, &f).is_empty(), "ложная тревога: {answer} → {:?}", check(answer, &f));
    }
}

#[test]
fn dates_years_and_periods_are_not_counts() {
    let mut f = Facts::default();
    f.add_tool_result("get_overdue_cards", &json!({
        "count": 1,
        "cards": [{ "title": "Сдать отчёт по проекту", "due_date": "2026-11-25" }],
        "today": "2026-11-26",
    }).to_string());
    for answer in [
        "Просрочена задача «Сдать отчёт по проекту». Эта задача была запланирована на 2026-11-25, но не выполнена.",
        "У вас просрочена задача \"Сдать отчёт по проекту\", срок до 25 ноября 2026 года.",
        "За 3 дня просрочена 1 задача.",
    ] {
        assert!(check(answer, &f).is_empty(), "ложная тревога: {answer} → {:?}", check(answer, &f));
    }
    assert!(!check("У вас 3 просроченные задачи.", &f).is_empty());
}

#[test]
fn without_data_nothing_is_checked() {
    let f = Facts::default();
    assert!(check("Всего было 4 попытки и нет открытых задач.", &f).is_empty());
    let mut created = Facts::default();
    created.add_tool_result("create_card", r#"{"ok":true,"what_was_done":"Создать задачу"}"#);
    assert!(created.is_empty(), "изменения числами не считаются");
}

#[test]
fn kinds_the_data_does_not_have_are_not_judged() {
    // В get_boards нет просроченных — «3 просроченных» не с чем сверять.
    let f = boards_facts();
    assert!(check("Из них 3 просроченных.", &f).is_empty());
}

#[test]
fn per_board_values_count_as_known_numbers() {
    let f = report_facts(7, 0);
    // 3 — и итог активных, и значение по доске: принимается.
    assert!(check("На доске «Shady's tasks» 3 активные задачи.", &f).is_empty());
}

#[test]
fn a_number_does_not_reach_across_and() {
    let f = report_facts(7, 0);
    let answer = "У вас 2 задачи просрочены и 1 требует вашего внимания. Свободного времени сегодня нет.";
    assert_eq!(check(answer, &f), vec![]);
    assert!(!check("У вас 2 задачи просрочены и 3 требуют внимания.", &f).is_empty());
}

#[test]
fn a_date_the_tool_did_not_write_is_replaced_with_the_written_one() {
    let mut f = Facts::default();
    f.add_tool_result("create_card", &json!({
        "ok": true, "card_id": 5,
        "what_was_done": "Создать задачу «Купить домен» на доске «Shady's tasks» в колонке «Тестирование»: срок 30.11.2026, приоритет средний",
    }).to_string());
    f.allow_dates_from("2026-11-26");
    let answer = "Задача «Купить домен» создана в колонке «Тестирование». Срок до понедельника 2026-12-03, приоритет — средний.";
    let problems = check(answer, &f);
    assert_eq!(problems, vec![Problem::Date { said: "2026-12-03".into(), right: Some("2026-11-30".into()) }]);
    assert!(nudge(&problems, &f).contains("записано 30.11.2026"));
    let fixed = correct(answer, &problems, &f);
    assert!(fixed.contains("до понедельника 30.11.2026"), "{fixed}");
    // Верная дата в любой записи — без вопросов; сегодняшняя — тоже.
    assert!(check("Срок — 30.11.2026.", &f).is_empty());
    assert!(check("Срок — 2026-11-30.", &f).is_empty());
    assert!(check("Отмечено за сегодня, 26.11.2026.", &f).is_empty());
}

#[test]
fn dates_are_found_in_both_spellings_and_not_inside_longer_numbers() {
    assert_eq!(
        dates_in("до 2026-12-03 или 30.11.2026, но не 12026-11-30 и не 1.11.2026"),
        vec![("2026-12-03".to_string(), "2026-12-03".to_string()), ("30.11.2026".to_string(), "2026-11-30".to_string())]
    );
}
