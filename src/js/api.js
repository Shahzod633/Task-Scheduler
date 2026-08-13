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

export async function reorderCards(columnId, cardIds) {
    return await invoke('reorder_cards', { columnId, cardIds });
}

export async function archiveCard(id) {
    return await invoke('archive_card', { id });
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

export async function exportBoard(boardId) {
    return await invoke('export_board', { boardId });
}

export async function importBoard(workspaceId, jsonData) {
    return await invoke('import_board', { workspaceId, jsonData });
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
