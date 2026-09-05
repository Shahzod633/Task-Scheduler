// ============================================
// TaskFlow — tests for the mail layer
// ============================================
// Ни один тест здесь не открывает соединения и не трогает Диспетчер учётных
// данных: проверяется то, что можно проверить без сети и без чужого сервера —
// разбор настроек и сборка конверта. Именно на них ломается настройка почты в
// первые пять минут, а не на самом SMTP.

use super::*;

fn valid() -> EmailSettings {
    EmailSettings {
        enabled: true,
        smtp_host: "smtp.gmail.com".into(),
        smtp_port: 465,
        username: "sender@gmail.com".into(),
        recipient: "boss@example.com".into(),
        has_password: false,
    }
}

#[test]
fn a_filled_in_configuration_passes() {
    assert_eq!(check_settings(&valid()), Ok(()));
}

#[test]
fn every_empty_field_is_named_separately() {
    // Общее «настройки почты заполнены не полностью» заставляет человека
    // перебирать поля наугад; каждое сообщение здесь показывает ровно одно.
    let mut s = valid();
    s.smtp_host = "  ".into();
    assert_eq!(check_settings(&s), Err(ERR_NO_SMTP_HOST.to_string()));

    let mut s = valid();
    s.username = String::new();
    assert_eq!(check_settings(&s), Err(ERR_NO_USERNAME.to_string()));

    let mut s = valid();
    s.recipient = String::new();
    assert_eq!(check_settings(&s), Err(ERR_NO_RECIPIENT.to_string()));
}

#[test]
fn a_port_outside_the_range_is_refused() {
    for port in [0, -1, 65536, 999999] {
        let mut s = valid();
        s.smtp_port = port;
        assert_eq!(
            check_settings(&s),
            Err(ERR_BAD_SMTP_PORT.to_string()),
            "порт {} не должен проходить проверку",
            port
        );
    }
}

#[test]
fn a_typo_in_an_address_is_caught_before_the_connection() {
    // Иначе опечатка выясняется через двадцать секунд таймаута или, хуже,
    // отказом сервера уже после ввода пароля.
    let mut s = valid();
    s.recipient = "boss@".into();
    let err = check_settings(&s).unwrap_err();
    assert!(err.starts_with("Адрес получателя"), "получено: {}", err);

    let mut s = valid();
    s.username = "не адрес вовсе".into();
    let err = check_settings(&s).unwrap_err();
    assert!(err.starts_with("Логин отправителя"), "получено: {}", err);
}

#[test]
fn spaces_around_an_address_do_not_break_it() {
    // Адрес, скопированный из письма, почти всегда приезжает с пробелом.
    let mut s = valid();
    s.recipient = "  boss@example.com  ".into();
    assert_eq!(check_settings(&s), Ok(()));

    let msg = build_message(
        &s,
        &Outgoing {
            subject: "Тема".into(),
            body: "Текст".into(),
        },
    )
    .unwrap();
    let raw = String::from_utf8_lossy(&msg.formatted()).to_string();
    assert!(raw.contains("To: boss@example.com"), "получено: {}", raw);
}

#[test]
fn the_envelope_carries_the_sender_the_recipient_and_the_text() {
    let msg = build_message(
        &valid(),
        &Outgoing {
            subject: "Срок задачи".into(),
            body: "Две строки\nтекста".into(),
        },
    )
    .unwrap();

    let raw = String::from_utf8_lossy(&msg.formatted()).to_string();
    assert!(
        raw.contains("From: TaskFlow <sender@gmail.com>"),
        "письмо должно приходить от логина отправителя: {}",
        raw
    );
    assert!(raw.contains("To: boss@example.com"), "получено: {}", raw);
    // Простой текст, а не HTML: напоминание — это две строки, и вторая копия
    // того же текста в разметке только расходилась бы с первой.
    assert!(raw.contains("text/plain"), "получено: {}", raw);
}

#[test]
fn a_broken_sender_address_stops_the_message_from_being_built() {
    let mut s = valid();
    s.username = "@@@".into();
    let err = build_message(
        &s,
        &Outgoing {
            subject: "Тема".into(),
            body: "Текст".into(),
        },
    )
    .unwrap_err();
    assert!(err.starts_with("Логин отправителя"), "получено: {}", err);
}
