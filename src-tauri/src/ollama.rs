//! Разговор с локальной Ollama по HTTP.
//!
//! Ollama — **внешний процесс**: TaskFlow её не запускает, не останавливает и
//! не зависит от неё при сборке. Весь обмен — два адреса её API
//! (`/api/tags` и `/api/chat`) через `reqwest`. Нет Ollama — не работает
//! только экран ассистента, всё остальное приложение этого не замечает.
//!
//! Как и `email.rs`, модуль не открывает базу: что спросить, решает
//! `commands.rs`, здесь — только как спросить.
//!
//! ## Только этот компьютер
//!
//! Адрес принимается лишь петлевой — `localhost`, `127.x.x.x`, `[::1]`.
//! Приложение офлайновое (PROJECT_NOTES §5), а в чат уходят названия задач и
//! всё, что человек напишет; адрес «на другом порту» — законная правка, адрес
//! другой машины — это уже выход в сеть, и молча его открывать нельзя.
//!
//! ## Почему запрос уходит из отдельного потока
//!
//! `reqwest::blocking` держит внутри собственный рантайм tokio и падает с
//! паникой, если его создать или уронить внутри чужого асинхронного контекста.
//! А `#[tauri::command(async)]` у синхронной функции выполняет её тело именно
//! там — в задаче tokio (`respond_async_serialized` → `async_runtime::spawn`).
//! Поэтому команды ассистента объявлены `async fn` и зовут функции этого
//! модуля через `spawn_blocking`: там блокироваться разрешено.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Адрес по умолчанию — тот, на котором Ollama слушает после установки.
pub const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";

/// Сколько ждать один ответ модели, если в настройках не задано иное.
/// 180 с, а не 120: холодная загрузка 7B-модели на машине пользователя —
/// 53 с, и с инструментами запросов к модели на один вопрос уже два-три
/// (решение пользователя от 2026-09-26). Предел действует на **каждый** запрос
/// к Ollama, а не на весь вопрос целиком.
pub const DEFAULT_CHAT_TIMEOUT_SECS: i64 = 180;
pub const CHAT_TIMEOUT_MIN_SECS: i64 = 30;
pub const CHAT_TIMEOUT_MAX_SECS: i64 = 600;

/// Сколько ждать список моделей. Ollama отвечает на него мгновенно, а
/// «не запущена» при отказе в соединении приходит сразу; этот предел только
/// на случай, если по адресу слушает что-то чужое и молчит.
const STATUS_TIMEOUT: Duration = Duration::from_secs(5);

pub const ERR_NOT_RUNNING: &str =
    "Ollama не запущена или недоступна по этому адресу. Запустите Ollama и проверьте адрес в Настройках.";
pub const ERR_TIMEOUT: &str =
    "Модель не ответила вовремя — попробуйте ещё раз или выберите более лёгкую модель.";
pub const ERR_BAD_URL: &str =
    "Адрес Ollama должен выглядеть как http://localhost:11434";
pub const ERR_NOT_LOCAL: &str =
    "Адрес Ollama должен указывать на этот компьютер (localhost, 127.0.0.1 или [::1])";

/// Одно сообщение в формате `/api/chat`: `role` — `system`, `user`,
/// `assistant` или `tool`.
///
/// `tool_calls` бывает только у ответа модели: вместо текста она просит
/// вызвать инструменты. `tool_name` — у сообщения с результатом инструмента,
/// чтобы модель знала, на какой из вызовов это ответ.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    #[serde(default)]
    pub content: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
}

impl Message {
    pub fn new(role: &str, content: impl Into<String>) -> Self {
        Message { role: role.into(), content: content.into(), tool_calls: Vec::new(), tool_name: None }
    }

    pub fn tool_result(name: &str, content: impl Into<String>) -> Self {
        Message { tool_name: Some(name.into()), ..Message::new("tool", content) }
    }
}

/// Просьба модели вызвать инструмент. Формат OpenAI-совместимый; `arguments`
/// Ollama отдаёт объектом, а не строкой JSON, как OpenAI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub function: FunctionCall,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    #[serde(default)]
    pub arguments: serde_json::Value,
}

/// Модель из `/api/tags` — только то, что показывает интерфейс.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelInfo {
    pub name: String,
    /// Размер файла модели в байтах: по нему видно, какая из двух похожих
    /// моделей «лёгкая».
    #[serde(default)]
    pub size: u64,
}

#[derive(Deserialize)]
struct TagsResponse {
    #[serde(default)]
    models: Vec<ModelInfo>,
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: &'a [Message],
    #[serde(skip_serializing_if = "<[serde_json::Value]>::is_empty")]
    tools: &'a [serde_json::Value],
    stream: bool,
}

#[derive(Deserialize)]
struct ChatResponse {
    message: Option<Message>,
}

/// Ollama отвечает на ошибки телом `{"error": "..."}` — например, когда
/// выбранную модель успели удалить.
#[derive(Deserialize)]
struct ErrorResponse {
    error: String,
}

/// Приводит адрес к виду `http://хост:порт` без хвостового `/` и проверяет,
/// что он указывает на этот компьютер.
///
/// `https` отвергается не из вредности: Ollama сама TLS не поднимает, а
/// сборка `reqwest` здесь без TLS — ради одного петлевого адреса тащить в
/// бинарник второй стек шифрования незачем.
pub fn normalize_url(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim().trim_end_matches('/');
    let rest = trimmed
        .strip_prefix("http://")
        .ok_or_else(|| ERR_BAD_URL.to_string())?;
    // Путь после хоста не нужен и только мешал бы склеивать `/api/...`.
    if rest.is_empty() || rest.contains('/') || rest.contains('@') {
        return Err(ERR_BAD_URL.to_string());
    }

    let host = host_of(rest).ok_or_else(|| ERR_BAD_URL.to_string())?;
    if !is_loopback_host(&host) {
        return Err(ERR_NOT_LOCAL.to_string());
    }
    Ok(format!("http://{}", rest))
}

/// Хост из `хост[:порт]`, с IPv6 в квадратных скобках. Порт, если он есть,
/// обязан быть числом.
fn host_of(authority: &str) -> Option<String> {
    let (host, port) = if let Some(after) = authority.strip_prefix('[') {
        let end = after.find(']')?;
        let host = &after[..end];
        let tail = &after[end + 1..];
        let port = if tail.is_empty() { None } else { Some(tail.strip_prefix(':')?) };
        (host, port)
    } else {
        match authority.rsplit_once(':') {
            Some((h, p)) => (h, Some(p)),
            None => (authority, None),
        }
    };
    if let Some(p) = port {
        p.parse::<u16>().ok()?;
    }
    if host.is_empty() {
        return None;
    }
    Some(host.to_ascii_lowercase())
}

fn is_loopback_host(host: &str) -> bool {
    if host == "localhost" {
        return true;
    }
    host.parse::<std::net::IpAddr>()
        .map(|ip| ip.is_loopback())
        .unwrap_or(false)
}

fn client(timeout: Duration) -> Result<reqwest::blocking::Client, String> {
    reqwest::blocking::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|e| format!("Не удалось подготовить HTTP-клиент: {}", e))
}

/// Человеческий текст для ошибки транспорта.
fn describe(err: reqwest::Error) -> String {
    if err.is_timeout() {
        ERR_TIMEOUT.to_string()
    } else if err.is_connect() {
        ERR_NOT_RUNNING.to_string()
    } else {
        format!("Ошибка связи с Ollama: {}", err)
    }
}

/// Текст ошибки из тела ответа с кодом не 2xx.
fn error_from_body(status: reqwest::StatusCode, body: &str) -> String {
    match serde_json::from_str::<ErrorResponse>(body) {
        Ok(e) => format!("Ollama ответила ошибкой: {}", e.error),
        Err(_) => format!("Ollama ответила кодом {}", status.as_u16()),
    }
}

/// Список установленных моделей. Он же — проверка «Ollama запущена»: другого
/// способа спросить её о здоровье, кроме как задать вопрос, нет.
pub fn list_models(base_url: &str) -> Result<Vec<ModelInfo>, String> {
    let url = format!("{}/api/tags", normalize_url(base_url)?);
    let resp = client(STATUS_TIMEOUT)?.get(url).send().map_err(describe)?;
    let status = resp.status();
    let body = resp.text().map_err(describe)?;
    if !status.is_success() {
        return Err(error_from_body(status, &body));
    }
    parse_tags(&body)
}

pub(crate) fn parse_tags(body: &str) -> Result<Vec<ModelInfo>, String> {
    let tags: TagsResponse = serde_json::from_str(body)
        // Ответил кто-то, но не Ollama — например, другой сервер на этом порту.
        .map_err(|_| "По этому адресу отвечает не Ollama".to_string())?;
    let mut models = tags.models;
    models.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(models)
}

/// Один запрос к модели без потоковой выдачи: ответ приходит целиком.
///
/// `messages` уже собраны вызывающим — системный промпт первым, дальше
/// история, новое сообщение и, если модель уже просила инструменты, их
/// результаты. `tools` — описания инструментов (JSON-схемы); пустой срез —
/// запрос без инструментов.
///
/// Возвращает ответ модели: либо текст, либо `tool_calls` — тогда вызывающий
/// выполняет инструменты и спрашивает снова.
pub fn chat(
    base_url: &str,
    model: &str,
    messages: &[Message],
    tools: &[serde_json::Value],
    timeout_secs: i64,
) -> Result<Message, String> {
    if model.trim().is_empty() {
        return Err("Модель не выбрана — выберите её в Настройках → «ИИ-ассистент»".to_string());
    }
    let url = format!("{}/api/chat", normalize_url(base_url)?);
    let request = ChatRequest { model, messages, tools, stream: false };
    let timeout = timeout_secs.clamp(CHAT_TIMEOUT_MIN_SECS, CHAT_TIMEOUT_MAX_SECS) as u64;
    let resp = client(Duration::from_secs(timeout))?
        .post(url)
        .json(&request)
        .send()
        .map_err(describe)?;
    let status = resp.status();
    let body = resp.text().map_err(describe)?;
    if !status.is_success() {
        return Err(chat_error(status, &body, model));
    }
    parse_chat(&body)
}

/// Ошибка `/api/chat`. Отдельно узнаётся модель без инструментов: Ollama
/// отвечает на такое «does not support tools», и человеку нужен не этот текст,
/// а что с ним делать.
fn chat_error(status: reqwest::StatusCode, body: &str, model: &str) -> String {
    if body.contains("does not support tools") {
        return format!(
            "Модель «{}» не умеет вызывать инструменты, а без них ассистент не видит ваших задач. \
             Выберите в Настройках модель с поддержкой инструментов — например, qwen2.5 или llama3.1.",
            model
        );
    }
    error_from_body(status, body)
}

pub(crate) fn parse_chat(body: &str) -> Result<Message, String> {
    let parsed: ChatResponse = serde_json::from_str(body)
        .map_err(|_| "Не удалось разобрать ответ Ollama".to_string())?;
    let mut message = parsed.message.unwrap_or_else(|| Message::new("assistant", ""));
    message.content = message.content.trim().to_string();
    if message.content.is_empty() && message.tool_calls.is_empty() {
        return Err("Модель вернула пустой ответ — попробуйте переформулировать".to_string());
    }
    Ok(message)
}

#[cfg(test)]
#[path = "ollama_tests.rs"]
mod tests;
