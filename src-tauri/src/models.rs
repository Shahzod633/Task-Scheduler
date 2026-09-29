use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Workspace {
    pub id: i64,
    pub name: String,
    pub visibility: String,
    pub created_at: String,
    pub archived: i8,
    /// File name of this workspace's sidebar background inside the
    /// `backgrounds` folder, or `None` if it has none. Carried on the workspace
    /// itself so the sidebar knows whether to ask for the picture at all, and
    /// so the front-end can key its cache on it: every upload gets a new name,
    /// which makes a stale cached image impossible.
    pub background_image_path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Board {
    pub id: i64,
    pub workspace_id: i64,
    pub name: String,
    pub gradient: String,
    pub is_starred: bool,
    pub created_at: String,
    pub archived: i8,
    pub is_system: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Column {
    pub id: i64,
    pub board_id: i64,
    pub name: String,
    pub position: i64,
    pub created_at: String,
    pub archived: i8,
    /// Финальная колонка: карточка, попавшая сюда, обратно уже не уезжает.
    /// Проверяется в `update_card_position`, а не только в интерфейсе, — иначе
    /// «необратимо» держалось бы на одном лишь диалоге подтверждения.
    pub is_final: bool,
    /// Колонка из обязательного костяка (`commands::REQUIRED_COLUMNS`): её
    /// нельзя удалить и нельзя переименовать. Флаг отвечает за защиту самой
    /// колонки, `is_final` — за судьбу карточек в ней; совпадают они только
    /// на «Закрыто».
    pub is_required: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Card {
    pub id: i64,
    pub column_id: i64,
    pub title: String,
    pub description: String,
    pub position: i64,
    pub due_date: Option<String>,
    pub created_at: String,
    pub archived: i8,
    pub is_mistake: bool,
    pub mistake_marked_at: Option<String>,
    pub mistake_resolved_at: Option<String>,
    /// Сколько раз по карточке уже просили ещё одну попытку. Дойдя до
    /// `RETRY_LIMIT`, карточка при следующей просрочке уходит в архив.
    pub retry_count: i64,
    /// Ids only — the board view already holds the member directory and looks
    /// the avatar up there, rather than re-joining `members` into every query.
    pub assignee_id: Option<i64>,
    pub author_id: Option<i64>,
    pub priority: String,
    /// Checklist progress, so the card face can show "2 из 5" without asking
    /// for the items themselves. Only filled in by `get_cards`; elsewhere both
    /// stay 0 and the counter simply is not drawn.
    pub checklist_total: i64,
    pub checklist_done: i64,
    /// Сколько незавершённых карточек блокирует эту — значок звена на лицевой
    /// стороне. Как и счётчик чек-листа, заполняется только `get_cards`;
    /// в остальных местах остаётся 0 и значок просто не рисуется.
    pub blocked_open: i64,
    pub labels: Vec<Label>,
    /// Populated only by queries that join across boards/columns (e.g. planner, mistake dashboard).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub board_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub board_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_name: Option<String>,
}

/// One sub-task inside a card.
#[derive(Debug, Serialize, Deserialize)]
pub struct ChecklistItem {
    pub id: i64,
    pub card_id: i64,
    pub text: String,
    pub is_done: bool,
    pub position: i64,
}

/// Комментарий к карточке.
///
/// Автор приезжает целиком, а не идентификатором: подпись рисуется сразу же, и
/// отдельный поход за участником ради инициалов и цвета был бы лишним. `None`
/// — участника удалили; сам комментарий при этом остаётся.
#[derive(Debug, Serialize, Deserialize)]
pub struct CardComment {
    pub id: i64,
    pub card_id: i64,
    pub body: String,
    pub created_at: String,
    pub author: Option<Member>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Label {
    pub id: i64,
    pub board_id: i64,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Notification {
    pub id: i64,
    pub title: String,
    pub body: String,
    pub created_at: String,
    pub read: bool,
}

/// Настройки напоминаний о дедлайнах — одни на всё приложение.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct ReminderSettings {
    pub enabled: bool,
    /// За сколько часов до истечения срока показывать напоминание.
    pub hours: i64,
}

/// Карточка, по которой пора напомнить. Не хранится — собирается запросом и
/// живёт до показа уведомления.
#[derive(Debug, Clone, PartialEq)]
pub struct DueReminder {
    pub card_id: i64,
    pub title: String,
    pub due_date: String,
    pub board_name: String,
}

/// Настройки email-напоминаний — тоже одни на всё приложение, рядом с
/// `ReminderSettings`: это два канала одного и того же напоминания.
///
/// Пароля приложения здесь нет и быть не может: он лежит в Диспетчере учётных
/// данных (`email.rs`), а наружу отдаётся только `has_password` — признак
/// того, что он там есть.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct EmailSettings {
    pub enabled: bool,
    pub smtp_host: String,
    pub smtp_port: i64,
    /// Логин отправителя. Он же адрес в поле `From`: почтовые серверы всё
    /// равно не дают отправлять от чужого имени.
    pub username: String,
    pub recipient: String,
    /// Заполняется не из базы, а из Диспетчера учётных данных. Читатель
    /// настроек оставляет здесь `false`, команда подставляет настоящее
    /// значение.
    #[serde(default)]
    pub has_password: bool,
}

/// Значения по умолчанию живут константами, а не значениями `DEFAULT` в схеме:
/// в схеме они оказались бы сразу в двух местах (`CREATE TABLE` и миграция
/// `ALTER TABLE`) и со временем разошлись бы. В базе новые поля пустые, а
/// пустое читается как «ещё не настраивали» — см. `read_email_settings`.
pub const DEFAULT_SMTP_HOST: &str = "smtp.gmail.com";

/// 465 — TLS с первого байта. Выбран вместо 587 (STARTTLS) потому, что
/// шифрование на нём не зависит от того, объявит ли сервер поддержку
/// STARTTLS.
pub const DEFAULT_SMTP_PORT: i64 = 465;

/// Адрес, который подставляется в поле получателя при первом открытии
/// настроек. Редактируется, как и всё остальное.
pub const DEFAULT_EMAIL_RECIPIENT: &str = "shahzodisorbon633@gmail.com";

/// Карточка, по которой пора отправить письмо. Как и `DueReminder`, не
/// хранится: собирается запросом и живёт до отправки.
#[derive(Debug, Clone, PartialEq)]
pub struct EmailReminder {
    pub card_id: i64,
    pub title: String,
    pub due_date: String,
    pub board_name: String,
    /// Сколько дней осталось до конца дня срока. 0 — срок сегодня.
    pub days_left: i64,
}

// ─── ИИ-ассистент ───

/// Настройки ассистента — одни на всё приложение, в `user_profile`, как и
/// прочие. Модель — имя из `/api/tags` Ollama; пустая строка — «не выбрана».
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct AiSettings {
    pub ollama_url: String,
    pub model: String,
    /// Сколько последних сообщений истории уходит модели с каждым вопросом.
    pub context_length: i64,
    /// Сколько ждать один ответ модели, в секундах (по умолчанию 180).
    pub timeout_seconds: i64,
}

/// Сколько сообщений истории отправлять по умолчанию. У локальных моделей
/// окно в 4–8 тысяч токенов, и двадцать реплик в него помещаются с запасом.
pub const DEFAULT_AI_CONTEXT_LENGTH: i64 = 20;
/// Границы «Длины контекста». Меньше двух — модель не увидит даже своего
/// прошлого ответа; больше двухсот — переполнит окно любой локальной модели.
pub const AI_CONTEXT_LENGTH_MIN: i64 = 2;
pub const AI_CONTEXT_LENGTH_MAX: i64 = 200;

/// Реплика чата ассистента. История своя у каждого пространства.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct ChatMessage {
    pub id: i64,
    pub workspace_id: i64,
    /// `user` или `assistant`. `system` схема допускает, но в историю его не
    /// пишут: системный промпт зашит в коде и подставляется при каждом вопросе.
    pub role: String,
    pub content: String,
    /// UTC, `datetime('now')` — как все отметки времени в базе.
    pub created_at: String,
    /// Инструменты, которые модель вызвала ради этого ответа, по порядку.
    /// У реплик человека и ответов без инструментов — пусто.
    pub tools_used: Vec<String>,
    /// Изменения, которые ассистент предлагал по ходу ответа, и что с ними
    /// стало. Остаются в истории: по ним видно, что в базе поменялось и кто
    /// это подтвердил.
    pub actions: Vec<AiActionLog>,
}

/// Одно предложенное ассистентом изменение и его судьба.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct AiActionLog {
    /// То же описание, что человек видел в превью.
    pub description: String,
    /// `done` — выполнено, `cancelled` — человек отказался, `failed` —
    /// подтвердил, но выполнить не вышло (данные успели измениться).
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Изменение, которое ждёт «Да» или «Отмена».
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct AiPendingAction {
    /// Номер ожидания: подтверждение со старым номером не выполнит новое
    /// действие, если между ними что-то поменялось.
    pub id: i64,
    /// Вопрос человека, на который идёт ответ, — чат показывает его, пока ждёт.
    pub user_text: String,
    pub tool: String,
    pub description: String,
    /// О чём стоит знать до «Да»: необратимость, блокирующие задачи,
    /// остановка другого таймера.
    pub warnings: Vec<String>,
}

/// Итог одного обращения к ассистенту: либо ответ готов и записан в историю
/// (`messages` — вопрос и ответ), либо ассистент ждёт подтверждения
/// (`pending`), и ничего ещё не записано.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct ChatTurn {
    pub messages: Vec<ChatMessage>,
    pub pending: Option<AiPendingAction>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserProfile {
    pub avatar_initials: String,
    pub display_name: String,
    pub theme: String,
}

// ─── Board notes ───

/// Markdown-заметки доски.
///
/// `updated_at` необязателен намеренно: у доски, которой ещё ни разу не
/// сохраняли заметку, строки в `board_notes` нет, и `get_board_notes`
/// возвращает пустой текст без времени. Иначе пришлось бы либо выдумывать
/// отметку, либо создавать строку на чтении — а чтение писать в базу не
/// должно.
#[derive(Debug, Serialize, Deserialize)]
pub struct BoardNotes {
    pub board_id: i64,
    pub content: String,
    pub updated_at: Option<String>,
}

// ─── Members ───
//
// A local directory of people used purely as a label on cards. There are no
// accounts, no passwords and no sync: the app stays a single offline SQLite
// file, exactly as before.

/// Avatar background colours, handed out round-robin as members are created.
/// Fixed and small on purpose — a random colour per member produces muddy,
/// indistinguishable circles once there are more than a few.
pub const MEMBER_COLORS: [&str; 8] = [
    "#6366f1", // indigo
    "#ec4899", // pink
    "#f59e0b", // amber
    "#10b981", // emerald
    "#3b82f6", // blue
    "#8b5cf6", // violet
    "#ef4444", // red
    "#14b8a6", // teal
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Member {
    pub id: i64,
    pub name: String,
    pub initials: String,
    pub color: String,
    /// True for the single row representing the user of this installation.
    pub is_self: bool,
    pub created_at: String,
}

// ─── Зависимости между карточками ───

/// Карточка на другом конце зависимости — ровно столько, сколько нужно, чтобы
/// нарисовать строку и дойти до неё кликом.
///
/// Полной `Card` здесь быть не должно: список зависимостей рисует название,
/// место и состояние, а описание, чек-лист и метки к нему не относятся.
#[derive(Debug, Serialize, Deserialize)]
pub struct DependencyCard {
    /// Id строки в `card_dependencies` — им же связь и снимают.
    pub link_id: i64,
    pub card_id: i64,
    pub title: String,
    pub board_id: i64,
    pub board_name: String,
    pub column_name: String,
    /// Карточка лежит в финальной колонке, то есть доведена до конца.
    pub is_done: bool,
    /// Карточка убрана в архив — сама, вместе с колонкой или с доской. Связь
    /// при архивации не удаляется (§31.1), но пока карточка в архиве, она
    /// ничего не блокирует.
    pub is_archived: bool,
}

/// Обе стороны зависимостей одной карточки.
#[derive(Debug, Serialize, Deserialize)]
pub struct CardDependencies {
    /// Кого блокирует эта карточка.
    pub blocking: Vec<DependencyCard>,
    /// Кто блокирует эту карточку.
    pub blocked_by: Vec<DependencyCard>,
}

// ─── Нагрузка по исполнителям ───

/// Строка виджета «Нагрузка»: участник и его открытые задачи в пространстве.
///
/// Справочник участников общий на всё приложение (у `members` нет
/// `workspace_id`), поэтому пространством ограничены **карточки**, а не люди:
/// один и тот же человек в разных пространствах имеет разную нагрузку.
///
/// `low + medium + high` всегда равно `total`: у карточки со снятым
/// приоритетом он читается как `Medium`, ровно как в «Списке».
#[derive(Debug, Serialize, Deserialize)]
pub struct WorkloadRow {
    pub member: Member,
    pub total: i64,
    pub low: i64,
    pub medium: i64,
    pub high: i64,
}

// ─── Учёт времени ───

/// Одна сессия работы над карточкой — с таймера или вписанная руками.
///
/// `ended_at` и `duration_seconds` пусты, пока таймер идёт. Участник приезжает
/// целиком, как автор комментария: строка лога рисует его кружок, и отдельный
/// поход за инициалами был бы лишним. `None` — участника удалили; время при
/// этом остаётся на карточке.
#[derive(Debug, Serialize, Deserialize)]
pub struct TimeEntry {
    pub id: i64,
    pub card_id: i64,
    pub member: Option<Member>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub duration_seconds: Option<i64>,
    pub note: Option<String>,
}

/// Идущий таймер — то, что рисует индикатор в шапке и что показывает диалог
/// про забытый таймер. Название карточки приезжает сразу: без него индикатор
/// не может сказать, *что* идёт.
#[derive(Debug, Serialize, Deserialize)]
pub struct ActiveTimer {
    pub entry_id: i64,
    pub card_id: i64,
    pub card_title: String,
    pub member_id: Option<i64>,
    pub started_at: String,
    /// Секунд с запуска на момент чтения, по часам SQLite. Диалогу про
    /// забытый таймер нужно готовое «запущен N часов назад», а тикающий
    /// счётчик в шапке считает сам от `started_at`.
    pub elapsed_seconds: i64,
}

/// Таймер, который `start_timer` остановил сам: у того же человека уже шёл
/// другой. Приезжает, чтобы фронтенд мог сказать об этом тостом, — молча
/// закрытая сессия выглядела бы как потерянное время.
#[derive(Debug, Serialize, Deserialize)]
pub struct StoppedTimer {
    pub entry_id: i64,
    pub card_id: i64,
    pub card_title: String,
    pub duration_seconds: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StartTimerResult {
    pub timer: ActiveTimer,
    pub stopped: Option<StoppedTimer>,
}

// ─── Пользовательские поля ───

/// Допустимые типы поля — те же, что в `CHECK` схемы. Хранятся по-английски,
/// подписи рисует интерфейс, как у приоритета.
pub const CUSTOM_FIELD_TYPES: [&str; 4] = ["text", "number", "date", "select"];

/// Поле доски вместе со значениями на её карточках.
///
/// Значения приезжают внутри поля, а не отдельным списком с id поля в каждой
/// строке: читают их всегда в связке «какое поле → что на какой карточке».
#[derive(Debug, Serialize, Deserialize)]
pub struct CustomField {
    pub id: i64,
    pub board_id: i64,
    pub name: String,
    /// Одно из `CUSTOM_FIELD_TYPES`.
    pub field_type: String,
    /// Варианты выбора в порядке ввода — только у `select`, у прочих пусто.
    /// В базе — JSON-массив строк, наружу — готовым списком.
    pub select_options: Vec<String>,
    pub position: i64,
    pub values: Vec<CustomFieldValue>,
}

/// Значение поля на одной карточке. Карточки без значения здесь нет вовсе:
/// пустого значения в базе не бывает.
#[derive(Debug, Serialize, Deserialize)]
pub struct CustomFieldValue {
    pub card_id: i64,
    pub value: String,
}

/// Причина автоматической архивации: попытки исчерпаны, а срок опять прошёл.
/// Хранится строкой в `cards.archive_reason`; интерфейс рисует свой ярлык.
pub const ARCHIVE_REASON_MAX_RETRIES: &str = "incomplete_max_retries";

/// Allowed values of `cards.priority`, matching the CHECK constraint in the
/// schema. Stored in English; the interface renders its own Russian labels.
pub const PRIORITIES: [&str; 3] = ["Low", "Medium", "High"];

// ─── Workspace-wide card list (the "Список" screen) ───

/// One row of the list screen: a card plus everything needed to render and edit
/// it without a second query — which board and column it sits in, and the full
/// member records for its assignee and author.
#[derive(Debug, Serialize, Deserialize)]
pub struct CardRow {
    pub id: i64,
    pub title: String,
    pub description: String,
    pub position: i64,
    pub due_date: Option<String>,
    pub priority: String,
    pub created_at: String,
    pub is_mistake: bool,
    pub archived: i8,
    /// Почему карточка в архиве: `incomplete_max_retries` — исчерпаны
    /// попытки, `None` — убрал человек руками.
    pub archive_reason: Option<String>,
    pub column_id: i64,
    pub column_name: String,
    pub board_id: i64,
    pub board_name: String,
    pub board_is_system: bool,
    pub assignee: Option<Member>,
    pub author: Option<Member>,
}

/// A board reduced to what the list screen's status dropdown needs: its own
/// columns, so each row can offer the columns of *its* board rather than a
/// merged list from every board in the workspace.
#[derive(Debug, Serialize, Deserialize)]
pub struct BoardColumns {
    pub id: i64,
    pub name: String,
    pub is_system: bool,
    pub columns: Vec<Column>,
}

/// Everything the list screen needs, in one IPC round trip.
#[derive(Debug, Serialize, Deserialize)]
pub struct WorkspaceCardList {
    pub cards: Vec<CardRow>,
    pub boards: Vec<BoardColumns>,
    /// Пользовательские поля видимых досок пространства со значениями — из них
    /// «Список» собирает необязательные столбцы.
    pub fields: Vec<CustomField>,
}

// ─── Board export / import ───
//
// A self-contained snapshot of one board. Database ids are deliberately NOT
// reused on import (they would collide with existing rows); labels instead get
// export-local ids that cards reference and the importer remaps.
//
// Every field that a future version might add is `#[serde(default)]`, so an
// export written by an older build still imports cleanly.

/// Bumped whenever the shape below changes incompatibly.
pub const EXPORT_FORMAT_VERSION: i64 = 1;

#[derive(Debug, Serialize, Deserialize)]
pub struct BoardExport {
    pub taskflow_export_version: i64,
    pub exported_at: String,
    pub board: BoardExportBody,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BoardExportBody {
    pub name: String,
    #[serde(default)]
    pub gradient: String,
    #[serde(default)]
    pub is_starred: bool,
    #[serde(default)]
    pub labels: Vec<LabelExport>,
    /// Members referenced by this board's cards. Like labels, these carry
    /// export-local ids: a raw database id means nothing in another install,
    /// where the same number belongs to a different person.
    #[serde(default)]
    pub members: Vec<MemberExport>,
    #[serde(default)]
    pub columns: Vec<ColumnExport>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MemberExport {
    /// Export-local id, referenced by `CardExport::assignee_id` / `author_id`.
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub initials: String,
    #[serde(default)]
    pub color: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LabelExport {
    /// Export-local id, referenced by `CardExport::label_ids`.
    pub id: i64,
    #[serde(default)]
    pub name: String,
    pub color: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ColumnExport {
    pub name: String,
    #[serde(default)]
    pub position: i64,
    #[serde(default)]
    pub archived: i8,
    /// Финальность колонки переносится вместе с доской: без неё
    /// перенесённая доска молча теряла бы правило, ради которого её так
    /// настроили. Как и `checklist`/`comments`, поле добавлено позже формата,
    /// поэтому `default` — файл без него читается как «обычная колонка».
    #[serde(default)]
    pub is_final: bool,
    /// То же и для обязательности: иначе после переезда доски костяк
    /// оказался бы незащищённым.
    #[serde(default)]
    pub is_required: bool,
    #[serde(default)]
    pub cards: Vec<CardExport>,
}

/// Один пункт чек-листа в файле экспорта.
///
/// Своего id здесь нет намеренно: на пункт никто не ссылается, в отличие от
/// меток и участников, — он целиком принадлежит своей карточке и восстановить
/// его можно как есть.
#[derive(Debug, Serialize, Deserialize)]
pub struct ChecklistItemExport {
    pub text: String,
    #[serde(default)]
    pub is_done: bool,
    #[serde(default)]
    pub position: i64,
}

/// Комментарий в файле экспорта.
///
/// `author_id` — экспорт-локальный идентификатор из `BoardExportBody::members`,
/// как у `CardExport::assignee_id`: настоящий id участника в другой установке
/// принадлежит другому человеку. `None` — автор был удалён ещё до экспорта.
///
/// Время создания переносится как есть: комментарий, написанный в марте, не
/// должен превратиться в сегодняшний оттого, что доску перенесли.
#[derive(Debug, Serialize, Deserialize)]
pub struct CommentExport {
    pub body: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub author_id: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CardExport {
    pub title: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub position: i64,
    #[serde(default)]
    pub due_date: Option<String>,
    #[serde(default)]
    pub archived: i8,
    #[serde(default)]
    pub is_mistake: bool,
    #[serde(default)]
    pub mistake_marked_at: Option<String>,
    #[serde(default)]
    pub mistake_resolved_at: Option<String>,
    /// Число потраченных попыток и причина архивации переносятся вместе с
    /// карточкой: без них перенесённая задача получала бы три попытки заново,
    /// а «не выполнено» превращалось бы в «убрали руками».
    #[serde(default)]
    pub retry_count: i64,
    #[serde(default)]
    pub archive_reason: Option<String>,
    #[serde(default)]
    pub label_ids: Vec<i64>,
    /// Export-local member ids (see `BoardExportBody::members`), not database
    /// ids. `None` in a file written before members existed.
    #[serde(default)]
    pub assignee_id: Option<i64>,
    #[serde(default)]
    pub author_id: Option<i64>,
    /// Absent in older exports; those cards import at the schema default.
    #[serde(default)]
    pub priority: Option<String>,
    /// Пункты чек-листа. Появились позже формата, поэтому `default`: файл без
    /// них — это просто карточка без подзадач, а не ошибка чтения. Версия
    /// формата ради этого не поднималась (как и для `members`/`priority`):
    /// иначе старая сборка отвергла бы весь файл целиком вместо того, чтобы
    /// прочитать его без чек-листов.
    #[serde(default)]
    pub checklist: Vec<ChecklistItemExport>,
    /// Комментарии. Как и чек-листы, добавлены позже формата — отсюда
    /// `default`. Версия формата не поднималась по той же причине: старая
    /// сборка должна прочитать файл без комментариев, а не отвергнуть его.
    #[serde(default)]
    pub comments: Vec<CommentExport>,
}

/// Result of a full database export, so the Settings screen can confirm what
/// actually landed in the file rather than just claiming success.
/// Counts are reported twice on purpose. The raw row totals are what makes this
/// a backup — archived boards and the hidden Inbox boards are the user's data
/// too, and they are all in the file. But a message saying "29 boards" to
/// someone who sees 9 on their hub reads like a bug, so the visible figure
/// leads and the total explains itself.
#[derive(Debug, Serialize, Deserialize)]
pub struct DatabaseExport {
    pub path: String,
    pub size_bytes: u64,
    pub boards: i64,
    pub boards_active: i64,
    pub cards: i64,
    pub cards_active: i64,
    pub members: i64,
}

/// One automatic backup file, as shown on the Settings screen.
#[derive(Debug, Serialize, Deserialize)]
pub struct BackupInfo {
    pub file_name: String,
    pub size_bytes: u64,
    /// Timestamp parsed out of the file name (`YYYY-MM-DD HH:MM:SS`).
    pub created_at: String,
}
