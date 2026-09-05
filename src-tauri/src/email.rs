//! Отправка писем: SMTP через `lettre` и пароль приложения в Диспетчере
//! учётных данных Windows.
//!
//! **Это единственное место, откуда приложение выходит в сеть.** Всё
//! остальное — один файл SQLite на одной машине, и таким оно и задумано
//! (PROJECT_NOTES §5). Отправка включается человеком вручную, адрес сервера
//! вписывает тоже он: сама программа ни с чьим сервером не разговаривает по
//! собственной инициативе и никуда не ходит «за обновлениями».
//!
//! Разделение с `commands.rs` проведено по границе «что сказать» / «как
//! отправить»: там знают про карточки, сроки и базу, здесь — про почту.
//! Ни одна функция этого модуля не открывает соединение с базой.
//!
//! ## Пароль приложения
//!
//! Пароль **не хранится в SQLite** — тем же приёмом, что и ключ шифрования
//! базы (`crypto.rs`): база целиком расшифровывается ключом из Диспетчера, и
//! класть в неё второй секрет значило бы, что один унесённый файл выдаёт оба.
//! Диспетчер привязывает секрет к учётной записи Windows, поэтому копия базы
//! на чужой машине пароля от почты не содержит.
//!
//! Обратная сторона та же, что у ключа: переустановка системы или сброс
//! профиля стирают пароль, и его придётся ввести заново. Для пароля это
//! мелочь — в отличие от ключа, его всегда можно выписать заново в настройках
//! почты.

use std::time::Duration;

use lettre::message::{header::ContentType, Mailbox, Message};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Address, SmtpTransport, Transport};

use crate::models::EmailSettings;

/// Служба та же, что у ключа шифрования базы, — это одно приложение. Разное
/// только имя записи.
const KEYRING_SERVICE: &str = "TaskFlow";

/// Под этим именем пароль виден в Диспетчере учётных данных. Переименование
/// здесь не «переносит» секрет: старая запись останется висеть, а приложение
/// решит, что пароля нет.
const KEYRING_ENTRY: &str = "smtp-app-password";

/// Порт, на котором TLS поднимается сразу, до первой команды SMTP (SMTPS).
/// Всё остальное считается STARTTLS — см. `transport_for`.
const IMPLICIT_TLS_PORT: i64 = 465;

/// Сколько ждать сервер. Значение `lettre` по умолчанию — минута; столько
/// ждать нельзя: тестовое письмо отправляют, глядя на экран, и минута тишины
/// после нажатия кнопки читается как «приложение зависло».
const SMTP_TIMEOUT: Duration = Duration::from_secs(20);

/// Имя отправителя в поле `From`. Латиницей намеренно: кириллическое имя
/// пришлось бы кодировать по RFC 2047, а разбирают такие заголовки не все
/// почтовые программы одинаково.
const SENDER_NAME: &str = "TaskFlow";

pub const ERR_NO_SMTP_HOST: &str = "Не указан SMTP-сервер";
pub const ERR_BAD_SMTP_PORT: &str = "Порт SMTP должен быть числом от 1 до 65535";
pub const ERR_NO_USERNAME: &str = "Не указан логин отправителя";
pub const ERR_NO_RECIPIENT: &str = "Не указан адрес получателя";
pub const ERR_NO_PASSWORD: &str =
    "Пароль приложения не сохранён — введите его и нажмите «Сохранить пароль»";

/// Готовое письмо: тему и текст сочиняет `commands.rs`, здесь их только
/// упаковывают в конверт.
#[derive(Debug, Clone, PartialEq)]
pub struct Outgoing {
    pub subject: String,
    pub body: String,
}

// ─── Пароль в Диспетчере учётных данных ───

fn entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_ENTRY)
        .map_err(|e| format!("Диспетчер учётных данных недоступен: {}", e))
}

/// Кладёт пароль в Диспетчер. Пустая строка означает «убрать»: иначе поле,
/// очищенное человеком, сохраняло бы пустой пароль, с которым сервер
/// отказывает невнятной ошибкой авторизации.
pub fn store_password(password: &str) -> Result<(), String> {
    if password.is_empty() {
        return clear_password();
    }
    entry()?
        .set_password(password)
        .map_err(|e| format!("Не удалось сохранить пароль: {}", e))
}

/// Убирает пароль. Отсутствие записи — не ошибка: «удалить то, чего нет»
/// приводит ровно к тому состоянию, которого просили.
pub fn clear_password() -> Result<(), String> {
    match entry()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("Не удалось удалить пароль: {}", e)),
    }
}

/// Пароль из Диспетчера. `None` — записи нет; это обычное состояние до первой
/// настройки, а не сбой.
pub fn load_password() -> Result<Option<String>, String> {
    match entry()?.get_password() {
        Ok(password) => Ok(Some(password)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(format!("Не удалось прочитать пароль: {}", e)),
    }
}

/// Есть ли сохранённый пароль. Наружу, в интерфейс, отдаётся только этот
/// признак — сам пароль во фронтенд не уходит никогда: показывать его в поле
/// ввода незачем, а хранить копию в памяти веб-страницы тем более.
pub fn has_password() -> bool {
    matches!(load_password(), Ok(Some(_)))
}

// ─── Проверка настроек ───

/// Всё ли заполнено для отправки. Проверяется до соединения, чтобы человек
/// получил «не указан логин», а не таймаут на двадцать секунд.
pub fn check_settings(settings: &EmailSettings) -> Result<(), String> {
    if settings.smtp_host.trim().is_empty() {
        return Err(ERR_NO_SMTP_HOST.to_string());
    }
    if !(1..=65535).contains(&settings.smtp_port) {
        return Err(ERR_BAD_SMTP_PORT.to_string());
    }
    if settings.username.trim().is_empty() {
        return Err(ERR_NO_USERNAME.to_string());
    }
    if settings.recipient.trim().is_empty() {
        return Err(ERR_NO_RECIPIENT.to_string());
    }
    // Адреса разбираются здесь же: опечатка в адресе — самая частая причина
    // отказа, и узнавать о ней от сервера через двадцать секунд глупо.
    parse_address(&settings.username, "Логин отправителя")?;
    parse_address(&settings.recipient, "Адрес получателя")?;
    Ok(())
}

fn parse_address(value: &str, what: &str) -> Result<Address, String> {
    value
        .trim()
        .parse::<Address>()
        .map_err(|e| format!("{} не похож на адрес почты: {}", what, e))
}

// ─── Письмо ───

/// Собирает конверт: от кого, кому, тема, текст.
///
/// Письмо простым текстом, без HTML: напоминание — это две строки, а
/// html-версия потребовала бы второй копии того же текста и разбиралась бы
/// разными клиентами по-разному.
pub fn build_message(settings: &EmailSettings, msg: &Outgoing) -> Result<Message, String> {
    let from = Mailbox::new(
        Some(SENDER_NAME.to_string()),
        parse_address(&settings.username, "Логин отправителя")?,
    );
    let to = Mailbox::new(None, parse_address(&settings.recipient, "Адрес получателя")?);

    Message::builder()
        .from(from)
        .to(to)
        .subject(msg.subject.clone())
        .header(ContentType::TEXT_PLAIN)
        .body(msg.body.clone())
        .map_err(|e| format!("Не удалось собрать письмо: {}", e))
}

// ─── Отправка ───

/// Соединение с сервером.
///
/// Порт выбирает способ шифрования, потому что на практике их всего два и
/// путать их нельзя: 465 — TLS с первого байта, всё остальное (587 у Gmail,
/// Яндекса и Mail.ru) — STARTTLS поверх открытого соединения. Незашифрованной
/// отправки нет ни на каком порту: пароль приложения уходит в том же
/// соединении, и отдавать его открытым текстом нельзя.
fn transport_for(settings: &EmailSettings, password: &str) -> Result<SmtpTransport, String> {
    let host = settings.smtp_host.trim();
    let port = u16::try_from(settings.smtp_port).map_err(|_| ERR_BAD_SMTP_PORT.to_string())?;

    let builder = if settings.smtp_port == IMPLICIT_TLS_PORT {
        SmtpTransport::relay(host)
    } else {
        SmtpTransport::starttls_relay(host)
    }
    .map_err(|e| format!("Не удалось настроить соединение с {}: {}", host, e))?;

    Ok(builder
        .port(port)
        .timeout(Some(SMTP_TIMEOUT))
        .credentials(Credentials::new(
            settings.username.trim().to_string(),
            password.to_string(),
        ))
        .build())
}

/// Отправляет одно письмо и возвращает человеческую причину отказа.
///
/// Соединение открывается на каждое письмо заново — пул `lettre` не включён:
/// за один проход уходит одно-два письма, и держать ради них постоянное
/// соединение с почтовым сервером не за чем.
pub fn send(settings: &EmailSettings, password: &str, msg: &Outgoing) -> Result<(), String> {
    check_settings(settings)?;
    let message = build_message(settings, msg)?;
    let transport = transport_for(settings, password)?;
    transport.send(&message).map(|_| ()).map_err(describe_error)
}

/// Ошибка SMTP словами, которые что-то говорят человеку.
///
/// Сообщения `lettre` — это в основном ответ сервера как есть
/// («permanent error (535 5.7.8): Username and Password not accepted»).
/// Такое можно показать, но самая частая причина — не опечатка в пароле, а
/// обычный пароль от почты вместо пароля приложения, и подсказать об этом
/// стоит прямо здесь.
fn describe_error(e: lettre::transport::smtp::Error) -> String {
    let base = e.to_string();

    if e.is_timeout() {
        return format!(
            "{} — сервер не ответил вовремя. Проверьте адрес и порт, а также \
             не блокирует ли соединение брандмауэр или антивирус.",
            base
        );
    }
    if e.is_permanent() && base.contains("535") {
        return format!(
            "{} — сервер не принял логин и пароль. Для Gmail нужен именно \
             пароль приложения из настроек аккаунта, а не обычный пароль от \
             почты.",
            base
        );
    }
    base
}

#[cfg(test)]
#[path = "email_tests.rs"]
mod tests;
