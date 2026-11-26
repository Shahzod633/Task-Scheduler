//! Трекер привычек: привычки, отметки за день, стрики, тепловая карта,
//! сводка «задачи + привычки» по дням и ИИ-анализ нагрузки.
//!
//! Модуль — дочерний к `commands.rs`, как `ai_tools`: даты считаются теми же
//! `days_from_civil`/`parse_day`, настройки и запрос к Ollama — теми же
//! `read_ai_settings`/`run_blocking`, что у ассистента, без копий.
//!
//! ## Стрик
//!
//! Стрик — производная от `habit_logs`, а не счётчик, который правят по
//! событию: пересчитывается целиком при каждой отметке и при чтении списка.
//! Иначе он врал бы в двух случаях — когда отметку исправили задним числом и
//! когда человек просто не открывал приложение несколько дней.
//!
//! Единица стрика — день по расписанию (`daily`, `weekly_days`) или неделя,
//! в которую набрана норма (`x_per_week`). Для каждой единицы:
//! - `completed` и `fallback` — стрик растёт;
//! - `skipped` — стрик заморожен: не растёт и не рвётся;
//! - `failed` — стрик сбрасывается в 0;
//! - отметки нет, а день прошёл — тоже сброс (иначе стрик рвался бы только
//!   у тех, кто честно жмёт «Сорвалось»); сегодняшний день без отметки —
//!   ещё не прошёл и ничего не решает.
//!
//! У `x_per_week` пропуски уменьшают норму недели: при норме 3 и одном
//! «пропущено» неделе хватит двух выполнений. Неделя, где всё пропущено, —
//! заморожена. Неполная первая неделя (привычку завели в четверг) требует не
//! больше дней, чем в ней осталось.

use super::*;
use crate::models::{
    DailyLoad, Habit, HabitAdjustment, HabitAnalysis, HabitAnalysisReport, HabitCorrelation,
    HabitHeatmapDay, HabitInput, HabitLog, HabitMark, HABIT_FREQUENCIES, HABIT_LOG_STATUSES,
    HABIT_TIMES_OF_DAY, WEEKDAY_KEYS,
};
use serde_json::{json, Value};

const MAX_HABIT_TITLE: usize = 120;
const MAX_HABIT_DESCRIPTION: usize = 1000;
const MAX_SKIP_REASON: usize = 300;

/// Тепловая карта — до 53 недель: больше года на экран не помещается.
pub const HEATMAP_MAX_DAYS: i64 = 371;

/// Период сводки для ИИ-анализа (задание: «за последние 14 дней»).
pub const ANALYSIS_DAYS: i64 = 14;

/// Больше пунктов модель всё равно не обоснует цифрами, а читать их некому.
const MAX_ANALYSIS_ITEMS: usize = 6;

pub const ERR_HABIT_NOT_FOUND: &str = "Привычка не найдена";
pub const ERR_HABIT_TITLE_EMPTY: &str = "Укажите название привычки";
pub const ERR_HABIT_TITLE_LONG: &str = "Название длиннее 120 символов";
pub const ERR_HABIT_DESCRIPTION_LONG: &str = "Описание длиннее 1000 символов";
pub const ERR_HABIT_FREQUENCY: &str = "Неизвестная периодичность привычки";
pub const ERR_HABIT_WEEKDAYS: &str = "Выберите хотя бы один день недели";
pub const ERR_HABIT_WEEKDAY_UNKNOWN: &str = "Неизвестный день недели";
pub const ERR_HABIT_TARGET: &str = "Сколько раз в неделю — от 1 до 7";
pub const ERR_HABIT_TIME: &str = "Неизвестное время суток";
pub const ERR_HABIT_EFFORT: &str = "Нагрузка — от 1 до 5";
pub const ERR_HABIT_STATUS: &str = "Неизвестный статус отметки";
pub const ERR_HABIT_DATE: &str = "Дата отметки должна быть в виде ГГГГ-ММ-ДД";
pub const ERR_HABIT_DATE_FUTURE: &str = "Нельзя отметить привычку на будущий день";
pub const ERR_HABIT_ARCHIVED: &str = "Привычка в архиве — верните её, чтобы отмечать";
pub const ERR_HABIT_NO_FALLBACK: &str = "У этой привычки нет лайт-версии";
pub const ERR_SKIP_REASON_LONG: &str = "Причина длиннее 300 символов";
pub const ERR_ANALYSIS_NO_HABITS: &str = "Для анализа нужна хотя бы одна активная привычка";
pub const ERR_ANALYSIS_NO_LOGS: &str =
    "За последние 14 дней нет ни одной отметки привычек — анализировать пока нечего";
pub const ERR_ANALYSIS_BAD_ANSWER: &str =
    "Модель ответила не по формату анализа — попробуйте ещё раз или выберите другую модель";

// ─── Расписание и даты ───

#[derive(Debug, Clone, PartialEq)]
pub(super) enum Frequency {
    Daily,
    /// Дни недели с понедельника.
    WeeklyDays([bool; 7]),
    XPerWeek(i64),
}

impl Frequency {
    /// Из того, что лежит в базе. Записи туда проверены `normalize_input`;
    /// испорченная руками строка читается как «ежедневно», а не роняет экран.
    pub(super) fn from_db(kind: &str, config: Option<&str>) -> Frequency {
        match kind {
            "weekly_days" => {
                let days: Vec<String> = config.and_then(|c| serde_json::from_str(c).ok()).unwrap_or_default();
                let mut mask = [false; 7];
                for day in &days {
                    if let Some(i) = weekday_index(day) {
                        mask[i] = true;
                    }
                }
                if mask.iter().any(|on| *on) { Frequency::WeeklyDays(mask) } else { Frequency::Daily }
            }
            "x_per_week" => {
                let target = config
                    .and_then(|c| serde_json::from_str::<Value>(c).ok())
                    .and_then(|v| v.get("target").and_then(Value::as_i64))
                    .unwrap_or(1)
                    .clamp(1, 7);
                Frequency::XPerWeek(target)
            }
            _ => Frequency::Daily,
        }
    }

    /// День по расписанию. У `x_per_week` расписания по дням нет.
    pub(super) fn is_scheduled(&self, day: i64) -> bool {
        match self {
            Frequency::Daily => true,
            Frequency::WeeklyDays(mask) => mask[weekday_of(day)],
            Frequency::XPerWeek(_) => false,
        }
    }

    fn streak_unit(&self) -> &'static str {
        if matches!(self, Frequency::XPerWeek(_)) { "week" } else { "day" }
    }
}

/// День недели номера дня от 1970-01-01, понедельник — 0. 1970-01-01 был
/// четвергом.
pub(super) fn weekday_of(day: i64) -> usize {
    (day + 3).rem_euclid(7) as usize
}

/// Понедельник недели, в которую попадает день.
pub(super) fn week_start(day: i64) -> i64 {
    day - weekday_of(day) as i64
}

fn weekday_index(key: &str) -> Option<usize> {
    WEEKDAY_KEYS.iter().position(|k| *k == key)
}

pub(super) fn day_number(s: &str) -> Option<i64> {
    parse_day(s).map(|(y, m, d)| days_from_civil(y, m, d))
}

pub(super) fn day_string(n: i64) -> String {
    format_day(civil_from_days(n))
}

fn today_number(conn: &rusqlite::Connection) -> CmdResult<i64> {
    let today: String = conn
        .query_row("SELECT date('now', 'localtime')", [], |r| r.get(0))
        .map_err(to_string_err)?;
    day_number(&today).ok_or_else(|| format!("Не удалось разобрать сегодняшнюю дату «{}»", today))
}

fn is_done(status: &str) -> bool {
    status == "completed" || status == "fallback"
}

// ─── Стрик ───

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Unit {
    Success,
    Frozen,
    Broken,
    /// Единица ещё идёт (сегодня, текущая неделя) и ничего не решает.
    Pending,
}

/// Единицы стрика от `start` до `today` включительно. `logs` — статус
/// отметки по номеру дня.
pub(super) fn streak_units(freq: &Frequency, start: i64, today: i64, logs: &HashMap<i64, String>) -> Vec<Unit> {
    if start > today {
        return Vec::new();
    }
    match freq {
        Frequency::XPerWeek(target) => {
            let mut units = Vec::new();
            let mut week = week_start(start);
            while week <= today {
                let first = week.max(start);
                let last = week + 6;
                let (mut done, mut skipped) = (0i64, 0i64);
                for day in first..=last.min(today) {
                    match logs.get(&day).map(String::as_str) {
                        Some(s) if is_done(s) => done += 1,
                        Some("skipped") => skipped += 1,
                        _ => {}
                    }
                }
                let available = last - first + 1;
                let needed = ((*target).min(available) - skipped).max(0);
                units.push(if done > 0 && done >= needed {
                    Unit::Success
                } else if needed == 0 {
                    Unit::Frozen
                } else if last >= today {
                    Unit::Pending
                } else {
                    Unit::Broken
                });
                week += 7;
            }
            units
        }
        _ => (start..=today)
            .filter(|day| freq.is_scheduled(*day))
            .map(|day| match logs.get(&day).map(String::as_str) {
                Some(s) if is_done(s) => Unit::Success,
                Some("skipped") => Unit::Frozen,
                Some(_) => Unit::Broken,
                None if day == today => Unit::Pending,
                None => Unit::Broken,
            })
            .collect(),
    }
}

/// (текущий, самый длинный). Текущий — серия успехов в конце, сквозь
/// замороженные единицы; самый длинный — лучшая такая серия за всё время.
pub(super) fn streaks_from_units(units: &[Unit]) -> (i64, i64) {
    let (mut run, mut longest) = (0i64, 0i64);
    for unit in units {
        match unit {
            Unit::Success => {
                run += 1;
                longest = longest.max(run);
            }
            Unit::Broken => run = 0,
            Unit::Frozen | Unit::Pending => {}
        }
    }
    (run, longest)
}

// ─── Проверка формы ───

pub(super) struct NormalizedHabit {
    pub title: String,
    pub description: Option<String>,
    pub frequency_type: &'static str,
    pub frequency_config: Option<String>,
    pub time_of_day: &'static str,
    pub effort_level: i64,
    pub fallback_title: Option<String>,
}

fn trimmed_or_none(value: &Option<String>) -> Option<String> {
    value.as_deref().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
}

pub(super) fn normalize_input(input: &HabitInput) -> CmdResult<NormalizedHabit> {
    let title = input.title.trim().to_string();
    if title.is_empty() {
        return Err(ERR_HABIT_TITLE_EMPTY.into());
    }
    if title.chars().count() > MAX_HABIT_TITLE {
        return Err(ERR_HABIT_TITLE_LONG.into());
    }
    let description = trimmed_or_none(&input.description);
    if description.as_ref().is_some_and(|d| d.chars().count() > MAX_HABIT_DESCRIPTION) {
        return Err(ERR_HABIT_DESCRIPTION_LONG.into());
    }
    let fallback_title = trimmed_or_none(&input.fallback_title);
    if fallback_title.as_ref().is_some_and(|f| f.chars().count() > MAX_HABIT_TITLE) {
        return Err(ERR_HABIT_TITLE_LONG.into());
    }

    let frequency_type = HABIT_FREQUENCIES
        .iter()
        .copied()
        .find(|f| *f == input.frequency_type)
        .ok_or_else(|| ERR_HABIT_FREQUENCY.to_string())?;
    let frequency_config = match frequency_type {
        "weekly_days" => {
            let mut mask = [false; 7];
            for day in &input.weekdays {
                let i = weekday_index(day.trim()).ok_or_else(|| ERR_HABIT_WEEKDAY_UNKNOWN.to_string())?;
                mask[i] = true;
            }
            // Порядок недели и без повторов — как бы их ни прислали.
            let days: Vec<&str> = WEEKDAY_KEYS.iter().zip(mask).filter(|(_, on)| *on).map(|(k, _)| *k).collect();
            if days.is_empty() {
                return Err(ERR_HABIT_WEEKDAYS.into());
            }
            Some(serde_json::to_string(&days).map_err(to_string_err)?)
        }
        "x_per_week" => {
            let target = input.target_per_week.ok_or_else(|| ERR_HABIT_TARGET.to_string())?;
            if !(1..=7).contains(&target) {
                return Err(ERR_HABIT_TARGET.into());
            }
            Some(json!({ "target": target }).to_string())
        }
        _ => None,
    };

    let time_of_day = HABIT_TIMES_OF_DAY
        .iter()
        .copied()
        .find(|t| *t == input.time_of_day)
        .ok_or_else(|| ERR_HABIT_TIME.to_string())?;
    if !(1..=5).contains(&input.effort_level) {
        return Err(ERR_HABIT_EFFORT.into());
    }

    Ok(NormalizedHabit {
        title,
        description,
        frequency_type,
        frequency_config,
        time_of_day,
        effort_level: input.effort_level,
        fallback_title,
    })
}

// ─── Чтение ───

const HABIT_SELECT: &str = "SELECT id, title, COALESCE(description, ''), frequency_type, frequency_config,
        time_of_day, effort_level, fallback_title, current_streak, longest_streak, is_archived,
        created_at, date(created_at, 'localtime')
   FROM habits";

/// Привычка без того, что считается на сегодня, плюс расписание и местный
/// день создания — их дозаполняет `refresh_habit`.
fn row_to_habit(row: &rusqlite::Row) -> rusqlite::Result<(Habit, Frequency, String)> {
    let kind: String = row.get(3)?;
    let config: Option<String> = row.get(4)?;
    let freq = Frequency::from_db(&kind, config.as_deref());
    let weekdays = match &freq {
        Frequency::WeeklyDays(mask) => WEEKDAY_KEYS
            .iter()
            .zip(mask)
            .filter(|(_, on)| **on)
            .map(|(k, _)| k.to_string())
            .collect(),
        _ => Vec::new(),
    };
    let target_per_week = match freq {
        Frequency::XPerWeek(t) => Some(t),
        _ => None,
    };
    let archived: i64 = row.get(10)?;
    let created_day: Option<String> = row.get(12)?;
    let habit = Habit {
        id: row.get(0)?,
        title: row.get(1)?,
        description: row.get(2)?,
        frequency_type: kind,
        weekdays,
        target_per_week,
        time_of_day: row.get(5)?,
        effort_level: row.get(6)?,
        fallback_title: row.get(7)?,
        current_streak: row.get(8)?,
        longest_streak: row.get(9)?,
        is_archived: archived != 0,
        created_at: row.get::<_, Option<String>>(11)?.unwrap_or_default(),
        streak_unit: freq.streak_unit().to_string(),
        due_today: false,
        today: None,
        week_done: 0,
    };
    Ok((habit, freq, created_day.unwrap_or_default()))
}

fn row_to_log(row: &rusqlite::Row) -> rusqlite::Result<HabitLog> {
    Ok(HabitLog {
        id: row.get(0)?,
        habit_id: row.get(1)?,
        log_date: row.get(2)?,
        status: row.get(3)?,
        skip_reason: row.get(4)?,
        completed_at: row.get(5)?,
    })
}

const LOG_SELECT: &str = "SELECT id, habit_id, log_date, status, skip_reason, completed_at FROM habit_logs";

fn read_log(conn: &rusqlite::Connection, habit_id: i64, day: &str) -> CmdResult<Option<HabitLog>> {
    conn.query_row(
        &format!("{} WHERE habit_id = ?1 AND log_date = ?2", LOG_SELECT),
        params![habit_id, day],
        row_to_log,
    )
    .optional()
    .map_err(to_string_err)
}

/// Все отметки привычки: номер дня → статус.
fn log_map(conn: &rusqlite::Connection, habit_id: i64) -> CmdResult<HashMap<i64, String>> {
    let mut stmt = conn
        .prepare("SELECT log_date, status FROM habit_logs WHERE habit_id = ?1")
        .map_err(to_string_err)?;
    let rows = stmt
        .query_map(params![habit_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(to_string_err)?;
    let mut map = HashMap::new();
    for row in rows {
        let (day, status) = row.map_err(to_string_err)?;
        if let Some(n) = day_number(&day) {
            map.insert(n, status);
        }
    }
    Ok(map)
}

/// Первый день, с которого привычка в счёте: день создания или самая ранняя
/// отметка, если её вписали задним числом.
fn start_day(created_day: &str, logs: &HashMap<i64, String>, today: i64) -> i64 {
    let created = day_number(created_day).unwrap_or(today);
    logs.keys().copied().min().map_or(created, |earliest| earliest.min(created))
}

/// Дозаполняет привычку на сегодня и пересчитывает стрик; изменившийся стрик
/// записывает в базу. У архивной стрик не трогается — архив работает как
/// пауза, и пока привычка лежит там, пропущенные дни её не обнуляют.
fn refresh_habit(
    conn: &rusqlite::Connection,
    mut habit: Habit,
    freq: &Frequency,
    created_day: &str,
    today: i64,
) -> CmdResult<Habit> {
    let logs = log_map(conn, habit.id)?;
    if !habit.is_archived {
        let units = streak_units(freq, start_day(created_day, &logs, today), today, &logs);
        let (current, longest) = streaks_from_units(&units);
        if current != habit.current_streak || longest != habit.longest_streak {
            conn.execute(
                "UPDATE habits SET current_streak = ?1, longest_streak = ?2 WHERE id = ?3",
                params![current, longest, habit.id],
            )
            .map_err(to_string_err)?;
            habit.current_streak = current;
            habit.longest_streak = longest;
        }
    }
    habit.week_done = (week_start(today)..=today)
        .filter(|day| logs.get(day).is_some_and(|s| is_done(s)))
        .count() as i64;
    habit.today = read_log(conn, habit.id, &day_string(today))?;
    habit.due_today = match freq {
        Frequency::XPerWeek(target) => habit.week_done < *target || habit.today.is_some(),
        other => other.is_scheduled(today),
    };
    Ok(habit)
}

pub(super) fn read_habit(conn: &rusqlite::Connection, id: i64) -> CmdResult<Habit> {
    let (habit, freq, created_day) = conn
        .query_row(&format!("{} WHERE id = ?1", HABIT_SELECT), params![id], row_to_habit)
        .optional()
        .map_err(to_string_err)?
        .ok_or_else(|| ERR_HABIT_NOT_FOUND.to_string())?;
    refresh_habit(conn, habit, &freq, &created_day, today_number(conn)?)
}

pub(super) fn list_habits_in(conn: &rusqlite::Connection, include_archived: bool) -> CmdResult<Vec<Habit>> {
    let today = today_number(conn)?;
    let sql = format!(
        "{} WHERE (?1 OR is_archived = 0)
          ORDER BY is_archived,
                   CASE time_of_day WHEN 'morning' THEN 0 WHEN 'afternoon' THEN 1
                                    WHEN 'evening' THEN 2 ELSE 3 END,
                   id",
        HABIT_SELECT
    );
    let rows: Vec<(Habit, Frequency, String)> = {
        let mut stmt = conn.prepare(&sql).map_err(to_string_err)?;
        let mapped = stmt.query_map(params![include_archived], row_to_habit).map_err(to_string_err)?;
        mapped.collect::<rusqlite::Result<_>>().map_err(to_string_err)?
    };
    rows.into_iter()
        .map(|(habit, freq, created_day)| refresh_habit(conn, habit, &freq, &created_day, today))
        .collect()
}

// ─── Команды: привычки ───

#[tauri::command]
pub fn list_habits(include_archived: bool, state: State<'_, DbState>) -> CmdResult<Vec<Habit>> {
    let conn = state.conn.lock().unwrap();
    list_habits_in(&conn, include_archived)
}

#[tauri::command]
pub fn create_habit(input: HabitInput, state: State<'_, DbState>) -> CmdResult<Habit> {
    let conn = state.conn.lock().unwrap();
    create_habit_in(&conn, &input)
}

pub(super) fn create_habit_in(conn: &rusqlite::Connection, input: &HabitInput) -> CmdResult<Habit> {
    let h = normalize_input(input)?;
    conn.execute(
        "INSERT INTO habits (title, description, frequency_type, frequency_config, time_of_day,
                             effort_level, fallback_title)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![h.title, h.description, h.frequency_type, h.frequency_config, h.time_of_day, h.effort_level, h.fallback_title],
    )
    .map_err(to_string_err)?;
    read_habit(conn, conn.last_insert_rowid())
}

#[tauri::command]
pub fn update_habit(id: i64, input: HabitInput, state: State<'_, DbState>) -> CmdResult<Habit> {
    let conn = state.conn.lock().unwrap();
    update_habit_in(&conn, id, &input)
}

/// Смена расписания пересчитывает стрик по старым отметкам: «ежедневно»,
/// ставшее «3 раза в неделю», меряется уже неделями.
pub(super) fn update_habit_in(conn: &rusqlite::Connection, id: i64, input: &HabitInput) -> CmdResult<Habit> {
    let h = normalize_input(input)?;
    let changed = conn
        .execute(
            "UPDATE habits
                SET title = ?1, description = ?2, frequency_type = ?3, frequency_config = ?4,
                    time_of_day = ?5, effort_level = ?6, fallback_title = ?7
              WHERE id = ?8",
            params![h.title, h.description, h.frequency_type, h.frequency_config, h.time_of_day, h.effort_level, h.fallback_title, id],
        )
        .map_err(to_string_err)?;
    if changed == 0 {
        return Err(ERR_HABIT_NOT_FOUND.into());
    }
    read_habit(conn, id)
}

#[tauri::command]
pub fn set_habit_archived(id: i64, archived: bool, state: State<'_, DbState>) -> CmdResult<Habit> {
    let conn = state.conn.lock().unwrap();
    set_habit_archived_in(&conn, id, archived)
}

pub(super) fn set_habit_archived_in(conn: &rusqlite::Connection, id: i64, archived: bool) -> CmdResult<Habit> {
    let changed = conn
        .execute("UPDATE habits SET is_archived = ?1 WHERE id = ?2", params![archived, id])
        .map_err(to_string_err)?;
    if changed == 0 {
        return Err(ERR_HABIT_NOT_FOUND.into());
    }
    read_habit(conn, id)
}

/// Удаляет привычку вместе с отметками — их снимает каскад `habit_logs`.
#[tauri::command]
pub fn delete_habit(id: i64, state: State<'_, DbState>) -> CmdResult<()> {
    let conn = state.conn.lock().unwrap();
    delete_habit_in(&conn, id)
}

pub(super) fn delete_habit_in(conn: &rusqlite::Connection, id: i64) -> CmdResult<()> {
    let changed = conn.execute("DELETE FROM habits WHERE id = ?1", params![id]).map_err(to_string_err)?;
    if changed == 0 {
        return Err(ERR_HABIT_NOT_FOUND.into());
    }
    Ok(())
}

// ─── Команды: отметки ───

/// Отметка за день: одна на привычку и день, повторная заменяет прежнюю.
/// Причина хранится только у «пропущено» и «сорвалось». Будущий день
/// отвергается; прошедший — можно, отметка задним числом законна.
#[tauri::command]
pub fn log_habit(
    habit_id: i64,
    log_date: String,
    status: String,
    skip_reason: Option<String>,
    state: State<'_, DbState>,
) -> CmdResult<Habit> {
    let conn = state.conn.lock().unwrap();
    log_habit_in(&conn, habit_id, &log_date, &status, skip_reason)
}

pub(super) fn log_habit_in(
    conn: &rusqlite::Connection,
    habit_id: i64,
    log_date: &str,
    status: &str,
    skip_reason: Option<String>,
) -> CmdResult<Habit> {
    let habit = read_habit(conn, habit_id)?;
    if habit.is_archived {
        return Err(ERR_HABIT_ARCHIVED.into());
    }
    let day = day_number(log_date).ok_or_else(|| ERR_HABIT_DATE.to_string())?;
    if day > today_number(conn)? {
        return Err(ERR_HABIT_DATE_FUTURE.into());
    }
    let status = HABIT_LOG_STATUSES
        .iter()
        .copied()
        .find(|s| *s == status)
        .ok_or_else(|| ERR_HABIT_STATUS.to_string())?;
    if status == "fallback" && habit.fallback_title.is_none() {
        return Err(ERR_HABIT_NO_FALLBACK.into());
    }
    let reason = if is_done(status) { None } else { trimmed_or_none(&skip_reason) };
    if reason.as_ref().is_some_and(|r| r.chars().count() > MAX_SKIP_REASON) {
        return Err(ERR_SKIP_REASON_LONG.into());
    }
    conn.execute(
        "INSERT INTO habit_logs (habit_id, log_date, status, skip_reason, completed_at)
         VALUES (?1, ?2, ?3, ?4, CASE WHEN ?5 THEN datetime('now') END)
         ON CONFLICT(habit_id, log_date) DO UPDATE
            SET status = excluded.status,
                skip_reason = excluded.skip_reason,
                completed_at = excluded.completed_at",
        params![habit_id, log_date, status, reason, is_done(status)],
    )
    .map_err(to_string_err)?;
    read_habit(conn, habit_id)
}

/// Снимает отметку за день — «отменить» в виджете.
#[tauri::command]
pub fn clear_habit_log(habit_id: i64, log_date: String, state: State<'_, DbState>) -> CmdResult<Habit> {
    let conn = state.conn.lock().unwrap();
    clear_habit_log_in(&conn, habit_id, &log_date)
}

pub(super) fn clear_habit_log_in(conn: &rusqlite::Connection, habit_id: i64, log_date: &str) -> CmdResult<Habit> {
    let habit = read_habit(conn, habit_id)?;
    if habit.is_archived {
        return Err(ERR_HABIT_ARCHIVED.into());
    }
    conn.execute(
        "DELETE FROM habit_logs WHERE habit_id = ?1 AND log_date = ?2",
        params![habit_id, log_date],
    )
    .map_err(to_string_err)?;
    read_habit(conn, habit_id)
}

// ─── Тепловая карта ───

/// Что нужно о привычке для подсчётов по дням.
struct HabitMeta {
    id: i64,
    title: String,
    freq: Frequency,
    start: i64,
}

/// Привычки для подсчёта: одна (`Some`, хоть из архива) или все активные.
fn habit_metas(conn: &rusqlite::Connection, habit_id: Option<i64>, today: i64) -> CmdResult<Vec<HabitMeta>> {
    let mut stmt = conn
        .prepare(
            "SELECT h.id, h.title, h.frequency_type, h.frequency_config,
                    MIN(date(h.created_at, 'localtime'),
                        COALESCE((SELECT MIN(l.log_date) FROM habit_logs l WHERE l.habit_id = h.id), '9999-12-31'))
               FROM habits h
              WHERE (?1 IS NULL AND h.is_archived = 0) OR h.id = ?1
              ORDER BY h.id",
        )
        .map_err(to_string_err)?;
    let rows = stmt
        .query_map(params![habit_id], |r| {
            let kind: String = r.get(2)?;
            let config: Option<String> = r.get(3)?;
            let start: Option<String> = r.get(4)?;
            Ok(HabitMeta {
                id: r.get(0)?,
                title: r.get(1)?,
                freq: Frequency::from_db(&kind, config.as_deref()),
                start: start.as_deref().and_then(day_number).unwrap_or(today),
            })
        })
        .map_err(to_string_err)?;
    rows.collect::<rusqlite::Result<_>>().map_err(to_string_err)
}

/// Отметки за период: (привычка, день) → (статус, причина).
fn logs_between(
    conn: &rusqlite::Connection,
    from: i64,
    to: i64,
) -> CmdResult<HashMap<(i64, i64), (String, Option<String>)>> {
    let mut stmt = conn
        .prepare("SELECT habit_id, log_date, status, skip_reason FROM habit_logs WHERE log_date BETWEEN ?1 AND ?2")
        .map_err(to_string_err)?;
    let rows = stmt
        .query_map(params![day_string(from), day_string(to)], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, r.get::<_, Option<String>>(3)?))
        })
        .map_err(to_string_err)?;
    let mut map = HashMap::new();
    for row in rows {
        let (habit_id, day, status, reason) = row.map_err(to_string_err)?;
        if let Some(n) = day_number(&day) {
            map.insert((habit_id, n), (status, reason));
        }
    }
    Ok(map)
}

/// Тепловая карта за последние `days` дней, старые первыми: одна привычка
/// или все активные.
#[tauri::command]
pub fn get_habit_heatmap(habit_id: Option<i64>, days: i64, state: State<'_, DbState>) -> CmdResult<Vec<HabitHeatmapDay>> {
    let conn = state.conn.lock().unwrap();
    heatmap_in(&conn, habit_id, days)
}

pub(super) fn heatmap_in(conn: &rusqlite::Connection, habit_id: Option<i64>, days: i64) -> CmdResult<Vec<HabitHeatmapDay>> {
    let days = days.clamp(1, HEATMAP_MAX_DAYS);
    let today = today_number(conn)?;
    let from = today - days + 1;
    let metas = habit_metas(conn, habit_id, today)?;
    if habit_id.is_some() && metas.is_empty() {
        return Err(ERR_HABIT_NOT_FOUND.into());
    }
    let logs = logs_between(conn, from, today)?;

    Ok((from..=today)
        .map(|day| {
            let mut cell = HabitHeatmapDay {
                date: day_string(day),
                due: 0,
                completed: 0,
                fallback: 0,
                skipped: 0,
                failed: 0,
            };
            for meta in &metas {
                let status = logs.get(&(meta.id, day)).map(|(s, _)| s.as_str());
                let scheduled = day >= meta.start && meta.freq.is_scheduled(day);
                if scheduled || status.is_some_and(is_done) {
                    cell.due += 1;
                }
                match status {
                    Some("completed") => cell.completed += 1,
                    Some("fallback") => cell.fallback += 1,
                    Some("skipped") => cell.skipped += 1,
                    Some("failed") => cell.failed += 1,
                    _ => {}
                }
            }
            cell
        })
        .collect())
}

// ─── Сводка «задачи + привычки» по дням ───

/// Задачи и отметки привычек одним запросом, по дню за строку.
///
/// Задачи — со всех досок неархивных пространств, кроме архивных досок
/// (нагрузка у человека одна на все проекты). По дню:
/// - `tasks_due` — срок на этот день;
/// - `tasks_completed` — попала в финальную колонку в этот день
///   (`completed_at`, местная дата);
/// - `tasks_overdue` — срок на этот день, а к его концу не закрыта (закрыта
///   позже или не закрыта вовсе). Сегодняшний день ещё не кончился и
///   просрочек не даёт. Закрытые до появления `completed_at` (финальная
///   колонка, даты нет) — не в счёт: когда их закрыли, неизвестно;
/// - `tracked_seconds` — учтённое время сессий, начатых в этот день.
///
/// Отметки — только активных привычек.
const DAILY_LOAD_SQL: &str = "
WITH RECURSIVE days(day) AS (
    SELECT ?1
    UNION ALL
    SELECT date(day, '+1 day') FROM days WHERE day < ?2
),
live_cards AS (
    SELECT c.id AS card_id,
           substr(c.due_date, 1, 10) AS due_day,
           date(c.completed_at, 'localtime') AS done_day,
           col.is_final AS is_final
      FROM cards c
      JOIN columns col ON col.id = c.column_id
      JOIN boards b ON b.id = col.board_id
      JOIN workspaces w ON w.id = b.workspace_id
     WHERE b.archived = 0 AND w.archived = 0
)
SELECT d.day,
       (SELECT COUNT(*) FROM live_cards lc WHERE lc.due_day = d.day),
       (SELECT COUNT(*) FROM live_cards lc WHERE lc.done_day = d.day),
       (SELECT COUNT(*) FROM live_cards lc
         WHERE lc.due_day = d.day AND d.day < ?3
           AND (lc.done_day IS NULL OR lc.done_day > lc.due_day)
           AND NOT (lc.is_final = 1 AND lc.done_day IS NULL)),
       (SELECT COALESCE(SUM(t.duration_seconds), 0)
          FROM time_entries t JOIN live_cards lc ON lc.card_id = t.card_id
         WHERE date(t.started_at, 'localtime') = d.day AND t.duration_seconds IS NOT NULL),
       COALESCE(SUM(l.status = 'completed'), 0),
       COALESCE(SUM(l.status = 'fallback'), 0),
       COALESCE(SUM(l.status = 'skipped'), 0),
       COALESCE(SUM(l.status = 'failed'), 0)
  FROM days d
  LEFT JOIN habit_logs l
         ON l.log_date = d.day
        AND l.habit_id IN (SELECT id FROM habits WHERE is_archived = 0)
 GROUP BY d.day
 ORDER BY d.day";

/// Сводка за последние `days` дней, включая сегодня. К тому, что дал запрос,
/// добавляется то, что в SQL не выразить без расписания: сколько привычек
/// стояло на день, сколько из них пропущено без отметки, и отметки по
/// каждой привычке с причинами.
pub(super) fn daily_load_in(conn: &rusqlite::Connection, days: i64) -> CmdResult<Vec<DailyLoad>> {
    let days = days.clamp(1, HEATMAP_MAX_DAYS);
    let today = today_number(conn)?;
    let from = today - days + 1;

    let mut loads: Vec<DailyLoad> = {
        let mut stmt = conn.prepare(DAILY_LOAD_SQL).map_err(to_string_err)?;
        let rows = stmt
            .query_map(params![day_string(from), day_string(today), day_string(today)], |r| {
                let tracked: i64 = r.get(4)?;
                Ok(DailyLoad {
                    date: r.get(0)?,
                    tasks_due: r.get(1)?,
                    tasks_completed: r.get(2)?,
                    tasks_overdue: r.get(3)?,
                    tracked_minutes: tracked / 60,
                    habits_due: 0,
                    habits_completed: r.get(5)?,
                    habits_fallback: r.get(6)?,
                    habits_skipped: r.get(7)?,
                    habits_failed: r.get(8)?,
                    habits_missed: 0,
                    marks: Vec::new(),
                })
            })
            .map_err(to_string_err)?;
        rows.collect::<rusqlite::Result<_>>().map_err(to_string_err)?
    };

    let metas = habit_metas(conn, None, today)?;
    let logs = logs_between(conn, from, today)?;
    for load in &mut loads {
        let Some(day) = day_number(&load.date) else { continue };
        for meta in &metas {
            if day < meta.start {
                continue;
            }
            let scheduled = meta.freq.is_scheduled(day);
            if scheduled {
                load.habits_due += 1;
            }
            let mark = match logs.get(&(meta.id, day)) {
                Some((status, reason)) => Some((status.clone(), reason.clone())),
                None if scheduled && day < today => {
                    load.habits_missed += 1;
                    Some(("missed".to_string(), None))
                }
                None => None,
            };
            if let Some((status, reason)) = mark {
                load.marks.push(HabitMark { habit_id: meta.id, habit: meta.title.clone(), status, reason });
            }
        }
    }
    Ok(loads)
}

/// Сводка за 14 дней — для графика рядом с ИИ-анализом.
#[tauri::command]
pub fn get_habit_task_summary(state: State<'_, DbState>) -> CmdResult<Vec<DailyLoad>> {
    let conn = state.conn.lock().unwrap();
    daily_load_in(&conn, ANALYSIS_DAYS)
}

// ─── ИИ-анализ привычек и нагрузки ───

/// Системный промпт аналитика. Отдельный от промпта ассистента: здесь нет
/// инструментов и разговора — одна сводка на входе, один JSON на выходе.
const ANALYSIS_PROMPT: &str = "Ты — аналитик нагрузки в приложении TaskFlow. Тебе дают JSON-сводку \
за последние 14 дней: задачи и привычки пользователя по дням.

Поля дня: tasks_due — задач со сроком на этот день; tasks_completed — закрыто задач; tasks_overdue — \
задач со сроком на этот день, не закрытых к его концу; tracked_minutes — учтённое время по задачам; \
habit_marks — отметки привычек: completed (выполнено), fallback (выполнена лайт-версия), skipped \
(пропущено, стрик заморожен), failed (сорвалось), missed (день по расписанию прошёл без отметки); \
reason — причина, которую написал пользователь.
Поля привычки: time_of_day (morning — утро, afternoon — день, evening — вечер, anytime — без времени), \
effort_level — нагрузка от 1 до 5, fallback_title — лайт-версия (null — её нет).

Задача: найти причинно-следственные связи между нагрузкой по задачам и срывами привычек.
Ищи:
1. Корреляции пиков задач (tasks_due, tasks_overdue, tracked_minutes) со срывами привычек (failed, \
missed, skipped) в тот же или на следующий день.
2. Временные конфликты: привычки одного времени суток или с высоким effort_level, которые срываются \
в дни нагрузки.
3. Эффект домино: срыв одной привычки, после которого в следующие дни срываются другие.
4. Привычки, которые держатся только на лайт-версии, и привычки без лайт-версии, которые срываются \
в тяжёлые дни.

Правила:
- Только факты из сводки. Каждое утверждение опирается на конкретные даты (ГГГГ-ММ-ДД) и числа из неё.
- Запрещены мотивационные фразы, похвала, утешения и общие советы («важно не сдаваться», «вы \
молодец», «старайтесь», «берегите себя»). Только сухая аналитика.
- Не хватает данных для вывода — не выдумывай. Нет закономерностей — верни пустой массив correlations.
- confidence: high — закономерность повторилась 3 и больше раз без исключений; medium — 2 раза или \
с исключениями; low — один эпизод.
- actionable_adjustments — конкретные изменения: какую привычку (название ровно как в сводке) на \
какое время суток или дни перенести, при каком пороге включать лайт-версию (например, «в дни, когда \
задач со сроком больше 3»), какую лайт-версию завести, какую нагрузку снизить. habit — название \
привычки или пустая строка, если совет общий. reason — на какие даты и числа опирается совет.
- workload_verdict — 1–2 предложения: как нагрузка по задачам сказалась на привычках за период.
- Пиши только по-русски. Ответ — только JSON по заданной схеме, без текста вокруг.";

pub(super) fn analysis_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "workload_verdict": { "type": "string" },
            "correlations": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "pattern": { "type": "string" },
                        "evidence": { "type": "string" },
                        "confidence": { "type": "string", "enum": ["high", "medium", "low"] }
                    },
                    "required": ["pattern", "evidence", "confidence"]
                }
            },
            "actionable_adjustments": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "habit": { "type": "string" },
                        "change": { "type": "string" },
                        "reason": { "type": "string" }
                    },
                    "required": ["habit", "change", "reason"]
                }
            }
        },
        "required": ["workload_verdict", "correlations", "actionable_adjustments"]
    })
}

const RU_WEEKDAYS: [&str; 7] = ["пн", "вт", "ср", "чт", "пт", "сб", "вс"];

fn describe_frequency(habit: &Habit) -> String {
    match habit.frequency_type.as_str() {
        "weekly_days" => {
            let days: Vec<&str> = habit
                .weekdays
                .iter()
                .filter_map(|d| weekday_index(d).map(|i| RU_WEEKDAYS[i]))
                .collect();
            format!("по дням: {}", days.join(", "))
        }
        "x_per_week" => format!("{} раз(а) в неделю", habit.target_per_week.unwrap_or(1)),
        _ => "ежедневно".to_string(),
    }
}

/// Вход модели: период, привычки и дни. Без id — модели они не нужны, а
/// в ответе выглядели бы мусором. Возвращает и границы периода.
pub(super) fn analysis_input(conn: &rusqlite::Connection) -> CmdResult<(Value, String, String)> {
    let habits = list_habits_in(conn, false)?;
    let loads = daily_load_in(conn, ANALYSIS_DAYS)?;
    let from = loads.first().map(|d| d.date.clone()).unwrap_or_default();
    let to = loads.last().map(|d| d.date.clone()).unwrap_or_default();

    let habits_json: Vec<Value> = habits
        .iter()
        .map(|h| {
            json!({
                "title": h.title,
                "frequency": describe_frequency(h),
                "time_of_day": h.time_of_day,
                "effort_level": h.effort_level,
                "fallback_title": h.fallback_title,
                "current_streak": h.current_streak,
                "streak_unit": if h.streak_unit == "week" { "недель" } else { "дней" },
            })
        })
        .collect();
    let days_json: Vec<Value> = loads
        .iter()
        .map(|d| {
            let weekday = day_number(&d.date).map(|n| RU_WEEKDAYS[weekday_of(n)]).unwrap_or("");
            let marks: Vec<Value> = d
                .marks
                .iter()
                .map(|m| json!({ "habit": m.habit, "status": m.status, "reason": m.reason }))
                .collect();
            json!({
                "date": d.date,
                "weekday": weekday,
                "tasks_due": d.tasks_due,
                "tasks_completed": d.tasks_completed,
                "tasks_overdue": d.tasks_overdue,
                "tracked_minutes": d.tracked_minutes,
                "habits_due": d.habits_due,
                "habits_completed": d.habits_completed,
                "habits_fallback": d.habits_fallback,
                "habits_skipped": d.habits_skipped,
                "habits_failed": d.habits_failed,
                "habits_missed": d.habits_missed,
                "habit_marks": marks,
            })
        })
        .collect();

    let input = json!({
        "period": {
            "from": from,
            "to": to,
            "note": "Последний день периода — сегодня, он ещё идёт: неотмеченные сегодня привычки и \
                     незакрытые сегодня задачи пропуском и просрочкой не считаются. У задач нет времени \
                     суток — только дата срока.",
        },
        "habits": habits_json,
        "days": days_json,
    });
    Ok((input, from, to))
}

/// Можно ли что-то анализировать: есть привычки и хоть одна отметка за период.
fn check_analysis_input(input: &Value) -> CmdResult<()> {
    if input["habits"].as_array().map_or(true, |h| h.is_empty()) {
        return Err(ERR_ANALYSIS_NO_HABITS.into());
    }
    let logged = input["days"].as_array().is_some_and(|days| {
        days.iter().any(|d| {
            d["habit_marks"]
                .as_array()
                .is_some_and(|marks| marks.iter().any(|m| m["status"] != "missed"))
        })
    });
    if !logged {
        return Err(ERR_ANALYSIS_NO_LOGS.into());
    }
    Ok(())
}

/// JSON из ответа модели: с кодовыми кавычками ```json и текстом вокруг
/// тоже — от первой `{` до последней `}`.
fn json_object_in(raw: &str) -> Option<&str> {
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;
    (end > start).then(|| &raw[start..=end])
}

fn normalize_confidence(raw: &str) -> String {
    let lower = raw.trim().to_lowercase();
    match lower.as_str() {
        "high" | "высокая" | "высокий" => "high",
        "medium" | "средняя" | "средний" => "medium",
        _ => "low",
    }
    .to_string()
}

/// Разбор и проверка ответа модели. Промпт маленькая модель выполняет через
/// раз (PROJECT_NOTES §39), поэтому то, что можно проверить, проверяется
/// здесь, а не выпрашивается:
/// - пустой вердикт — ответ не принят;
/// - закономерность без единой цифры в подтверждении — это «вода», а не
///   опора на даты и числа, она отбрасывается;
/// - совет про привычку, которой нет в сводке, отбрасывается, а название
///   существующей приводится к точному;
/// - уверенность — только high/medium/low, прочее читается как low.
pub(super) fn check_analysis(raw: &str, habit_titles: &[String]) -> Result<HabitAnalysis, String> {
    let body = json_object_in(raw).ok_or_else(|| "в ответе нет JSON-объекта".to_string())?;
    let parsed: HabitAnalysis =
        serde_json::from_str(body).map_err(|e| format!("JSON не по схеме: {}", e))?;

    let workload_verdict = parsed.workload_verdict.trim().to_string();
    if workload_verdict.is_empty() {
        return Err("пустой workload_verdict".into());
    }

    let correlations: Vec<HabitCorrelation> = parsed
        .correlations
        .into_iter()
        .filter_map(|c| {
            let pattern = c.pattern.trim().to_string();
            let evidence = c.evidence.trim().to_string();
            if pattern.is_empty() {
                return None;
            }
            if !evidence.chars().any(|ch| ch.is_ascii_digit()) {
                log::warn!("анализ привычек: закономерность без цифр отброшена: {}", pattern);
                return None;
            }
            Some(HabitCorrelation { pattern, evidence, confidence: normalize_confidence(&c.confidence) })
        })
        .take(MAX_ANALYSIS_ITEMS)
        .collect();

    let actionable_adjustments: Vec<HabitAdjustment> = parsed
        .actionable_adjustments
        .into_iter()
        .filter_map(|a| {
            let change = a.change.trim().to_string();
            if change.is_empty() {
                return None;
            }
            let named = a.habit.trim().trim_matches(['«', '»', '"']).to_lowercase();
            let habit = if named.is_empty() {
                String::new()
            } else {
                match habit_titles.iter().find(|t| t.trim().to_lowercase() == named) {
                    Some(title) => title.clone(),
                    None => {
                        log::warn!("анализ привычек: совет про неизвестную привычку «{}» отброшен", a.habit);
                        return None;
                    }
                }
            };
            Some(HabitAdjustment { habit, change, reason: a.reason.trim().to_string() })
        })
        .take(MAX_ANALYSIS_ITEMS)
        .collect();

    Ok(HabitAnalysis { workload_verdict, correlations, actionable_adjustments })
}

/// Весь текст анализа одной строкой — для проверки языка.
fn analysis_text(analysis: &HabitAnalysis) -> String {
    let mut parts = vec![analysis.workload_verdict.clone()];
    for c in &analysis.correlations {
        parts.push(c.pattern.clone());
        parts.push(c.evidence.clone());
    }
    for a in &analysis.actionable_adjustments {
        parts.push(a.change.clone());
        parts.push(a.reason.clone());
    }
    parts.join("\n")
}

/// Обрезает иероглифический хвост в каждом поле — последний рубеж после
/// повтора, как у ответа ассистента.
fn cut_foreign_tails(mut analysis: HabitAnalysis) -> HabitAnalysis {
    analysis.workload_verdict = super::ai_tools::cut_foreign_tail(&analysis.workload_verdict);
    for c in &mut analysis.correlations {
        c.pattern = super::ai_tools::cut_foreign_tail(&c.pattern);
        c.evidence = super::ai_tools::cut_foreign_tail(&c.evidence);
    }
    for a in &mut analysis.actionable_adjustments {
        a.change = super::ai_tools::cut_foreign_tail(&a.change);
        a.reason = super::ai_tools::cut_foreign_tail(&a.reason);
    }
    analysis
}

pub(super) fn analysis_request(input: &Value) -> Vec<crate::ollama::Message> {
    use crate::ollama::Message;
    vec![
        Message::new("system", ANALYSIS_PROMPT),
        Message::new("user", format!("Сводка за последние {} дней:\n{}", ANALYSIS_DAYS, input)),
    ]
}

/// Запрос к модели с проверками: ответ не по схеме — один повтор с
/// объяснением, что не так; ответ не по-русски — один повтор с просьбой
/// писать по-русски, затем обрезка чужого хвоста.
pub(super) async fn ask_analysis(settings: &AiSettings, input: &Value, habit_titles: &[String]) -> CmdResult<HabitAnalysis> {
    use crate::ollama::Message;
    let schema = analysis_schema();
    let ask = |messages: Vec<Message>| {
        let settings = settings.clone();
        let schema = schema.clone();
        run_blocking(move || {
            crate::ollama::chat_structured(&settings.ollama_url, &settings.model, &messages, &schema, settings.timeout_seconds)
        })
    };

    let request = analysis_request(input);
    let first = ask(request.clone()).await?;
    let (mut analysis, mut conversation) = match check_analysis(&first.content, habit_titles) {
        Ok(a) => (a, {
            let mut c = request.clone();
            c.push(first);
            c
        }),
        Err(reason) => {
            log::warn!("анализ привычек: ответ не принят ({}) — прошу повторить", reason);
            let mut retry = request.clone();
            retry.push(first);
            retry.push(Message::new(
                "system",
                format!(
                    "Проверка приложения: ответ не принят — {}. Пришли только JSON-объект по схеме: \
                     workload_verdict (строка), correlations (массив pattern/evidence/confidence), \
                     actionable_adjustments (массив habit/change/reason).",
                    reason
                ),
            ));
            let second = ask(retry.clone()).await?;
            let analysis = check_analysis(&second.content, habit_titles).map_err(|e| {
                log::warn!("анализ привычек: повтор тоже не принят ({})", e);
                ERR_ANALYSIS_BAD_ANSWER.to_string()
            })?;
            retry.push(second);
            (analysis, retry)
        }
    };

    if super::ai_tools::needs_language_retry(&analysis_text(&analysis)) {
        log::warn!("анализ привычек: ответ не на русском — прошу повторить");
        conversation.push(Message::new("system", super::ai_tools::NUDGE_LANGUAGE));
        if let Ok(reply) = ask(conversation).await {
            if let Ok(again) = check_analysis(&reply.content, habit_titles) {
                analysis = again;
            }
        }
        analysis = cut_foreign_tails(analysis);
    }
    Ok(analysis)
}

fn save_analysis(
    conn: &rusqlite::Connection,
    model: &str,
    from: &str,
    to: &str,
    input: &Value,
    analysis: &HabitAnalysis,
) -> CmdResult<()> {
    conn.execute(
        "INSERT INTO habit_ai_reports (id, generated_at, model, period_from, period_to, input_json, report_json)
         VALUES (1, datetime('now'), ?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(id) DO UPDATE
            SET generated_at = excluded.generated_at, model = excluded.model,
                period_from = excluded.period_from, period_to = excluded.period_to,
                input_json = excluded.input_json, report_json = excluded.report_json",
        params![model, from, to, input.to_string(), serde_json::to_string(analysis).map_err(to_string_err)?],
    )
    .map_err(to_string_err)?;
    Ok(())
}

/// Последний анализ из кэша и устарел ли он: устаревшим считается анализ,
/// сводка для которого сейчас вышла бы другой.
pub(super) fn cached_analysis_in(conn: &rusqlite::Connection) -> CmdResult<Option<HabitAnalysisReport>> {
    let row = conn
        .query_row(
            "SELECT generated_at, model, period_from, period_to, input_json, report_json FROM habit_ai_reports WHERE id = 1",
            [],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                ))
            },
        )
        .optional()
        .map_err(to_string_err)?;
    let Some((generated_at, model, period_from, period_to, input_json, report_json)) = row else {
        return Ok(None);
    };
    let Ok(analysis) = serde_json::from_str::<HabitAnalysis>(&report_json) else {
        // Кэш от другой версии формата — как будто анализа не было.
        return Ok(None);
    };
    let (fresh, _, _) = analysis_input(conn)?;
    let stale = serde_json::from_str::<Value>(&input_json).map_or(true, |old| old != fresh);
    Ok(Some(HabitAnalysisReport { analysis, generated_at, model, period_from, period_to, stale }))
}

#[tauri::command]
pub fn get_habit_analysis(state: State<'_, DbState>) -> CmdResult<Option<HabitAnalysisReport>> {
    let conn = state.conn.lock().unwrap();
    cached_analysis_in(&conn)
}

/// Новый анализ: сводка за 14 дней → модель → проверки → кэш. Замок базы на
/// время раздумий модели не держится — как у ассистента.
#[tauri::command]
pub async fn run_habit_analysis(app: tauri::AppHandle) -> CmdResult<HabitAnalysisReport> {
    use tauri::Manager;

    let (settings, input, from, to, titles) = {
        let state = app.state::<DbState>();
        let conn = state.conn.lock().map_err(|_| "База занята".to_string())?;
        let settings = read_ai_settings(&conn)?;
        let (input, from, to) = analysis_input(&conn)?;
        check_analysis_input(&input)?;
        let titles: Vec<String> = list_habits_in(&conn, false)?.into_iter().map(|h| h.title).collect();
        (settings, input, from, to, titles)
    };

    let analysis = ask_analysis(&settings, &input, &titles).await?;

    let state = app.state::<DbState>();
    let conn = state.conn.lock().map_err(|_| "База занята".to_string())?;
    save_analysis(&conn, &settings.model, &from, &to, &input, &analysis)?;
    cached_analysis_in(&conn)?.ok_or_else(|| "Анализ не сохранился".to_string())
}

#[cfg(test)]
#[path = "habits_tests.rs"]
mod tests;
