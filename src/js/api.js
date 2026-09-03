// ============================================
// TaskFlow — API Layer (Tauri IPC Wrappers)
// ============================================
// All data operations go through Tauri invoke commands.
// This module provides a clean JS API over the Rust backend.

const { invoke } = window.__TAURI__.core;

// ─── Workspaces ───

export async function getWorkspaces() {
    return await invoke('get_workspaces');
}

export async function createWorkspace(name) {
    return await invoke('create_workspace', { name });
}

export async function updateWorkspace(id, name, visibility) {
    return await invoke('update_workspace', { id, name, visibility });
}

export async function archiveWorkspace(id) {
    return await invoke('archive_workspace', { id });
}

// ─── Sidebar background (per workspace) ───

/**
 * Imports the picture at `sourcePath` as this workspace's sidebar background.
 *
 * The backend copies it into the app's own folder after shrinking it — the
 * original is never referenced, so moving or unplugging it later changes
 * nothing. Any picture the workspace had before is deleted.
 *
 * @returns {Promise<string>} stored file name
 */
export async function setWorkspaceBackground(workspaceId, sourcePath) {
    return await invoke('set_workspace_background', { workspaceId, sourcePath });
}

export async function clearWorkspaceBackground(workspaceId) {
    return await invoke('clear_workspace_background', { workspaceId });
}

/**
 * The workspace's background as a `data:` URL, or null if it has none.
 * Go through `background.js` rather than calling this directly — it caches.
 */
export async function getWorkspaceBackground(workspaceId) {
    return await invoke('get_workspace_background', { workspaceId });
}

// ─── Boards ───

export async function getBoards(workspaceId) {
    return await invoke('get_boards', { workspaceId });
}

export async function getBoard(id) {
    return await invoke('get_board', { id });
}

export async function createBoard(workspaceId, name, gradient) {
    return await invoke('create_board', { workspaceId, name, gradient });
}

export async function updateBoard(id, name, gradient, isStarred) {
    return await invoke('update_board', { id, name, gradient, isStarred });
}

export async function archiveBoard(id) {
    return await invoke('archive_board', { id });
}

export async function getArchivedBoards(workspaceId) {
    return await invoke('get_archived_boards', { workspaceId });
}

export async function restoreBoard(id) {
    return await invoke('restore_board', { id });
}

// ─── Columns ───

export async function getColumns(boardId) {
    return await invoke('get_columns', { boardId });
}

export async function createColumn(boardId, name) {
    return await invoke('create_column', { boardId, name });
}

export async function updateColumn(id, name) {
    return await invoke('update_column', { id, name });
}

export async function reorderColumns(boardId, columnIds) {
    return await invoke('reorder_columns', { boardId, columnIds });
}

export async function archiveColumn(id) {
    return await invoke('archive_column', { id });
}

/**
 * Помечает колонку финальной (или снимает пометку).
 *
 * Карточка, доехавшая до финальной колонки, обратно уже не уезжает — это
 * проверяет `update_card_position` на бэкенде, а не только интерфейс.
 */
export async function setColumnFinal(id, isFinal) {
    return await invoke('set_column_final', { id, isFinal });
}

// ─── Cards ───

export async function getCards(columnId) {
    return await invoke('get_cards', { columnId });
}

export async function createCard(columnId, title, description) {
    return await invoke('create_card', { columnId, title, description: description || '' });
}

export async function updateCard(id, title, description, dueDate) {
    return await invoke('update_card', { id, title, description, dueDate });
}

export async function updateCardPosition(id, newColumnId, newPosition) {
    return await invoke('update_card_position', { id, newColumnId, newPosition });
}

export async function archiveCard(id) {
    return await invoke('archive_card', { id });
}

// ─── Checklists (sub-tasks inside a card) ───

export async function listChecklistItems(cardId) {
    return await invoke('list_checklist_items', { cardId });
}

export async function createChecklistItem(cardId, text) {
    return await invoke('create_checklist_item', { cardId, text });
}

/** Flips one item; resolves to its new `is_done` state. */
export async function toggleChecklistItem(id) {
    return await invoke('toggle_checklist_item', { id });
}

export async function deleteChecklistItem(id) {
    return await invoke('delete_checklist_item', { id });
}

// ─── Members ───
// A local directory of names, used only as a label on cards. No accounts, no
// passwords, no network — the app stays a single offline SQLite file.

export async function listMembers() {
    return await invoke('list_members');
}

export async function createMember(name) {
    return await invoke('create_member', { name });
}

/** `initials` may be null to re-derive them from the name. */
export async function updateMember(id, name, color, initials = null) {
    return await invoke('update_member', { id, name, color, initials });
}

export async function deleteMember(id) {
    return await invoke('delete_member', { id });
}

// ─── Assignment and priority ───

/** Pass `null` as memberId to clear the assignee. */
export async function updateCardAssignee(cardId, memberId) {
    return await invoke('update_card_assignee', { cardId, memberId });
}

export async function updateCardAuthor(cardId, memberId) {
    return await invoke('update_card_author', { cardId, memberId });
}

/** `priority` is one of 'Low' | 'Medium' | 'High'. */
export async function updateCardPriority(cardId, priority) {
    return await invoke('update_card_priority', { cardId, priority });
}

// ─── Workspace-wide card list (the "Список" screen) ───

/**
 * Every non-archived card across every board of the workspace, plus each
 * board's columns — in one call, so the screen never fans out into a query
 * per board.
 *
 * @returns {Promise<{cards: object[], boards: object[]}>}
 */
export async function listAllCardsInWorkspace(workspaceId) {
    return await invoke('list_all_cards_in_workspace', { workspaceId });
}

// ─── Labels ───

export async function getLabels(boardId) {
    return await invoke('get_labels', { boardId });
}

export async function createLabel(boardId, name, color) {
    return await invoke('create_label', { boardId, name, color });
}

export async function addLabelToCard(cardId, labelId) {
    return await invoke('add_label_to_card', { cardId, labelId });
}

export async function removeLabelFromCard(cardId, labelId) {
    return await invoke('remove_label_from_card', { cardId, labelId });
}

// ─── Export/Import ───

/** Returns the board as a JSON string (no file is written). */
export async function exportBoard(boardId) {
    return await invoke('export_board', { boardId });
}

/** Writes the board's JSON to `path`; returns the path written. */
export async function exportBoardToFile(boardId, path) {
    return await invoke('export_board_to_file', { boardId, path });
}

export async function importBoard(workspaceId, jsonData) {
    return await invoke('import_board', { workspaceId, jsonData });
}

/** Reads a `.json` export from `path` and imports it as a new board. */
export async function importBoardFromFile(workspaceId, path) {
    return await invoke('import_board_from_file', { workspaceId, path });
}

// ─── Archive (restore) and permanent deletion ───
// The archive doubles as the trash: restore brings an item back, delete* wipes
// it for good. Only boards had this before; cards and columns were a one-way
// trip.

export async function getArchivedColumns(boardId) {
    return await invoke('get_archived_columns', { boardId });
}

export async function getArchivedCards(boardId) {
    return await invoke('get_archived_cards', { boardId });
}

export async function restoreCard(id) {
    return await invoke('restore_card', { id });
}

export async function restoreColumn(id) {
    return await invoke('restore_column', { id });
}

export async function deleteCard(id) {
    return await invoke('delete_card', { id });
}

export async function deleteColumn(id) {
    return await invoke('delete_column', { id });
}

export async function deleteBoard(id) {
    return await invoke('delete_board', { id });
}

// ─── Backups ───

/**
 * Writes a consistent snapshot of the entire database to `path`.
 *
 * Not a file copy: the backend uses SQLite's `VACUUM INTO`, so the result is a
 * complete database rather than whatever bytes happened to be on disk.
 *
 * @returns {Promise<{path: string, size_bytes: number, boards: number, cards: number, members: number}>}
 */
export async function exportDatabase(path) {
    return await invoke('export_database', { path });
}

/** Dated default file name for the save dialog, e.g. `taskflow-2026-08-22.db`. */
export async function suggestExportName() {
    return await invoke('suggest_export_name');
}

export async function getBackups() {
    return await invoke('get_backups');
}

export async function getBackupDir() {
    return await invoke('get_backup_dir');
}

export async function openBackupDir() {
    return await invoke('open_backup_dir');
}

export async function getAppVersion() {
    return await invoke('get_app_version');
}

// ─── Native file dialogs (tauri-plugin-dialog) ───
// Called through `invoke` rather than the plugin's npm package: the project has
// no bundler, so pulling in @tauri-apps/plugin-dialog would mean adding one.
// The command names below are the plugin's own IPC contract.

/**
 * Native "save file" dialog. Returns the chosen path, or null if cancelled.
 */
export async function showSaveDialog(defaultPath, filterName = 'JSON', extensions = ['json']) {
    return await invoke('plugin:dialog|save', {
        options: { defaultPath, filters: [{ name: filterName, extensions }] }
    });
}

/**
 * Native "open file" dialog. Returns the chosen path, or null if cancelled.
 */
export async function showOpenDialog(filterName = 'JSON', extensions = ['json']) {
    return await invoke('plugin:dialog|open', {
        options: { multiple: false, directory: false, filters: [{ name: filterName, extensions }] }
    });
}

// ─── Notifications ───

export async function getNotifications() {
    return await invoke('get_notifications');
}

export async function markAllNotificationsRead() {
    return await invoke('mark_all_notifications_read');
}

// ─── User profile ───

export async function getUserProfile() {
    return await invoke('get_user_profile');
}

export async function updateUserProfile(displayName, avatarInitials, theme) {
    return await invoke('update_user_profile', { displayName, avatarInitials, theme });
}

// ─── Recently viewed boards ───

export async function recordBoardView(boardId) {
    return await invoke('record_board_view', { boardId });
}

export async function getRecentBoards(workspaceId, limit) {
    return await invoke('get_recent_boards', { workspaceId, limit });
}

// ─── Inbox ───

export async function getInboxColumn(workspaceId) {
    return await invoke('get_inbox_column', { workspaceId });
}

// ─── Planner ───

export async function getCardsWithDueDates(workspaceId) {
    return await invoke('get_cards_with_due_dates', { workspaceId });
}

// ─── Комментарии к карточке ───

export async function listCardComments(cardId) {
    return await invoke('list_card_comments', { cardId });
}

export async function createCardComment(cardId, body) {
    return await invoke('create_card_comment', { cardId, body });
}

export async function deleteCardComment(id) {
    return await invoke('delete_card_comment', { id });
}

// ─── Напоминания о дедлайнах ───

export async function getReminderSettings() {
    return await invoke('get_reminder_settings');
}

export async function updateReminderSettings(enabled, hours) {
    return await invoke('update_reminder_settings', { enabled, hours });
}

// ─── Mistake tracking ───

export async function markCardMistake(cardId) {
    return await invoke('mark_card_mistake', { cardId });
}

export async function resolveCardMistake(cardId) {
    return await invoke('resolve_card_mistake', { cardId });
}

export async function getMistakeCards(workspaceId) {
    return await invoke('get_mistake_cards', { workspaceId });
}
