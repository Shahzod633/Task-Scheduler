//! Инструменты ИИ-ассистента: через них модель читает данные TaskFlow.
//!
//! Механизм — tool calling Ollama (формат OpenAI): модель вместо текста
//! возвращает `tool_calls`, `ollama_chat` выполняет их здесь и отправляет
//! результаты обратно, модель формулирует ответ. Самописного разбора текста
//! нет.
//!
//! Модуль — дочерний к `commands.rs`: инструменты зовут те же функции, что и
//! команды интерфейса (`boards_in`, `cards_in`, `build_workspace_card_list`,
//! `build_workload_summary`, …), а не свои копии запросов.
//!
//! ## Модели не доверяем
//!
//! Локальные модели придумывают id и путают типы. Поэтому:
//! - пространство модель не выбирает вовсе — оно берётся из чата;
//! - каждый id проверяется: существует, не в архиве, **из этого
//!   пространства** — чужую доску по угаданному номеру не прочитать;
//! - число принимается и строкой из цифр (`"7"`), всё прочее — отказ;
//! - ошибка уходит модели как `{"error": "..."}` с подсказкой, откуда взять
//!   правильный id, — она успевает исправиться в следующем вызове.
//!
//! Читающие инструменты выполняются сразу. Изменяющие (`WRITE_TOOLS`) —
//! никогда молча: `prepare` проверяет и описывает, человек подтверждает в
//! чате, `execute` проверяет заново и только тогда пишет.

use super::*;
use serde_json::{json, Value};

/// Сколько раз подряд модель может попросить инструменты, прежде чем ответить
/// текстом. Вопросу «что просрочено на доске X» хватает двух кругов; больше
/// шести — это уже петля, а не рассуждение.
pub(super) const MAX_TOOL_ROUNDS: usize = 6;

/// Сколько карточек отдавать одним ответом. Окно контекста локальной модели —
/// несколько тысяч токенов, и список из сотни карточек вытеснил бы из него
/// разговор. Общее число отдаётся всегда, отдельно.
const MAX_CARDS: usize = 40;

fn tool(name: &str, description: &str, properties: Value, required: &[&str]) -> Value {
    json!({
        "type": "function",
        "function": {
            "name": name,
            "description": description,
            "parameters": { "type": "object", "properties": properties, "required": required },
        }
    })
}

/// Описания инструментов для `tools` в запросе к Ollama.
pub(super) fn definitions() -> Vec<Value> {
    let id = |what: &str| json!({ "type": "integer", "description": what });
    vec![
        tool("get_boards",
             "Список досок текущего пространства: id, название, сколько открытых и завершённых задач.",
             json!({}), &[]),
        tool("get_board_columns",
             "Колонки доски по порядку и сколько карточек в каждой. is_final — колонка завершённых задач.",
             json!({ "board_id": id("id доски из get_boards") }), &["board_id"]),
        tool("get_cards_in_column",
             "Карточки (задачи) одной колонки: название, приоритет, срок, исполнитель.",
             json!({ "column_id": id("id колонки из get_board_columns") }), &["column_id"]),
        tool("search_cards",
             "Поиск задач по части названия во всём пространстве.",
             json!({ "query": { "type": "string", "description": "часть названия задачи" } }), &["query"]),
        tool("get_card_details",
             "Всё о задаче: описание, доска и колонка, исполнитель, автор, приоритет, срок, чек-лист, \
              потраченное время, зависимости.",
             json!({ "card_id": id("id задачи из других инструментов") }), &["card_id"]),
        tool("get_workload",
             "Нагрузка по исполнителям: сколько открытых задач у каждого и с каким приоритетом.",
             json!({}), &[]),
        tool("get_overdue_cards",
             "Просроченные задачи: срок уже прошёл, а задача не в завершающей колонке.",
             json!({}), &[]),
        tool("get_cards_by_deadline_range",
             "Задачи со сроком в диапазоне дат включительно. Даты — ГГГГ-ММ-ДД.",
             json!({
                 "from": { "type": "string", "description": "первый день, ГГГГ-ММ-ДД" },
                 "to":   { "type": "string", "description": "последний день, ГГГГ-ММ-ДД" },
             }), &["from", "to"]),
        tool("get_time_summary",
             "Сколько времени потрачено по таймеру на задачу или на всю доску. Укажи ровно одно: card_id или board_id.",
             json!({ "card_id": id("id задачи"), "board_id": id("id доски") }), &[]),

        // ─── Изменения: каждое пользователь подтверждает в чате ───
        tool("create_card",
             "Создать задачу на доске. Она всегда встаёт в первую рабочую колонку. Срок, приоритет и \
              исполнитель необязательны: без срока — без срока, приоритет по умолчанию Medium, \
              исполнитель по умолчанию — сам пользователь.",
             json!({
                 "board_id": id("id доски из get_boards"),
                 "title": { "type": "string", "description": "название задачи" },
                 "description": { "type": "string", "description": "описание, можно пустое" },
                 "due_date": { "type": "string", "description": "срок, ГГГГ-ММ-ДД; не раньше сегодняшнего дня" },
                 "priority": { "type": "string", "enum": PRIORITIES, "description": "Low, Medium или High" },
                 "assignee_id": id("member_id исполнителя из get_workload"),
             }), &["board_id", "title"]),
        tool("move_card",
             "Перенести задачу в другую колонку той же доски (в конец колонки). Из завершающей колонки \
              задачу не вернуть.",
             json!({ "card_id": id("id задачи"), "column_id": id("id колонки из get_board_columns") }),
             &["card_id", "column_id"]),
        tool("update_card_priority",
             "Сменить приоритет задачи.",
             json!({
                 "card_id": id("id задачи"),
                 "priority": { "type": "string", "enum": PRIORITIES, "description": "Low, Medium или High" },
             }), &["card_id", "priority"]),
        tool("update_card_assignee",
             "Назначить исполнителя задачи.",
             json!({ "card_id": id("id задачи"), "member_id": id("member_id из get_workload") }),
             &["card_id", "member_id"]),
        tool("add_checklist_item",
             "Добавить пункт в чек-лист задачи.",
             json!({ "card_id": id("id задачи"), "text": { "type": "string", "description": "текст пункта" } }),
             &["card_id", "text"]),
        tool("toggle_checklist_item",
             "Отметить пункт чек-листа выполненным или снять отметку.",
             json!({ "item_id": id("id пункта из checklist в get_card_details") }), &["item_id"]),
        tool("start_timer",
             "Запустить таймер учёта времени на задаче. Идущий таймер того же человека остановится.",
             json!({ "card_id": id("id задачи"), "member_id": id("чей таймер; по умолчанию — пользователя") }),
             &["card_id"]),
        tool("stop_timer",
             "Остановить идущий таймер.",
             json!({ "member_id": id("чей таймер; по умолчанию — пользователя") }), &[]),
    ]
}

/// Инструменты, меняющие данные. Их `ollama_chat` не выполняет сразу, а
/// показывает человеку превью и ждёт «Да».
pub(super) const WRITE_TOOLS: [&str; 8] = [
    "create_card", "move_card", "update_card_priority", "update_card_assignee",
    "add_checklist_item", "toggle_checklist_item", "start_timer", "stop_timer",
];

pub(super) fn is_write_tool(name: &str) -> bool {
    WRITE_TOOLS.contains(&name)
}

/// Выполняет инструмент и возвращает то, что уйдёт модели: JSON-строку с
/// данными или `{"error": "..."}`. Ошибка — не отказ всего вопроса, а
/// сообщение модели, чтобы она поправилась.
pub(super) fn run(conn: &rusqlite::Connection, workspace_id: i64, name: &str, args: &Value) -> String {
    let result = match name {
        "get_boards" => get_boards(conn, workspace_id),
        "get_board_columns" => arg_id(args, "board_id").and_then(|b| get_board_columns(conn, workspace_id, b)),
        "get_cards_in_column" => arg_id(args, "column_id").and_then(|c| get_cards_in_column(conn, workspace_id, c)),
        "search_cards" => arg_str(args, "query").and_then(|q| search_cards(conn, workspace_id, &q)),
        "get_card_details" => arg_id(args, "card_id").and_then(|c| get_card_details(conn, workspace_id, c)),
        "get_workload" => get_workload(conn, workspace_id),
        "get_overdue_cards" => get_overdue_cards(conn, workspace_id),
        "get_cards_by_deadline_range" => arg_date(args, "from")
            .and_then(|from| arg_date(args, "to").map(|to| (from, to)))
            .and_then(|(from, to)| get_cards_by_deadline_range(conn, workspace_id, &from, &to)),
        "get_time_summary" => get_time_summary(conn, workspace_id, args),
        _ => Err(format!("Инструмента «{}» нет. Доступны: {}", name, tool_names().join(", "))),
    };
    match result {
        Ok(value) => value.to_string(),
        Err(e) => json!({ "error": e }).to_string(),
    }
}

fn tool_names() -> Vec<String> {
    definitions()
        .iter()
        .filter_map(|t| t["function"]["name"].as_str().map(str::to_string))
        .collect()
}

// ─── Разбор аргументов ───

/// Целое из аргументов. Принимается и строка из цифр: модели нередко пишут
/// `"board_id": "3"`. Дробь, отрицательное, текст — отказ.
fn arg_id(args: &Value, key: &str) -> CmdResult<i64> {
    let value = args.get(key).filter(|v| !v.is_null());
    let parsed = match value {
        Some(Value::Number(n)) => n.as_i64(),
        Some(Value::String(s)) => s.trim().parse::<i64>().ok(),
        _ => return Err(format!("Не указан параметр {}", key)),
    };
    match parsed {
        Some(id) if id > 0 => Ok(id),
        _ => Err(format!("Параметр {} должен быть целым положительным числом — id из результатов других инструментов", key)),
    }
}

fn arg_str(args: &Value, key: &str) -> CmdResult<String> {
    match args.get(key) {
        Some(Value::String(s)) if !s.trim().is_empty() => Ok(s.trim().to_string()),
        _ => Err(format!("Не указан параметр {} (строка)", key)),
    }
}

fn arg_date(args: &Value, key: &str) -> CmdResult<String> {
    let s = arg_str(args, key)?;
    if is_calendar_date(&s) {
        Ok(s)
    } else {
        Err(format!("Параметр {} должен быть датой в формате ГГГГ-ММ-ДД, получено «{}»", key, s))
    }
}

// ─── Проверки принадлежности ───

/// Доска существует, не в архиве и принадлежит пространству чата.
fn check_board(conn: &rusqlite::Connection, workspace_id: i64, board_id: i64) -> CmdResult<String> {
    conn.query_row(
        "SELECT name FROM boards WHERE id = ?1 AND workspace_id = ?2 AND archived = 0",
        params![board_id, workspace_id],
        |row| row.get(0),
    )
    .optional()
    .map_err(to_string_err)?
    .ok_or_else(|| format!("Доски с id {} в этом пространстве нет — возьми id из get_boards", board_id))
}

/// Колонка существует, не в архиве, её доска — из пространства чата.
/// Возвращает `(название колонки, id доски, название доски)`.
fn check_column(conn: &rusqlite::Connection, workspace_id: i64, column_id: i64) -> CmdResult<(String, i64, String)> {
    conn.query_row(
        "SELECT col.name, b.id, b.name FROM columns col JOIN boards b ON b.id = col.board_id
          WHERE col.id = ?1 AND b.workspace_id = ?2 AND col.archived = 0 AND b.archived = 0",
        params![column_id, workspace_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )
    .optional()
    .map_err(to_string_err)?
    .ok_or_else(|| format!("Колонки с id {} в этом пространстве нет — возьми id из get_board_columns", column_id))
}

/// Сегодняшняя **местная** дата — срок карточки календарный (§12), и
/// «просрочено» считается по местному дню, как в `run_overdue_check`.
pub(super) fn today_local(conn: &rusqlite::Connection) -> CmdResult<String> {
    conn.query_row("SELECT date('now', 'localtime')", [], |row| row.get(0))
        .map_err(to_string_err)
}

/// id колонок-финалов пространства: карточка в такой колонке — сделанная.
fn final_columns(list: &WorkspaceCardList) -> HashSet<i64> {
    list.boards
        .iter()
        .flat_map(|b| b.columns.iter())
        .filter(|c| c.is_final)
        .map(|c| c.id)
        .collect()
}

/// Карточка в том виде, в каком её видит модель, — без позиций, цветов и
/// прочего, что только съест окно контекста.
fn card_brief(card: &CardRow, done: bool) -> Value {
    json!({
        "id": card.id,
        "title": card.title,
        "board": card.board_name,
        "column": card.column_name,
        "priority": card.priority,
        "due_date": card.due_date,
        "assignee": card.assignee.as_ref().map(|m| m.name.clone()),
        "done": done,
    })
}

fn cards_answer(cards: Vec<Value>) -> Value {
    let total = cards.len();
    let shown: Vec<Value> = cards.into_iter().take(MAX_CARDS).collect();
    let mut answer = json!({ "count": total, "cards": shown });
    if total > MAX_CARDS {
        answer["note"] = json!(format!("Показаны первые {} из {}", MAX_CARDS, total));
    }
    answer
}

// ─── Инструменты ───

fn get_boards(conn: &rusqlite::Connection, workspace_id: i64) -> CmdResult<Value> {
    let boards = boards_in(conn, workspace_id)?;
    let list = build_workspace_card_list(conn, workspace_id, false)?;
    let finals = final_columns(&list);
    let boards: Vec<Value> = boards
        .iter()
        .map(|b| {
            let (done, open): (Vec<&CardRow>, Vec<&CardRow>) = list
                .cards
                .iter()
                .filter(|c| c.board_id == b.id)
                .partition(|c| finals.contains(&c.column_id));
            json!({
                "id": b.id,
                "name": b.name,
                "starred": b.is_starred,
                "open_cards": open.len(),
                "done_cards": done.len(),
            })
        })
        .collect();
    Ok(json!({ "count": boards.len(), "boards": boards }))
}

fn get_board_columns(conn: &rusqlite::Connection, workspace_id: i64, board_id: i64) -> CmdResult<Value> {
    let board_name = check_board(conn, workspace_id, board_id)?;
    let mut columns = Vec::new();
    for col in columns_in(conn, board_id)? {
        let cards = cards_in(conn, col.id)?.len();
        columns.push(json!({ "id": col.id, "name": col.name, "is_final": col.is_final, "cards": cards }));
    }
    Ok(json!({ "board": board_name, "columns": columns }))
}

fn get_cards_in_column(conn: &rusqlite::Connection, workspace_id: i64, column_id: i64) -> CmdResult<Value> {
    let (column, _board_id, board) = check_column(conn, workspace_id, column_id)?;
    let names: HashMap<i64, String> = {
        let mut stmt = conn.prepare("SELECT id, name FROM members").map_err(to_string_err)?;
        let rows = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(to_string_err)?;
        rows.collect::<Result<_, _>>().map_err(to_string_err)?
    };
    let cards: Vec<Value> = cards_in(conn, column_id)?
        .iter()
        .map(|c| {
            json!({
                "id": c.id,
                "title": c.title,
                "priority": c.priority,
                "due_date": c.due_date,
                "assignee": c.assignee_id.and_then(|id| names.get(&id).cloned()),
                "checklist": format!("{}/{}", c.checklist_done, c.checklist_total),
            })
        })
        .collect();
    let mut answer = cards_answer(cards);
    answer["board"] = json!(board);
    answer["column"] = json!(column);
    Ok(answer)
}

fn search_cards(conn: &rusqlite::Connection, workspace_id: i64, query: &str) -> CmdResult<Value> {
    let needle = query.to_lowercase();
    let list = build_workspace_card_list(conn, workspace_id, false)?;
    let finals = final_columns(&list);
    let cards = list
        .cards
        .iter()
        .filter(|c| c.title.to_lowercase().contains(&needle))
        .map(|c| card_brief(c, finals.contains(&c.column_id)))
        .collect();
    Ok(cards_answer(cards))
}

fn get_card_details(conn: &rusqlite::Connection, workspace_id: i64, card_id: i64) -> CmdResult<Value> {
    // Отдельной `get_card` в приложении нет: карточка берётся из того же
    // списка пространства, что видит «Список», — заодно это и проверка, что
    // она из этого пространства и не в архиве.
    let list = build_workspace_card_list(conn, workspace_id, false)?;
    let finals = final_columns(&list);
    let card = list
        .cards
        .iter()
        .find(|c| c.id == card_id)
        .ok_or_else(|| format!("Задачи с id {} в этом пространстве нет — найди её через search_cards", card_id))?;

    let checklist: Vec<Value> = checklist_items_in(conn, card_id)?
        .iter()
        .map(|i| json!({ "id": i.id, "text": i.text, "done": i.is_done }))
        .collect();
    let seconds = total_time_in(conn, card_id)?;
    let deps = read_dependencies(conn, card_id)?;
    let dep = |d: &DependencyCard| json!({ "id": d.card_id, "title": d.title, "done": d.is_done });

    let mut answer = card_brief(card, finals.contains(&card.column_id));
    answer["description"] = json!(card.description);
    answer["author"] = json!(card.author.as_ref().map(|m| m.name.clone()));
    answer["created_at_utc"] = json!(card.created_at);
    answer["needs_attention"] = json!(card.is_mistake);
    answer["checklist"] = json!(checklist);
    answer["time_spent"] = json!(format_duration(seconds));
    answer["time_spent_seconds"] = json!(seconds);
    answer["blocked_by"] = json!(deps.blocked_by.iter().map(dep).collect::<Vec<_>>());
    answer["blocks"] = json!(deps.blocking.iter().map(dep).collect::<Vec<_>>());
    Ok(answer)
}

fn get_workload(conn: &rusqlite::Connection, workspace_id: i64) -> CmdResult<Value> {
    let rows: Vec<Value> = build_workload_summary(conn, workspace_id)?
        .iter()
        .map(|r| {
            json!({
                "member_id": r.member.id,
                "member": r.member.name,
                "is_me": r.member.is_self,
                "open_tasks": r.total,
                "high": r.high, "medium": r.medium, "low": r.low,
            })
        })
        .collect();
    Ok(json!({ "members": rows }))
}

fn get_overdue_cards(conn: &rusqlite::Connection, workspace_id: i64) -> CmdResult<Value> {
    let today = today_local(conn)?;
    let list = build_workspace_card_list(conn, workspace_id, false)?;
    let finals = final_columns(&list);
    let cards = list
        .cards
        .iter()
        .filter(|c| !finals.contains(&c.column_id))
        .filter(|c| c.due_date.as_deref().is_some_and(|d| d < today.as_str()))
        .map(|c| card_brief(c, false))
        .collect();
    let mut answer = cards_answer(cards);
    answer["today"] = json!(today);
    Ok(answer)
}

fn get_cards_by_deadline_range(conn: &rusqlite::Connection, workspace_id: i64, from: &str, to: &str) -> CmdResult<Value> {
    if from > to {
        return Err(format!("Начало диапазона {} позже конца {}", from, to));
    }
    let list = build_workspace_card_list(conn, workspace_id, false)?;
    let finals = final_columns(&list);
    let mut matched: Vec<&CardRow> = list
        .cards
        .iter()
        // Даты — строки ГГГГ-ММ-ДД, и сравнение строк здесь совпадает с
        // сравнением дат.
        .filter(|c| c.due_date.as_deref().is_some_and(|d| d >= from && d <= to))
        .collect();
    matched.sort_by(|a, b| a.due_date.cmp(&b.due_date));
    let open = matched.iter().filter(|c| !finals.contains(&c.column_id)).count();
    let cards = matched.iter().map(|c| card_brief(c, finals.contains(&c.column_id))).collect();
    let mut answer = cards_answer(cards);
    answer["open_count"] = json!(open);
    answer["done_count"] = json!(matched.len() - open);
    answer["from"] = json!(from);
    answer["to"] = json!(to);
    Ok(answer)
}

fn get_time_summary(conn: &rusqlite::Connection, workspace_id: i64, args: &Value) -> CmdResult<Value> {
    let has = |key: &str| args.get(key).is_some_and(|v| !v.is_null());
    match (has("card_id"), has("board_id")) {
        (true, false) => {
            let card_id = arg_id(args, "card_id")?;
            let list = build_workspace_card_list(conn, workspace_id, false)?;
            let card = list
                .cards
                .iter()
                .find(|c| c.id == card_id)
                .ok_or_else(|| format!("Задачи с id {} в этом пространстве нет — найди её через search_cards", card_id))?;
            let seconds = total_time_in(conn, card_id)?;
            Ok(json!({ "card": card.title, "time_spent": format_duration(seconds), "seconds": seconds }))
        }
        (false, true) => {
            let board_id = arg_id(args, "board_id")?;
            let board = check_board(conn, workspace_id, board_id)?;
            // Время архивных карточек тоже в счёт: оно было потрачено на эту
            // доску, архив его не отменяет. Идущая сессия — нет, как и в
            // `get_total_time`: её длительности ещё нет.
            let seconds: i64 = conn
                .query_row(
                    "SELECT COALESCE(SUM(t.duration_seconds), 0) FROM time_entries t
                       JOIN cards c ON c.id = t.card_id
                       JOIN columns col ON col.id = c.column_id
                      WHERE col.board_id = ?1",
                    params![board_id],
                    |row| row.get(0),
                )
                .map_err(to_string_err)?;
            Ok(json!({ "board": board, "time_spent": format_duration(seconds), "seconds": seconds }))
        }
        _ => Err("Укажи ровно один параметр: card_id или board_id".to_string()),
    }
}

/// «2 ч 05 мин» — модель пересказывает готовую строку, а не делит секунды сама
/// (и не ошибается в делении).
pub(super) fn format_duration(seconds: i64) -> String {
    let minutes = seconds.max(0) / 60;
    let (h, m) = (minutes / 60, minutes % 60);
    if h > 0 {
        format!("{} ч {:02} мин", h, m)
    } else {
        format!("{} мин", m)
    }
}

// ─── Изменения ───
//
// Изменение проходит два шага. `prepare` проверяет аргументы и составляет
// описание для превью — **ничего не меняя**. Человек видит превью в чате и
// жмёт «Да»; тогда `execute` проверяет всё заново (между превью и «Да» могли
// пройти минуты, и карточку успели перенести или убрать в архив) и только
// потом пишет в базу — теми же функциями, что и интерфейс.

/// Проверенное изменение: что именно сделать и как это описать человеку.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Prepared {
    pub tool: String,
    pub args: Value,
    pub description: String,
    pub warnings: Vec<String>,
}

/// Что делать с вызовом инструмента: читающий выполнен сразу, изменение ждёт
/// подтверждения. Ошибка проверки изменения — тоже `Result`: модели уходит
/// `{"error": …}`, человеку показывать нечего.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum ToolStep {
    Result(String),
    Confirm(Prepared),
}

pub(super) fn step(conn: &rusqlite::Connection, workspace_id: i64, name: &str, args: &Value) -> ToolStep {
    if !is_write_tool(name) {
        return ToolStep::Result(run(conn, workspace_id, name, args));
    }
    match prepare(conn, workspace_id, name, args) {
        Ok(prepared) => ToolStep::Confirm(prepared),
        Err(e) => ToolStep::Result(json!({ "error": e }).to_string()),
    }
}

/// Приоритет: английские значения из схемы и русские слова — модели пишут и
/// так, и так.
fn parse_priority(raw: &str) -> CmdResult<&'static str> {
    match raw.trim().to_lowercase().as_str() {
        "low" | "низкий" => Ok("Low"),
        "medium" | "средний" | "обычный" => Ok("Medium"),
        "high" | "высокий" => Ok("High"),
        _ => Err(format!("Приоритет «{}» неизвестен — только Low, Medium или High", raw)),
    }
}

fn priority_label(p: &str) -> &'static str {
    match p {
        "Low" => "низкий",
        "High" => "высокий",
        _ => "средний",
    }
}

/// «2026-10-10» → «10.10.2026».
fn date_label(d: &str) -> String {
    format!("{}.{}.{}", &d[8..10], &d[5..7], &d[0..4])
}

/// Карточка пространства из того же списка, что видит «Список»: заодно
/// проверка, что она наша и не в архиве.
fn find_card(conn: &rusqlite::Connection, workspace_id: i64, card_id: i64) -> CmdResult<CardRow> {
    build_workspace_card_list(conn, workspace_id, false)?
        .cards
        .into_iter()
        .find(|c| c.id == card_id)
        .ok_or_else(|| format!("Задачи с id {} в этом пространстве нет — найди её через search_cards", card_id))
}

fn column_is_final(conn: &rusqlite::Connection, column_id: i64) -> CmdResult<bool> {
    conn.query_row("SELECT is_final FROM columns WHERE id = ?1", params![column_id], |r| r.get::<_, i64>(0))
        .map(|v| v != 0)
        .map_err(to_string_err)
}

/// Участник по id: `(имя, это пользователь)`. Участники общие на всё
/// приложение (§27.1), поэтому проверки пространства здесь нет.
fn find_member(conn: &rusqlite::Connection, member_id: i64) -> CmdResult<(String, bool)> {
    conn.query_row("SELECT name, is_self FROM members WHERE id = ?1", params![member_id], |r| {
        Ok((r.get(0)?, r.get::<_, i64>(1)? != 0))
    })
    .optional()
    .map_err(to_string_err)?
    .ok_or_else(|| format!("Участника с id {} нет — возьми member_id из get_workload", member_id))
}

fn self_member(conn: &rusqlite::Connection) -> CmdResult<i64> {
    conn.query_row("SELECT id FROM members WHERE is_self = 1", [], |r| r.get(0))
        .optional()
        .map_err(to_string_err)?
        .ok_or_else(|| "В справочнике участников нет пользователя".to_string())
}

fn member_label(name: &str, is_self: bool) -> String {
    if is_self { format!("{} (вы)", name) } else { name.to_string() }
}

/// Необязательный id: нет или `null` — `None`, иначе как `arg_id`.
fn arg_opt_id(args: &Value, key: &str) -> CmdResult<Option<i64>> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(_) => arg_id(args, key).map(Some),
    }
}

fn arg_opt_str(args: &Value, key: &str) -> CmdResult<Option<String>> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if s.trim().is_empty() => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.trim().to_string())),
        Some(_) => Err(format!("Параметр {} должен быть строкой", key)),
    }
}

/// Первая рабочая колонка доски — туда встаёт новая задача. Финальная не
/// годится никогда: задача, созданная сразу закрытой, — не задача.
fn first_working_column(conn: &rusqlite::Connection, board_id: i64) -> CmdResult<(i64, String)> {
    conn.query_row(
        "SELECT id, name FROM columns WHERE board_id = ?1 AND archived = 0 AND is_final = 0
          ORDER BY position ASC, id ASC LIMIT 1",
        params![board_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .optional()
    .map_err(to_string_err)?
    .ok_or_else(|| "На доске нет рабочей колонки — задачу некуда поставить".to_string())
}

const MAX_TITLE_CHARS: usize = 200;
const MAX_CHECKLIST_CHARS: usize = 300;

/// Проверяет изменение и описывает его. Ничего не пишет.
pub(super) fn prepare(conn: &rusqlite::Connection, workspace_id: i64, name: &str, args: &Value) -> CmdResult<Prepared> {
    let mut warnings = Vec::new();
    let (args, description) = match name {
        "create_card" => {
            let board_id = arg_id(args, "board_id")?;
            let board = check_board(conn, workspace_id, board_id)?;
            let (column_id, column) = first_working_column(conn, board_id)?;
            let title = arg_str(args, "title")?;
            if title.chars().count() > MAX_TITLE_CHARS {
                return Err(format!("Название длиннее {} символов — сократи", MAX_TITLE_CHARS));
            }
            let description = arg_opt_str(args, "description")?.unwrap_or_default();
            let due_date = match arg_opt_str(args, "due_date")? {
                None => None,
                Some(d) => {
                    if !is_calendar_date(&d) {
                        return Err(format!("Срок должен быть датой ГГГГ-ММ-ДД, получено «{}»", d));
                    }
                    // Срок задаётся один раз и больше не правится (§20.2):
                    // задача, заведённая с прошедшим сроком, навсегда
                    // родилась бы просроченной.
                    if d < today_local(conn)? {
                        return Err(format!("Срок {} уже прошёл — уточни у пользователя дату", date_label(&d)));
                    }
                    Some(d)
                }
            };
            let priority = match arg_opt_str(args, "priority")? {
                None => "Medium",
                Some(p) => parse_priority(&p)?,
            };
            let assignee_id = match arg_opt_id(args, "assignee_id")? {
                Some(id) => id,
                None => self_member(conn)?,
            };
            let (assignee, is_self) = find_member(conn, assignee_id)?;

            let mut text = format!("Создать задачу «{}» на доске «{}» в колонке «{}»", title, board, column);
            let mut details = vec![
                match &due_date {
                    Some(d) => format!("срок {}", date_label(d)),
                    None => "без срока".to_string(),
                },
                format!("приоритет {}", priority_label(priority)),
                format!("исполнитель — {}", member_label(&assignee, is_self)),
            ];
            if !description.is_empty() {
                details.push(format!("описание: «{}»", description));
            }
            text.push_str(": ");
            text.push_str(&details.join(", "));
            if due_date.is_some() {
                warnings.push("Срок задаётся один раз — потом его можно будет только продлить попыткой.".to_string());
            }
            (json!({
                "board_id": board_id, "column_id": column_id, "title": title, "description": description,
                "due_date": due_date, "priority": priority, "assignee_id": assignee_id,
            }), text)
        }
        "move_card" => {
            let card = find_card(conn, workspace_id, arg_id(args, "card_id")?)?;
            let column_id = arg_id(args, "column_id")?;
            let (column, board_id, _) = check_column(conn, workspace_id, column_id)?;
            if board_id != card.board_id {
                return Err("Колонка на другой доске — переносить задачи между досками нельзя".to_string());
            }
            if column_id == card.column_id {
                return Err(format!("Задача уже в колонке «{}»", column));
            }
            if column_is_final(conn, card.column_id)? {
                return Err(ERR_CARD_IS_FINAL.to_string());
            }
            if column_is_final(conn, column_id)? {
                warnings.push(format!(
                    "«{}» — завершающая колонка: вернуть задачу оттуда будет нельзя.", column
                ));
                // Тот же довод, что у `confirmFinalColumnMove` в интерфейсе:
                // предупреждение, а не запрет (§28.1).
                let open: Vec<String> = read_dependencies(conn, card.id)?
                    .blocked_by
                    .iter()
                    .filter(|d| !d.is_done && !d.is_archived)
                    .map(|d| format!("«{}»", d.title))
                    .collect();
                if !open.is_empty() {
                    warnings.push(format!("Задачу блокируют незавершённые: {}.", open.join(", ")));
                }
            }
            (json!({ "card_id": card.id, "column_id": column_id }),
             format!("Перенести задачу «{}» из «{}» в «{}»", card.title, card.column_name, column))
        }
        "update_card_priority" => {
            let card = find_card(conn, workspace_id, arg_id(args, "card_id")?)?;
            let priority = parse_priority(&arg_str(args, "priority")?)?;
            if card.priority == priority {
                return Err(format!("У задачи уже {} приоритет", priority_label(priority)));
            }
            (json!({ "card_id": card.id, "priority": priority }),
             format!("Сменить приоритет задачи «{}»: {} → {}",
                     card.title, priority_label(&card.priority), priority_label(priority)))
        }
        "update_card_assignee" => {
            let card = find_card(conn, workspace_id, arg_id(args, "card_id")?)?;
            let member_id = arg_id(args, "member_id")?;
            let (name, is_self) = find_member(conn, member_id)?;
            if card.assignee.as_ref().map(|m| m.id) == Some(member_id) {
                return Err(format!("{} уже исполнитель этой задачи", name));
            }
            let was = card.assignee.as_ref().map(|m| member_label(&m.name, m.is_self))
                .unwrap_or_else(|| "никого".to_string());
            (json!({ "card_id": card.id, "member_id": member_id }),
             format!("Назначить исполнителем задачи «{}»: {} вместо {}", card.title, member_label(&name, is_self), was))
        }
        "add_checklist_item" => {
            let card = find_card(conn, workspace_id, arg_id(args, "card_id")?)?;
            let text = arg_str(args, "text")?;
            if text.chars().count() > MAX_CHECKLIST_CHARS {
                return Err(format!("Пункт длиннее {} символов — сократи", MAX_CHECKLIST_CHARS));
            }
            (json!({ "card_id": card.id, "text": text }),
             format!("Добавить в чек-лист задачи «{}» пункт «{}»", card.title, text))
        }
        "toggle_checklist_item" => {
            let item_id = arg_id(args, "item_id")?;
            let (card_id, text, done): (i64, String, bool) = conn
                .query_row(
                    "SELECT card_id, text, is_done FROM checklist_items WHERE id = ?1",
                    params![item_id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, i64>(2)? != 0)),
                )
                .optional()
                .map_err(to_string_err)?
                .ok_or_else(|| format!("Пункта чек-листа с id {} нет — возьми id из get_card_details", item_id))?;
            // Пункт проверяется через свою карточку: чужое пространство так же
            // недоступно, как и для самих карточек.
            let card = find_card(conn, workspace_id, card_id)
                .map_err(|_| format!("Пункта чек-листа с id {} в этом пространстве нет", item_id))?;
            let what = if done { "Снять отметку с пункта" } else { "Отметить выполненным пункт" };
            (json!({ "item_id": item_id }), format!("{} «{}» в задаче «{}»", what, text, card.title))
        }
        "start_timer" => {
            let card = find_card(conn, workspace_id, arg_id(args, "card_id")?)?;
            let member_id = match arg_opt_id(args, "member_id")? {
                Some(id) => id,
                None => self_member(conn)?,
            };
            let (name, is_self) = find_member(conn, member_id)?;
            if let Some(active) = read_active_timer(conn, member_id)? {
                if active.card_id == card.id {
                    return Err(format!("Таймер на «{}» уже идёт", card.title));
                }
                warnings.push(format!("Идущий таймер на «{}» остановится.", active.card_title));
            }
            let whose = if is_self { String::new() } else { format!(" ({})", name) };
            (json!({ "card_id": card.id, "member_id": member_id }),
             format!("Запустить таймер на задаче «{}»{}", card.title, whose))
        }
        "stop_timer" => {
            let member_id = match arg_opt_id(args, "member_id")? {
                Some(id) => id,
                None => self_member(conn)?,
            };
            let (name, is_self) = find_member(conn, member_id)?;
            let active = read_active_timer(conn, member_id)?.ok_or_else(|| {
                if is_self { "Сейчас таймер не идёт".to_string() } else { format!("У {} таймер не идёт", name) }
            })?;
            // Сколько идёт — в предупреждении, а не в описании: описание
            // сверяется перед выполнением, а это число растёт каждую минуту.
            warnings.push(format!("Таймер идёт {}.", format_duration(active.elapsed_seconds)));
            (json!({ "entry_id": active.entry_id, "member_id": member_id }),
             format!("Остановить таймер на задаче «{}»", active.card_title))
        }
        _ => return Err(format!("Инструмента «{}» нет", name)),
    };
    Ok(Prepared { tool: name.to_string(), args, description, warnings })
}

/// Выполняет подтверждённое изменение и возвращает то, что уйдёт модели.
///
/// Сначала — повторная проверка тем же `prepare` на тех же аргументах: если
/// за время ожидания что-то изменилось так, что описание уже не то, что
/// человек подтвердил, изменение не выполняется.
pub(super) fn execute(conn: &mut rusqlite::Connection, workspace_id: i64, prepared: &Prepared) -> CmdResult<Value> {
    let fresh = prepare(conn, workspace_id, &prepared.tool, &prepared.args)?;
    if fresh.description != prepared.description {
        return Err(format!(
            "Пока ждали подтверждения, данные изменились: теперь это «{}». Ничего не сделано.",
            fresh.description
        ));
    }
    let a = &fresh.args;
    let id = |key: &str| a[key].as_i64().ok_or_else(|| format!("нет {}", key));
    match fresh.tool.as_str() {
        "create_card" => {
            let tx = conn.unchecked_transaction().map_err(to_string_err)?;
            let title = a["title"].as_str().unwrap_or_default().to_string();
            let description = a["description"].as_str().unwrap_or_default().to_string();
            let card = create_card_in(&tx, id("column_id")?, title.clone(), description.clone())?;
            if let Some(due) = a["due_date"].as_str() {
                update_card_in(&tx, card.id, &title, &description, Some(due))?;
            }
            update_card_priority_in(&tx, card.id, a["priority"].as_str().unwrap_or("Medium"))?;
            update_card_assignee_in(&tx, card.id, Some(id("assignee_id")?))?;
            tx.commit().map_err(to_string_err)?;
            Ok(json!({ "ok": true, "card_id": card.id }))
        }
        "move_card" => {
            let column_id = id("column_id")?;
            let end: i64 = conn
                .query_row("SELECT COUNT(*) FROM cards WHERE column_id = ?1", params![column_id], |r| r.get(0))
                .map_err(to_string_err)?;
            move_card_in(conn, id("card_id")?, column_id, end)?;
            Ok(json!({ "ok": true }))
        }
        "update_card_priority" => {
            update_card_priority_in(conn, id("card_id")?, a["priority"].as_str().unwrap_or("Medium"))?;
            Ok(json!({ "ok": true }))
        }
        "update_card_assignee" => {
            update_card_assignee_in(conn, id("card_id")?, Some(id("member_id")?))?;
            Ok(json!({ "ok": true }))
        }
        "add_checklist_item" => {
            let item = create_checklist_item_in(conn, id("card_id")?, a["text"].as_str().unwrap_or_default())?;
            Ok(json!({ "ok": true, "item_id": item.id }))
        }
        "toggle_checklist_item" => {
            let done = toggle_checklist_item_in(conn, id("item_id")?)?;
            Ok(json!({ "ok": true, "done": done }))
        }
        "start_timer" => {
            let result = start_timer_in(conn, id("card_id")?, id("member_id")?)?;
            Ok(json!({ "ok": true, "stopped_other": result.stopped.map(|s| s.card_title) }))
        }
        "stop_timer" => {
            let entry = stop_timer_in(conn, id("entry_id")?)?;
            let seconds = entry.duration_seconds.unwrap_or(0);
            Ok(json!({ "ok": true, "session": format_duration(seconds) }))
        }
        other => Err(format!("Инструмента «{}» нет", other)),
    }
}

#[cfg(test)]
#[path = "ai_tools_tests.rs"]
mod tests;
