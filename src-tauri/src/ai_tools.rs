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
//! Все инструменты этой фазы **только читают**.

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
    ]
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
        .map(|i| json!({ "text": i.text, "done": i.is_done }))
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

#[cfg(test)]
#[path = "ai_tools_tests.rs"]
mod tests;
