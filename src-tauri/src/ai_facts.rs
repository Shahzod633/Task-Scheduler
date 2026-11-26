//! Сверка ответа ассистента с числами, которые вернули инструменты.
//!
//! Живые прогоны 26.11.2026 на qwen2.5 7B (`jarvis:latest`) показали три
//! устойчивых промаха пересказа — при верных данных у модели:
//! - «на досках нет открытых задач», хотя `get_boards` отдал `open_cards: 2`
//!   (оба прогона);
//! - «сегодня вы создали 4 задачи» о сводке за 7 дней (оба прогона);
//! - «было 4 попытки» при `retries_requested: 0`.
//!
//! Просьбы в промпте такая модель выполняет через раз (§39), поэтому здесь —
//! проверка в коде. Она намеренно узкая: смотрит только на то, что можно
//! однозначно привязать к полю данных, — число или «нет» рядом со словом
//! «просроч…», «попыт…», «созда…» и т. п. Число, которое ни к чему не
//! привязано («2 доски»), не проверяется, а сомнение трактуется в пользу
//! модели: подходит любое значение этого вида из данных — итог или по
//! отдельной доске.

use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// Что посчитано.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum FactKind {
    /// Открытые (активные) задачи.
    Open,
    Overdue,
    Attention,
    /// События за период сводки.
    Created,
    Completed,
    Retries,
}

impl FactKind {
    fn label(self) -> &'static str {
        match self {
            FactKind::Open => "открытых задач",
            FactKind::Overdue => "просроченных задач",
            FactKind::Attention => "требуют внимания",
            FactKind::Created => "создано задач",
            FactKind::Completed => "завершено задач",
            FactKind::Retries => "запрошено попыток",
        }
    }

    /// Событие за период — к нему относится проверка «сегодня».
    fn is_event(self) -> bool {
        matches!(self, FactKind::Created | FactKind::Completed | FactKind::Retries)
    }
}

/// Числа из результатов инструментов одного хода.
#[derive(Debug, Default, Clone, PartialEq)]
pub(super) struct Facts {
    /// Итог по всему пространству.
    totals: HashMap<FactKind, i64>,
    /// Значения по отдельным доскам — для «на доске X три задачи».
    items: HashMap<FactKind, Vec<i64>>,
    /// Длина периода сводки, если в ходе была сводка.
    period_days: Option<i64>,
    /// Все даты из данных хода и из слов человека, «ГГГГ-ММ-ДД».
    dates: HashSet<String>,
    /// Даты из выполненных изменений (`what_was_done`) — то, что записано на
    /// самом деле.
    written_dates: Vec<String>,
}

/// Даты в тексте, приведённые к «ГГГГ-ММ-ДД», вместе с тем, как они
/// написаны: «2026-11-30» и «30.11.2026». Дата словами («30 ноября») не
/// ищется — её не с чем однозначно сравнить.
pub(super) fn dates_in(text: &str) -> Vec<(String, String)> {
    let b: Vec<char> = text.chars().collect();
    let digit = |i: usize| b.get(i).is_some_and(|c| c.is_ascii_digit());
    let digits = |from: usize, n: usize| (from..from + n).all(digit);
    let boundary = |i: Option<usize>| i.map_or(true, |i| !digit(i));
    let mut found = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let before = boundary(i.checked_sub(1));
        if before && digits(i, 4) && b.get(i + 4) == Some(&'-') && digits(i + 5, 2)
            && b.get(i + 7) == Some(&'-') && digits(i + 8, 2) && boundary(Some(i + 10))
        {
            let raw: String = b[i..i + 10].iter().collect();
            found.push((raw.clone(), raw));
            i += 10;
            continue;
        }
        if before && digits(i, 2) && b.get(i + 2) == Some(&'.') && digits(i + 3, 2)
            && b.get(i + 5) == Some(&'.') && digits(i + 6, 4) && boundary(Some(i + 10))
        {
            let raw: String = b[i..i + 10].iter().collect();
            let iso = format!("{}-{}-{}", &raw[6..10], &raw[3..5], &raw[0..2]);
            found.push((raw, iso));
            i += 10;
            continue;
        }
        i += 1;
    }
    found
}

impl Facts {
    pub(super) fn is_empty(&self) -> bool {
        self.totals.is_empty() && self.items.is_empty() && self.dates.is_empty()
    }

    /// Даты, которые законно появятся в ответе, хотя инструменты их не
    /// возвращали: сегодняшняя и названные человеком.
    pub(super) fn allow_dates_from(&mut self, text: &str) {
        for (_, iso) in dates_in(text) {
            self.dates.insert(iso);
        }
    }

    /// Результат инструмента `tool` (JSON-строкой, как он ушёл модели).
    pub(super) fn add_tool_result(&mut self, tool: &str, result: &str) {
        let Ok(value) = serde_json::from_str::<Value>(result) else { return };
        self.allow_dates_from(result);
        if let Some(done) = value["what_was_done"].as_str() {
            for (_, iso) in dates_in(done) {
                if !self.written_dates.contains(&iso) {
                    self.written_dates.push(iso);
                }
            }
        }
        match tool {
            "get_boards" => {
                let boards = value["boards"].as_array().cloned().unwrap_or_default();
                let (mut open_sum, mut done_sum) = (0, 0);
                for b in &boards {
                    if let Some(n) = b["open_cards"].as_i64() {
                        open_sum += n;
                        self.items.entry(FactKind::Open).or_default().push(n);
                    }
                    if let Some(n) = b["done_cards"].as_i64() {
                        done_sum += n;
                        self.items.entry(FactKind::Completed).or_default().push(n);
                    }
                }
                if !boards.is_empty() {
                    self.totals.insert(FactKind::Open, open_sum);
                    // Не «за период», а всё, что лежит в финальных колонках.
                    self.totals.insert(FactKind::Completed, done_sum);
                }
            }
            "get_overdue_cards" => {
                if let Some(n) = value["count"].as_i64() {
                    self.totals.insert(FactKind::Overdue, n);
                }
            }
            "get_productivity_report" => self.add_report(&value),
            _ => {}
        }
    }

    /// Сводка продуктивности — та же, что уходит в приветствие.
    pub(super) fn add_report(&mut self, report: &Value) {
        self.period_days = report["period_days"].as_i64().or(self.period_days);
        let pairs = [
            (FactKind::Created, &report["during_period"]["created"]),
            (FactKind::Completed, &report["during_period"]["completed"]),
            (FactKind::Retries, &report["during_period"]["retries_requested"]),
            (FactKind::Open, &report["right_now"]["active"]),
            (FactKind::Overdue, &report["right_now"]["overdue"]),
            (FactKind::Attention, &report["right_now"]["needs_attention"]),
        ];
        for (kind, v) in pairs {
            if let Some(n) = v.as_i64() {
                self.totals.insert(kind, n);
            }
        }
        if let Some(boards) = report["top_boards_by_active_cards"].as_array() {
            for b in boards {
                if let Some(n) = b["active_cards"].as_i64() {
                    self.items.entry(FactKind::Open).or_default().push(n);
                }
            }
        }
    }

    fn knows(&self, kind: FactKind) -> bool {
        self.totals.contains_key(&kind) || self.items.contains_key(&kind)
    }

    fn accepts(&self, kind: FactKind, n: i64) -> bool {
        self.totals.get(&kind) == Some(&n) || self.items.get(&kind).is_some_and(|v| v.contains(&n))
    }

    /// Что сказать человеку о настоящем значении.
    fn actual_text(&self, kind: FactKind) -> String {
        match self.totals.get(&kind) {
            Some(n) => n.to_string(),
            None => {
                let v = self.items.get(&kind).cloned().unwrap_or_default();
                v.iter().map(i64::to_string).collect::<Vec<_>>().join(", ")
            }
        }
    }
}

/// Расхождение ответа с данными.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Problem {
    /// «4 попытки» при другом числе в данных; «нет …» — это 0.
    Number { clause: String, kind: FactKind, said: i64 },
    /// «Сегодня» о событиях сводки за несколько дней.
    Period { clause: String, days: i64 },
    /// Дата, которой нет ни в данных, ни в словах человека. `right` — дата из
    /// выполненного изменения, если она одна.
    Date { said: String, right: Option<String> },
}

/// Вид числа по слову рядом с ним. Корни — без окончаний и без «не…»: у
/// «незакрытых» свой корень ниже.
fn kind_of(word: &str) -> Option<FactKind> {
    const STEMS: [(&str, FactKind); 12] = [
        ("попыт", FactKind::Retries),
        ("просроч", FactKind::Overdue),
        ("внимани", FactKind::Attention),
        ("созда", FactKind::Created),
        ("заверш", FactKind::Completed),
        ("выполн", FactKind::Completed),
        ("закрыт", FactKind::Completed),
        ("закрыл", FactKind::Completed),
        ("открыт", FactKind::Open),
        ("активн", FactKind::Open),
        ("незакрыт", FactKind::Open),
        ("незаверш", FactKind::Open),
    ];
    STEMS.iter().find(|(stem, _)| word.starts_with(stem)).map(|(_, k)| *k)
}

const WORD_NUMBERS: [(&str, i64); 14] = [
    ("ноль", 0), ("одна", 1), ("один", 1), ("одну", 1), ("одной", 1), ("две", 2), ("два", 2),
    ("три", 3), ("четыре", 4), ("пять", 5), ("шесть", 6), ("семь", 7), ("восемь", 8), ("десять", 10),
];

/// После этих слов число — дата или длительность, а не количество задач.
const NOT_A_COUNT_AFTER: [&str; 20] = [
    "январ", "феврал", "март", "апрел", "мая", "май", "июн", "июл", "август", "сентябр",
    "октябр", "ноябр", "декабр", "дн", "ден", "недел", "месяц", "час", "минут", "мин",
];

fn number_of(token: &str) -> Option<i64> {
    if !token.is_empty() && token.chars().all(|c| c.is_ascii_digit()) {
        let n: i64 = token.parse().ok()?;
        // Четыре цифры от 1900 — это год, а не количество.
        return if token.len() >= 4 && n >= 1900 { None } else { Some(n) };
    }
    WORD_NUMBERS.iter().find(|(w, _)| *w == token).map(|(_, n)| *n)
}

/// Слова части фразы: в нижнем регистре, без кавычек и скобок. Токен с
/// цифрами и другими знаками («2026-11-25», «10:00», «топ-1») становится
/// пустым — это не количество.
fn words(clause: &str) -> Vec<String> {
    clause
        .split_whitespace()
        .map(|w| {
            let w = w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
            if w.chars().any(|c| c.is_ascii_digit()) && !w.chars().all(|c| c.is_ascii_digit()) {
                String::new()
            } else {
                w
            }
        })
        .collect()
}

/// Части ответа, внутри которых число относится к соседнему слову: фразы и
/// куски между запятыми.
fn clauses(answer: &str) -> Vec<&str> {
    answer
        .split(|c| matches!(c, '.' | '!' | '?' | ';' | ':' | ',' | '\n'))
        .map(str::trim)
        .filter(|c| !c.is_empty())
        .collect()
}

/// Слова, через которые число со словом вида не связывается: «2 задачи
/// просрочены и 1 требует внимания» — единица не о просроченных (живой
/// прогон 26.11 повесил на верное приветствие ложную приписку).
const BOUNDARIES: [&str; 4] = ["и", "а", "но", "или"];

/// Ближайший к позиции `i` вид: до двух слов до числа или до трёх после, не
/// переходя через «и», «а», «но», «или».
fn nearest_kind(ws: &[String], i: usize) -> Option<FactKind> {
    let mut best: Option<(usize, FactKind)> = None;
    let mut consider = |dist: usize, j: usize| {
        if let Some(k) = kind_of(&ws[j]) {
            if best.map_or(true, |(d, _)| dist < d) {
                best = Some((dist, k));
            }
        }
    };
    for d in 1..=2 {
        if i < d || BOUNDARIES.contains(&ws[i - d].as_str()) {
            break;
        }
        consider(d, i - d);
    }
    for d in 1..=3 {
        if i + d >= ws.len() || BOUNDARIES.contains(&ws[i + d].as_str()) {
            break;
        }
        consider(d, i + d);
    }
    best.map(|(_, k)| k)
}

/// Расхождения ответа с данными хода. Пусто — расхождений не нашлось или
/// проверять не с чем.
pub(super) fn check(answer: &str, facts: &Facts) -> Vec<Problem> {
    if facts.is_empty() {
        return Vec::new();
    }
    let mut problems = Vec::new();
    for clause in clauses(answer) {
        let ws = words(clause);

        // «Сегодня» о событиях многодневной сводки.
        if let Some(days) = facts.period_days.filter(|d| *d > 1) {
            if ws.iter().any(|w| w == "сегодня") && ws.iter().any(|w| kind_of(w).is_some_and(FactKind::is_event)) {
                problems.push(Problem::Period { clause: clause.to_string(), days });
            }
        }

        // Числа рядом со словом вида.
        for (i, w) in ws.iter().enumerate() {
            let Some(n) = number_of(w) else { continue };
            if ws.get(i + 1).is_some_and(|next| NOT_A_COUNT_AFTER.iter().any(|s| next.starts_with(s))) {
                continue;
            }
            let Some(kind) = nearest_kind(&ws, i) else { continue };
            if facts.knows(kind) && !facts.accepts(kind, n) {
                problems.push(Problem::Number { clause: clause.to_string(), kind, said: n });
            }
        }

        // «Нет открытых задач», «ни одной просроченной» — это ноль. Сверяется
        // только с итогом: на отдельной доске ноль бывает законно. Если в
        // части фразы названа доска (кавычки), речь о ней — не проверяем.
        let says_none = ws.iter().any(|w| w == "нет") || clause.to_lowercase().contains("ни одн");
        let names_something = clause.contains('«') || clause.contains('"');
        if says_none && !names_something {
            if let Some(kind) = ws.iter().find_map(|w| kind_of(w)) {
                if facts.totals.get(&kind).is_some_and(|n| *n > 0) {
                    problems.push(Problem::Number { clause: clause.to_string(), kind, said: 0 });
                }
            }
        }
    }
    // Даты. Живой прогон 26.11: задача записана со сроком 30.11, а модель
    // ответила «срок до понедельника 2026-12-03» — свою первую, неверную
    // дату, хотя инструмент вернул ей 30.11.2026.
    if !facts.dates.is_empty() {
        for (raw, iso) in dates_in(answer) {
            if !facts.dates.contains(&iso) {
                let right = (facts.written_dates.len() == 1).then(|| facts.written_dates[0].clone());
                problems.push(Problem::Date { said: raw, right });
            }
        }
    }
    problems.dedup();
    problems
}

/// «2026-11-30» → «30.11.2026».
fn date_ru(iso: &str) -> String {
    format!("{}.{}.{}", &iso[8..10], &iso[5..7], &iso[0..4])
}

/// Напоминание модели: что именно не сходится.
pub(super) fn nudge(problems: &[Problem], facts: &Facts) -> String {
    let lines: Vec<String> = problems
        .iter()
        .map(|p| match p {
            Problem::Number { clause, kind, .. } => {
                format!("- «{}» — а по данным {}: {}", clause, kind.label(), facts.actual_text(*kind))
            }
            Problem::Period { clause, days } => {
                format!("- «{}» — это сводка за последние {} дней, а не за сегодня", clause, days)
            }
            Problem::Date { said, right } => match right {
                Some(r) => format!("- дата «{}» — такой нет в данных; на самом деле записано {}", said, date_ru(r)),
                None => format!("- дата «{}» — такой нет в данных инструментов", said),
            },
        })
        .collect();
    format!(
        "Проверка приложения: ответ расходится с данными инструментов:\n{}\n\
         Напиши ответ заново и называй только те числа, что вернули инструменты.",
        lines.join("\n")
    )
}

/// Последний рубеж, если и повтор не сошёлся с данными: «сегодня» в
/// проблемной части фразы заменяется периодом сводки, а к неверным числам
/// приписывается, сколько на самом деле.
pub(super) fn correct(answer: &str, problems: &[Problem], facts: &Facts) -> String {
    let mut text = answer.to_string();
    let mut notes: Vec<String> = Vec::new();
    for p in problems {
        match p {
            Problem::Period { clause, days } => {
                let fixed = clause
                    .replacen("Сегодня", &format!("За последние {} дней", days), 1)
                    .replacen("сегодня", &format!("за последние {} дней", days), 1);
                text = text.replacen(clause.as_str(), &fixed, 1);
            }
            Problem::Number { kind, .. } => {
                let note = format!("{} — {}", kind.label(), facts.actual_text(*kind));
                if !notes.contains(&note) {
                    notes.push(note);
                }
            }
            Problem::Date { said, right: Some(r) } => {
                text = text.replace(said.as_str(), &date_ru(r));
            }
            Problem::Date { said, right: None } => {
                let note = format!("даты {} в данных нет", said);
                if !notes.contains(&note) {
                    notes.push(note);
                }
            }
        }
    }
    if !notes.is_empty() {
        text.push_str(&format!("\n\n⚠ Приложение: по данным {}.", notes.join("; ")));
    }
    text
}

#[cfg(test)]
#[path = "ai_facts_tests.rs"]
mod tests;
