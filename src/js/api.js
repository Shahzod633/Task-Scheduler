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

export async function moveCard(id, newColumnId, newPosition) {
    return await invoke('move_card', { id, newColumnId, newPosition });
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
