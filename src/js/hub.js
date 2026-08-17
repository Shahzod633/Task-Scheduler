// ============================================
// TaskFlow — Hub View (Dashboard - Screen А)
// ============================================

import * as api from './api.js';
import Icons from './icons.js';
import { TEMPLATE_DEFS, createBoardFromTemplate } from './templates.js';
import { confirmDialog } from './dialog.js';
import { createArchiveRow, createArchiveEmptyState } from './archive.js';
import { openPopover } from './popover.js';
import { createElement, $, $$, showToast, debounce, getRandomGradient, getGradients, escapeHtml, staggerIn } from './utils.js';

let currentWorkspaceId = null;
let showTemplates = true;

const DEFAULT_COLUMNS = ['Новые', 'В работе', 'На проверке', 'Готово'];

/**
 * Render the hub/dashboard view
 */
export async function renderHub(workspaceId) {
    currentWorkspaceId = workspaceId;
    const content = $('#content');
    content.innerHTML = '';
    content.classList.add('view-enter');
    
    const hub = createElement('div', { className: 'hub', id: 'hub-view' });
    
    try {
        const boards = await api.getBoards(workspaceId);
        const workspaces = await api.getWorkspaces();
        const workspace = workspaces.find(w => w.id === workspaceId);
        
        // Workspace header
        if (workspace) {
            hub.appendChild(createWorkspaceHeader(workspace));
        }
        
        // Templates section (dismissible)
        if (showTemplates) {
            hub.appendChild(createTemplatesSection());
        }
        
        // Boards section
        hub.appendChild(createBoardsSection(boards));
        
        // Footer links: archive and import
        const footerLinks = createElement('div', { className: 'hub__footer-links' });

        const closedLink = createElement('div', {
            className: 'hub__closed-boards',
            innerHTML: `${Icons.archive} <span>Посмотреть закрытые доски</span>`
        });
        closedLink.addEventListener('click', () => showArchivedBoards(workspaceId));
        footerLinks.appendChild(closedLink);

        const importLink = createElement('div', {
            className: 'hub__closed-boards',
            innerHTML: `${Icons.upload} <span>Импортировать доску из файла</span>`
        });
        importLink.addEventListener('click', () => importBoardFromDisk(workspaceId));
        footerLinks.appendChild(importLink);

        hub.appendChild(footerLinks);

    } catch (error) {
        console.error('Error loading hub:', error);
        showToast('Ошибка загрузки данных', 'error');
    }
    
    content.appendChild(hub);
    setTimeout(() => content.classList.remove('view-enter'), 420);
}

/**
 * Create workspace header
 */
function createWorkspaceHeader(workspace) {
    const header = createElement('div', { className: 'hub__workspace-header' });
    
    const icon = createElement('div', {
        className: 'hub__workspace-icon',
        style: { background: 'var(--gradient-1)' }
    }, workspace.name.charAt(0).toUpperCase());
    header.appendChild(icon);
    
    const info = createElement('div', { className: 'hub__workspace-info' });
    
    const nameRow = createElement('div', { className: 'hub__workspace-name' });
    nameRow.appendChild(document.createTextNode(workspace.name));
    const editIcon = createElement('span', {
        className: 'hub__workspace-edit',
        innerHTML: Icons.edit
    });
    editIcon.addEventListener('click', () => editWorkspaceName(workspace));
    nameRow.appendChild(editIcon);
    info.appendChild(nameRow);
    
    const meta = createElement('div', { className: 'hub__workspace-meta' });
    meta.appendChild(createElement('span', {
        className: 'hub__workspace-badge',
        innerHTML: `${Icons.lock} Приватная`
    }));
    info.appendChild(meta);
    
    header.appendChild(info);
    return header;
}

/**
 * Edit workspace name
 */
function editWorkspaceName(workspace) {
    const nameEl = $('.hub__workspace-name');
    if (!nameEl) return;
    
    const currentText = workspace.name;
    nameEl.innerHTML = '';
    
    const input = createElement('input', {
        className: 'form-input',
        value: currentText,
        style: { fontSize: 'var(--font-size-xl)', fontWeight: 'var(--font-weight-bold)', width: '300px' }
    });
    
    const save = async () => {
        const newName = input.value.trim();
        if (newName && newName !== currentText) {
            await api.updateWorkspace(workspace.id, newName, workspace.visibility);
            workspace.name = newName;
        }
        renderHub(currentWorkspaceId);
    };
    
    input.addEventListener('blur', save);
    input.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') save();
        if (e.key === 'Escape') renderHub(currentWorkspaceId);
    });
    
    nameEl.appendChild(input);
    input.focus();
    input.select();
}

/**
 * Create templates section
 */
function createTemplatesSection() {
    const section = createElement('div', { className: 'hub__templates', id: 'templates-section' });
    
    const header = createElement('div', { className: 'hub__templates-header' });
    header.appendChild(createElement('h3', { className: 'hub__templates-title' }, 'Начните с шаблона'));
    
    const closeBtn = createElement('button', {
        className: 'hub__templates-close',
        innerHTML: Icons.x
    });
    closeBtn.addEventListener('click', () => {
        showTemplates = false;
        section.remove();
    });
    header.appendChild(closeBtn);
    section.appendChild(header);
    
    const grid = createElement('div', { className: 'hub__templates-grid' });

    Object.keys(TEMPLATE_DEFS).forEach((name, i) => {
        const card = createElement('div', { className: 'hub__template-card' });
        staggerIn(card, i);
        card.appendChild(createElement('span', { className: 'hub__template-name' }, name));
        card.appendChild(createElement('span', {
            className: 'hub__template-icon',
            innerHTML: Icons.arrowUpRight
        }));
        card.addEventListener('click', async () => {
            await createBoardFromTemplate(currentWorkspaceId, name);
        });
        grid.appendChild(card);
    });

    section.appendChild(grid);
    return section;
}

/**
 * Create boards section
 */
function createBoardsSection(boards) {
    const section = createElement('div', { className: 'hub__boards-section' });
    
    const header = createElement('div', { className: 'hub__section-header' });
    header.appendChild(createElement('span', {
        className: 'hub__section-icon',
        innerHTML: Icons.boards
    }));
    header.appendChild(createElement('h3', { className: 'hub__section-title' }, 'Мои доски'));
    section.appendChild(header);
    
    const grid = createElement('div', { className: 'hub__boards-grid', id: 'boards-grid' });
    
    // Board cards
    boards.forEach((board, i) => {
        grid.appendChild(createBoardCard(board, i));
    });

    // Create board card
    grid.appendChild(createNewBoardCard(boards.length));
    
    section.appendChild(grid);
    return section;
}

/**
 * Create a board card.
 * @param {object} board
 * @param {number} index - позиция в сетке; задаёт задержку каскадного появления
 */
export function createBoardCard(board, index = 0) {
    const card = createElement('div', {
        className: 'hub__board-card',
        style: { background: board.gradient },
        dataset: { boardId: board.id }
    });
    staggerIn(card, index);
    
    card.appendChild(createElement('span', { className: 'hub__board-name' }, board.name));
    
    // Star button
    const starBtn = createElement('button', {
        className: `hub__board-star ${board.is_starred ? 'hub__board-star--active' : ''}`,
        innerHTML: board.is_starred ? Icons.starFilled : Icons.star
    });
    starBtn.addEventListener('click', async (e) => {
        e.stopPropagation();
        const newStarred = !board.is_starred;
        await api.updateBoard(board.id, board.name, board.gradient, newStarred);
        board.is_starred = newStarred;
        starBtn.innerHTML = newStarred ? Icons.starFilled : Icons.star;
        starBtn.className = `hub__board-star ${newStarred ? 'hub__board-star--active' : ''}`;
        window.dispatchEvent(new CustomEvent('board-star-toggled', { detail: { boardId: board.id, starred: newStarred } }));
    });
    card.appendChild(starBtn);

    // Board menu — the only way to archive a board. `archive_board` existed in
    // the backend from the start, but nothing in the UI ever called it, so the
    // "Посмотреть закрытые доски" list below could never fill up.
    const menuBtn = createElement('button', {
        className: 'hub__board-menu',
        innerHTML: Icons.moreHorizontal,
        'data-tooltip': 'Меню доски'
    });
    menuBtn.addEventListener('click', (e) => {
        e.stopPropagation();
        showBoardCardMenu(e, board);
    });
    card.appendChild(menuBtn);

    // Navigate to board
    card.addEventListener('click', () => {
        window.dispatchEvent(new CustomEvent('navigate', { detail: { view: 'board', boardId: board.id } }));
    });

    return card;
}

/**
 * Context menu of a board tile on the hub: archiving lives here, mirroring
 * where Trello keeps it.
 */
function showBoardCardMenu(event, board) {
    const existing = $('.context-menu');
    if (existing) existing.remove();

    const menu = createElement('div', { className: 'context-menu' });

    const archiveItem = createElement('div', {
        className: 'context-menu__item context-menu__item--danger',
        innerHTML: `${Icons.archive} <span>Архивировать доску</span>`
    });
    archiveItem.addEventListener('click', async () => {
        menu.remove();
        const ok = await confirmDialog({
            title: 'Архивировать доску?',
            message: `Доска «${board.name}» пропадёт из списка вместе со всеми колонками и карточками. Вернуть её можно через «Посмотреть закрытые доски».`,
            confirmText: 'Архивировать',
            danger: true,
        });
        if (!ok) return;

        try {
            await api.archiveBoard(board.id);
            showToast(`Доска «${board.name}» архивирована`);
            renderHub(currentWorkspaceId);
        } catch (e) {
            showToast('Не удалось архивировать доску', 'error');
        }
    });
    menu.appendChild(archiveItem);

    openPopover(menu, event.currentTarget, { placement: 'bottom', align: 'end', gap: 4 });
}

/**
 * Create the "new board" card
 */
function createNewBoardCard(index = 0) {
    const card = createElement('div', {
        className: 'hub__create-board',
        id: 'create-board-btn'
    });
    staggerIn(card, index);
    
    card.appendChild(createElement('span', {
        className: 'hub__create-board-icon',
        innerHTML: Icons.plus
    }));
    card.appendChild(createElement('span', { className: 'hub__create-board-text' }, 'Создать доску'));
    
    card.addEventListener('click', () => showCreateBoardModal());
    
    return card;
}

/**
 * Show create board modal
 */
function showCreateBoardModal() {
    const existing = $('.modal-overlay');
    if (existing) existing.remove();
    
    const overlay = createElement('div', { className: 'modal-overlay' });
    const modal = createElement('div', { className: 'modal' });
    
    // Header
    const header = createElement('div', { className: 'modal__header' });
    header.appendChild(createElement('h2', { className: 'modal__title' }, 'Создать доску'));
    const closeBtn = createElement('button', { className: 'modal__close', innerHTML: Icons.x });
    closeBtn.addEventListener('click', () => overlay.remove());
    header.appendChild(closeBtn);
    modal.appendChild(header);
    
    // Body
    const body = createElement('div', { className: 'modal__body' });
    
    // Board name
    const nameGroup = createElement('div', { className: 'form-group' });
    nameGroup.appendChild(createElement('label', { className: 'form-label' }, 'Название доски'));
    const nameInput = createElement('input', {
        className: 'form-input',
        placeholder: 'Введите название...',
        id: 'new-board-name'
    });
    nameGroup.appendChild(nameInput);
    body.appendChild(nameGroup);
    
    // Gradient picker
    const gradGroup = createElement('div', { className: 'form-group' });
    gradGroup.appendChild(createElement('label', { className: 'form-label' }, 'Фон'));
    const picker = createElement('div', { className: 'gradient-picker' });
    
    let selectedGradient = getRandomGradient();
    const gradients = getGradients();
    
    for (const grad of gradients) {
        const item = createElement('div', {
            className: `gradient-picker__item ${grad === selectedGradient ? 'gradient-picker__item--selected' : ''}`,
            style: { background: grad }
        });
        item.addEventListener('click', () => {
            selectedGradient = grad;
            $$('.gradient-picker__item', picker).forEach(i => i.classList.remove('gradient-picker__item--selected'));
            item.classList.add('gradient-picker__item--selected');
        });
        picker.appendChild(item);
    }
    
    gradGroup.appendChild(picker);
    body.appendChild(gradGroup);
    
    modal.appendChild(body);
    
    // Footer
    const footer = createElement('div', { className: 'modal__footer' });
    const createBtn = createElement('button', { className: 'btn btn--primary' }, 'Создать');
    createBtn.addEventListener('click', async () => {
        const name = nameInput.value.trim();
        if (!name) {
            nameInput.style.borderColor = 'var(--color-danger)';
            return;
        }
        
        try {
            const board = await api.createBoard(currentWorkspaceId, name, selectedGradient);
            for (const colName of DEFAULT_COLUMNS) {
                await api.createColumn(board.id, colName);
            }
            overlay.remove();
            showToast(`Доска "${name}" создана`);
            window.dispatchEvent(new CustomEvent('navigate', { detail: { view: 'board', boardId: board.id } }));
        } catch (e) {
            showToast('Ошибка создания доски', 'error');
        }
    });
    footer.appendChild(createBtn);
    modal.appendChild(footer);
    
    overlay.appendChild(modal);
    overlay.addEventListener('click', (e) => {
        if (e.target === overlay) overlay.remove();
    });
    
    document.body.appendChild(overlay);
    nameInput.focus();
    
    nameInput.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') createBtn.click();
    });
}

/**
 * Show archived boards
 */
async function showArchivedBoards(workspaceId) {
    const existing = $('.modal-overlay');
    if (existing) existing.remove();

    const overlay = createElement('div', { className: 'modal-overlay' });
    const modal = createElement('div', { className: 'modal modal--wide' });

    const header = createElement('div', { className: 'modal__header' });
    header.appendChild(createElement('h2', { className: 'modal__title' }, 'Закрытые доски'));
    const closeBtn = createElement('button', { className: 'modal__close', innerHTML: Icons.x });
    closeBtn.addEventListener('click', () => overlay.remove());
    header.appendChild(closeBtn);
    modal.appendChild(header);

    const body = createElement('div', { className: 'modal__body' });
    const list = createElement('div', { className: 'archive-list' });
    body.appendChild(list);
    modal.appendChild(body);

    overlay.appendChild(modal);
    overlay.addEventListener('click', (e) => {
        if (e.target === overlay) overlay.remove();
    });
    document.body.appendChild(overlay);

    /** Re-reads the archive so the list stays correct after each action. */
    async function refresh() {
        list.innerHTML = '';
        let archived;
        try {
            archived = await api.getArchivedBoards(workspaceId);
        } catch (e) {
            list.appendChild(createArchiveEmptyState('Не удалось загрузить архив'));
            return;
        }

        if (archived.length === 0) {
            list.appendChild(createArchiveEmptyState('Нет закрытых досок'));
            return;
        }

        for (const board of archived) {
            list.appendChild(createArchiveRow({
                title: board.name,
                onRestore: async () => {
                    try {
                        await api.restoreBoard(board.id);
                        showToast(`Доска «${board.name}» восстановлена`);
                        renderHub(workspaceId);
                        refresh();
                    } catch (e) {
                        showToast('Не удалось восстановить доску', 'error');
                    }
                },
                onDelete: async () => {
                    const ok = await confirmDialog({
                        title: 'Удалить доску навсегда?',
                        message: `Доска «${board.name}» со всеми колонками, карточками и метками будет удалена без возможности восстановления.`,
                        confirmText: 'Удалить навсегда',
                        danger: true,
                    });
                    if (!ok) return;
                    try {
                        await api.deleteBoard(board.id);
                        showToast('Доска удалена');
                        renderHub(workspaceId);
                        refresh();
                    } catch (e) {
                        console.error('Failed to delete board:', e);
                        showToast('Не удалось удалить доску', 'error');
                    }
                },
            }));
        }
    }

    refresh();
}

/**
 * Imports a board from a `.json` export produced by the board's Экспорт button.
 */
async function importBoardFromDisk(workspaceId) {
    try {
        const path = await api.showOpenDialog();
        if (!path) return; // cancelled

        // The dialog plugin returns a string for single selection, but guard
        // against an array in case that ever changes.
        const filePath = Array.isArray(path) ? path[0] : path;
        const board = await api.importBoardFromFile(workspaceId, filePath);

        showToast(`Доска «${board.name}» импортирована`);
        renderHub(workspaceId);
    } catch (e) {
        console.error('Board import failed:', e);
        // Backend errors are already user-readable ("Файл не похож на экспорт…").
        showToast(typeof e === 'string' ? e : 'Не удалось импортировать доску', 'error');
    }
}

/**
 * Filter boards by search query
 */
export function filterBoards(query) {
    const cards = $$('.hub__board-card');
    const lowerQuery = query.toLowerCase();
    
    for (const card of cards) {
        const name = $('.hub__board-name', card);
        if (name) {
            const matches = name.textContent.toLowerCase().includes(lowerQuery);
            card.style.display = matches ? '' : 'none';
        }
    }
}

export function getCurrentWorkspaceId() {
    return currentWorkspaceId;
}
