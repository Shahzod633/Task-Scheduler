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

#[derive(Debug, Serialize, Deserialize)]
pub struct UserProfile {
    pub avatar_initials: String,
    pub display_name: String,
    pub theme: String,
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
