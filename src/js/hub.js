// ============================================
// TaskFlow — Hub View (Dashboard - Screen А)
// ============================================

import * as api from './api.js';
import Icons from './icons.js';
import { TEMPLATE_DEFS, createBoardFromTemplate } from './templates.js';
import { createElement, $, $$, showToast, debounce, getRandomGradient, getGradients, escapeHtml } from './utils.js';

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
        
        // Closed boards link
        const closedLink = createElement('div', {
            className: 'hub__closed-boards',
            innerHTML: `${Icons.archive} <span>Посмотреть закрытые доски</span>`
        });
        closedLink.addEventListener('click', () => showArchivedBoards(workspaceId));
        hub.appendChild(closedLink);
        
    } catch (error) {
        console.error('Error loading hub:', error);
        showToast('Ошибка загрузки данных', 'error');
    }
    
    content.appendChild(hub);
    setTimeout(() => content.classList.remove('view-enter'), 250);
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

    for (const name of Object.keys(TEMPLATE_DEFS)) {
        const card = createElement('div', { className: 'hub__template-card' });
        card.appendChild(createElement('span', { className: 'hub__template-name' }, name));
        card.appendChild(createElement('span', {
            className: 'hub__template-icon',
            innerHTML: Icons.arrowUpRight
        }));
        card.addEventListener('click', async () => {
            await createBoardFromTemplate(currentWorkspaceId, name);
        });
        grid.appendChild(card);
    }

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
    for (const board of boards) {
        grid.appendChild(createBoardCard(board));
    }
    
    // Create board card
    grid.appendChild(createNewBoardCard());
    
    section.appendChild(grid);
    return section;
}

/**
 * Create a board card
 */
export function createBoardCard(board) {
    const card = createElement('div', {
        className: 'hub__board-card',
        style: { background: board.gradient },
        dataset: { boardId: board.id }
    });
    
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
    
    // Navigate to board
    card.addEventListener('click', () => {
        window.dispatchEvent(new CustomEvent('navigate', { detail: { view: 'board', boardId: board.id } }));
    });
    
    return card;
}

/**
 * Create the "new board" card
 */
function createNewBoardCard() {
    const card = createElement('div', {
        className: 'hub__create-board',
        id: 'create-board-btn'
    });
    
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
    try {
        const archived = await api.getArchivedBoards(workspaceId);
        
        const existing = $('.modal-overlay');
        if (existing) existing.remove();
        
        const overlay = createElement('div', { className: 'modal-overlay' });
        const modal = createElement('div', { className: 'modal' });
        
        const header = createElement('div', { className: 'modal__header' });
        header.appendChild(createElement('h2', { className: 'modal__title' }, 'Закрытые доски'));
        const closeBtn = createElement('button', { className: 'modal__close', innerHTML: Icons.x });
        closeBtn.addEventListener('click', () => overlay.remove());
        header.appendChild(closeBtn);
        modal.appendChild(header);
        
        const body = createElement('div', { className: 'modal__body' });
        
        if (archived.length === 0) {
            body.appendChild(createElement('p', {
                style: { color: 'var(--text-muted)', textAlign: 'center', padding: '24px 0' }
            }, 'Нет закрытых досок'));
        } else {
            for (const board of archived) {
                const item = createElement('div', {
                    style: {
                        display: 'flex', alignItems: 'center', justifyContent: 'space-between',
                        padding: '8px 12px', borderRadius: '8px', marginBottom: '4px'
                    }
                });
                item.appendChild(createElement('span', {}, board.name));
                const restoreBtn = createElement('button', { className: 'btn btn--secondary' }, 'Восстановить');
                restoreBtn.addEventListener('click', async () => {
                    await api.restoreBoard(board.id);
                    overlay.remove();
                    renderHub(workspaceId);
                    showToast(`Доска "${board.name}" восстановлена`);
                });
                item.appendChild(restoreBtn);
                body.appendChild(item);
            }
        }
        
        modal.appendChild(body);
        overlay.appendChild(modal);
        overlay.addEventListener('click', (e) => {
            if (e.target === overlay) overlay.remove();
        });
        document.body.appendChild(overlay);
    } catch (e) {
        showToast('Ошибка загрузки архива', 'error');
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
