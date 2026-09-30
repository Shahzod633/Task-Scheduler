// ============================================
// TaskFlow — tests for the Ollama client
// ============================================
// Сеть в обычном прогоне не трогается: проверяются разбор адреса и ответов.
// Живой разговор с моделью — в `#[ignore]`-тесте в конце, он требует
// запущенной Ollama.

use super::*;

#[test]
fn default_url_is_accepted_as_is() {
    assert_eq!(normalize_url(DEFAULT_OLLAMA_URL).unwrap(), "http://localhost:11434");
}

#[test]
fn url_is_trimmed_and_loses_trailing_slash() {
    assert_eq!(normalize_url("  http://localhost:11434/  ").unwrap(), "http://localhost:11434");
    assert_eq!(normalize_url("http://LOCALHOST:8080").unwrap(), "http://LOCALHOST:8080");
}

#[test]
fn loopback_addresses_are_accepted() {
    assert!(normalize_url("http://127.0.0.1:11434").is_ok());
    assert!(normalize_url("http://127.0.0.2:11434").is_ok());
    assert!(normalize_url("http://[::1]:11434").is_ok());
    assert!(normalize_url("http://localhost").is_ok());
}

#[test]
fn other_machines_are_refused() {
    for url in [
        "http://192.168.1.10:11434",
        "http://example.com:11434",
        "http://0.0.0.0:11434",
        "http://[2001:db8::1]:11434",
        // Имя, которое лишь начинается с localhost, — чужой хост.
        "http://localhost.evil.com:11434",
    ] {
        assert_eq!(normalize_url(url).unwrap_err(), ERR_NOT_LOCAL, "{url}");
    }
}

#[test]
fn malformed_urls_are_refused() {
    for url in [
        "",
        "localhost:11434",
        "https://localhost:11434",
        "http://",
        "http://localhost:11434/api",
        "http://localhost:порт",
        "http://localhost:99999",
        "http://user@localhost:11434",
        "http://[::1",
    ] {
        assert_eq!(normalize_url(url).unwrap_err(), ERR_BAD_URL, "{url:?}");
    }
}

#[test]
fn tags_are_parsed_and_sorted_by_name() {
    let body = r#"{"models":[
        {"name":"qwen2.5:7b","model":"qwen2.5:7b","size":4683087332,"details":{}},
        {"name":"llama3.1:8b","size":4920753328}
    ]}"#;
    let models = parse_tags(body).unwrap();
    assert_eq!(models.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), ["llama3.1:8b", "qwen2.5:7b"]);
    assert_eq!(models[1].size, 4683087332);
}

#[test]
fn empty_tag_list_is_not_an_error() {
    assert!(parse_tags(r#"{"models":[]}"#).unwrap().is_empty());
}

#[test]
fn foreign_server_is_reported() {
    assert!(parse_tags("<html>nginx</html>").is_err());
}

#[test]
fn chat_answer_is_extracted_and_trimmed() {
    let body = r#"{"model":"qwen2.5:7b","message":{"role":"assistant","content":"  Привет!\n"},"done":true}"#;
    let message = parse_chat(body).unwrap();
    assert_eq!(message.content, "Привет!");
    assert!(message.tool_calls.is_empty());
}

#[test]
fn tool_calls_are_parsed_with_object_arguments() {
    // Так отвечает Ollama: текст пуст, аргументы — объект, а не строка JSON.
    let body = r#"{"message":{"role":"assistant","content":"","tool_calls":[
        {"function":{"name":"get_boards","arguments":{}}},
        {"function":{"index":1,"name":"get_cards_in_column","arguments":{"column_id":7}}}
    ]},"done":true}"#;
    let message = parse_chat(body).unwrap();
    assert_eq!(message.tool_calls.len(), 2);
    assert_eq!(message.tool_calls[0].function.name, "get_boards");
    assert_eq!(message.tool_calls[1].function.arguments["column_id"], 7);
}

#[test]
fn tool_result_message_carries_the_tool_name_and_omits_empty_calls() {
    let json = serde_json::to_value(Message::tool_result("get_boards", "[]")).unwrap();
    assert_eq!(json, serde_json::json!({"role": "tool", "content": "[]", "tool_name": "get_boards"}));
    let json = serde_json::to_value(Message::new("user", "Привет")).unwrap();
    assert_eq!(json, serde_json::json!({"role": "user", "content": "Привет"}));
}

#[test]
fn a_model_without_tools_is_explained() {
    let text = chat_error(
        reqwest::StatusCode::BAD_REQUEST,
        r#"{"error":"registry.ollama.ai/library/gemma:2b does not support tools"}"#,
        "gemma:2b",
    );
    assert!(text.contains("не умеет вызывать инструменты"), "{text}");
    assert!(text.contains("gemma:2b"), "{text}");
}

#[test]
fn empty_chat_answer_is_an_error() {
    assert!(parse_chat(r#"{"message":{"role":"assistant","content":"   "}}"#).is_err());
    assert!(parse_chat(r#"{"done":true}"#).is_err());
}

#[test]
fn ollama_error_body_is_shown_to_the_user() {
    let text = error_from_body(reqwest::StatusCode::NOT_FOUND, r#"{"error":"model 'x' not found"}"#);
    assert_eq!(text, "Ollama ответила ошибкой: model 'x' not found");
    let text = error_from_body(reqwest::StatusCode::BAD_GATEWAY, "oops");
    assert_eq!(text, "Ollama ответила кодом 502");
}

#[test]
fn chat_without_model_fails_before_any_request() {
    // Порт 9 никто не слушает — но до соединения дело дойти не должно.
    let err = chat("http://127.0.0.1:9", "  ", &[], &[], DEFAULT_CHAT_TIMEOUT_SECS).unwrap_err();
    assert!(err.contains("Модель не выбрана"), "{err}");
}

#[test]
fn closed_port_reads_as_not_running() {
    // Порт 9 (discard) на Windows не слушается: соединение отвергается сразу.
    assert_eq!(list_models("http://127.0.0.1:9").unwrap_err(), ERR_NOT_RUNNING);
}

/// Живая проверка против запущенной Ollama — и заодно того, ради чего команды
/// ассистента зовут этот модуль через `spawn_blocking`: из рантайма Tauri
/// `reqwest::blocking` работает без паники.
///
/// `cargo test --lib ollama_live -- --ignored --nocapture`
#[test]
#[ignore]
fn ollama_live_round_trip_from_tauri_runtime() {
    let answer = tauri::async_runtime::block_on(async {
        tauri::async_runtime::spawn_blocking(|| {
            let models = list_models(DEFAULT_OLLAMA_URL)?;
            let model = models.first().ok_or("нет моделей")?.name.clone();
            println!("модель: {model}");
            chat(DEFAULT_OLLAMA_URL, &model, &[
                Message::new("system", "Отвечай одним словом."),
                Message::new("user", "Скажи «привет»."),
            ], &[], DEFAULT_CHAT_TIMEOUT_SECS)
        })
        .await
        .unwrap()
    });
    println!("ответ: {:?}", answer);
    assert!(answer.is_ok(), "{answer:?}");
}
