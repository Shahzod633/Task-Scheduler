// ============================================
// TaskFlow — Board View (Kanban Board - Screen В)
// ============================================

import * as api from './api.js';
import Icons from './icons.js';
import { createElement, $, $$, showToast, autoResize, escapeHtml, formatDate, isOverdue } from './utils.js';

let currentBoardId = null;
let columnsData = [];
let columnSortable = null;
let cardSortables = [];

/**
 * Initialize board view for a given board
 */
export async function renderBoard(boardId) {
    currentBoardId = boardId;
    const content = $('#content');
    
    try {
        const board = await api.getBoard(boardId);
        const columns = await api.getColumns(boardId);
        
        // Load cards for each column
        columnsData = [];
        for (const col of columns) {
            const cards = await api.getCards(col.id);
            columnsData.push({ ...col, cards });
        }
        
        content.innerHTML = '';
        content.classList.add('view-enter');
        
        // Board header
        content.appendChild(createBoardHeader(board));
        
        // Kanban board
        const boardEl = createElement('div', { className: 'board', id: 'board-columns' });
        
        // Render columns
        for (const col of columnsData) {
            boardEl.appendChild(createColumnElement(col));
        }
        
        // Add column button
        boardEl.appendChild(createAddColumnElement());
        
        content.appendChild(boardEl);
        
        // Initialize Sortable.js
        initSortable();
        
        setTimeout(() => content.classList.remove('view-enter'), 250);
    } catch (error) {
        console.error('Error loading board:', error);
        showToast('Ошибка загрузки доски', 'error');
    }
}

/**
 * Create board header element
 */
function createBoardHeader(board) {
    const header = createElement('div', { className: 'board-header' });
    
    const left = createElement('div', { className: 'board-header__left' });
    const title = createElement('span', { 
        className: 'board-header__title',
        id: 'board-title'
    }, board.name);
    title.addEventListener('click', () => editBoardTitle(board));
    left.appendChild(title);
    
    const viewBtn = createElement('button', { 
        className: 'board-header__btn',
        innerHTML: Icons.grid,
        'data-tooltip': 'Представление'
    });
    left.appendChild(viewBtn);
    
    const right = createElement('div', { className: 'board-header__right' });
    
    // Power-Ups
    right.appendChild(createElement('button', {
        className: 'board-header__btn',
        innerHTML: `${Icons.puzzle} <span>Power-Ups</span>`,
        'data-tooltip': 'Power-Ups'
    }));
    
    // Automation
    right.appendChild(createElement('button', {
        className: 'board-header__btn',
        innerHTML: `${Icons.zap} <span>Автоматизация</span>`,
        'data-tooltip': 'Автоматизация'
    }));
    
    // Filter
    right.appendChild(createElement('button', {
        className: 'board-header__btn',
        innerHTML: Icons.filter,
        'data-tooltip': 'Фильтры'
    }));
    
    // Star
    const starBtn = createElement('button', {
        className: 'board-header__btn',
        innerHTML: board.is_starred ? Icons.starFilled : Icons.star,
        'data-tooltip': 'Избранное'
    });
    starBtn.addEventListener('click', async () => {
        const newStarred = !board.is_starred;
        await api.updateBoard(board.id, board.name, board.gradient, newStarred);
        board.is_starred = newStarred;
        starBtn.innerHTML = newStarred ? Icons.starFilled : Icons.star;
        starBtn.style.color = newStarred ? 'var(--color-warning)' : '';
    });
    if (board.is_starred) starBtn.style.color = 'var(--color-warning)';
    right.appendChild(starBtn);
    
    // Privacy
    right.appendChild(createElement('button', {
        className: 'board-header__btn',
        innerHTML: `${Icons.lock} <span>Приватная</span>`,
        'data-tooltip': 'Видимость'
    }));
    
    // Share
    const shareBtn = createElement('button', {
        className: 'board-header__btn board-header__btn--share',
        innerHTML: `${Icons.share} <span>Поделиться</span>`
    });
    shareBtn.addEventListener('click', async () => {
        try {
            const json = await api.exportBoard(currentBoardId);
            showToast('Доска экспортирована в JSON', 'success');
        } catch (e) {
            showToast('Ошибка экспорта', 'error');
        }
    });
    right.appendChild(shareBtn);
    
    // More
    right.appendChild(createElement('button', {
        className: 'board-header__btn',
        innerHTML: Icons.moreHorizontal,
        'data-tooltip': 'Меню'
    }));
    
    header.appendChild(left);
    header.appendChild(right);
    return header;
}

/**
 * Edit board title inline
 */
function editBoardTitle(board) {
    const titleEl = $('#board-title');
    const input = createElement('input', {
        className: 'column__title-input',
        value: board.name,
        style: { fontSize: 'var(--font-size-xl)', fontWeight: 'var(--font-weight-bold)' }
    });
    
    const save = async () => {
        const newName = input.value.trim();
        if (newName && newName !== board.name) {
            await api.updateBoard(board.id, newName, board.gradient, board.is_starred);
            board.name = newName;
        }
        titleEl.textContent = board.name;
        titleEl.style.display = '';
        input.remove();
    };
    
    input.addEventListener('blur', save);
    input.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') save();
        if (e.key === 'Escape') {
            titleEl.style.display = '';
            input.remove();
        }
    });
    
    titleEl.style.display = 'none';
    titleEl.parentNode.insertBefore(input, titleEl);
    input.focus();
    input.select();
}

/**
 * Create a column DOM element
 */
function createColumnElement(colData) {
    const column = createElement('div', {
        className: 'column',
        dataset: { columnId: colData.id }
    });
    
    // Header
    const header = createElement('div', { className: 'column__header' });
    
    const titleWrapper = createElement('div', { className: 'column__title-wrapper' });
    const title = createElement('span', { className: 'column__title' }, colData.name);
    title.addEventListener('click', () => editColumnTitle(colData, title));
    titleWrapper.appendChild(title);
    
    const count = createElement('span', { className: 'column__count' }, String(colData.cards.length));
    titleWrapper.appendChild(count);
    
    header.appendChild(titleWrapper);
    
    const menuBtn = createElement('button', {
        className: 'column__menu-btn',
        innerHTML: Icons.moreHorizontal
    });
    menuBtn.addEventListener('click', (e) => showColumnMenu(e, colData));
    header.appendChild(menuBtn);
    
    column.appendChild(header);
    
    // Cards list
    const cardsList = createElement('div', {
        className: 'column__cards',
        dataset: { columnId: colData.id }
    });
    
    for (const card of colData.cards) {
        cardsList.appendChild(createCardElement(card));
    }
    
    column.appendChild(cardsList);
    
    // Add card button
    const addCardBtn = createElement('button', {
        className: 'column__add-card',
        innerHTML: `${Icons.plus} <span>Добавить карточку</span>`
    });
    addCardBtn.addEventListener('click', () => showAddCardForm(column, colData));
    column.appendChild(addCardBtn);
    
    return column;
}

/**
 * Edit column title inline
 */
function editColumnTitle(colData, titleEl) {
    const input = createElement('input', {
        className: 'column__title-input',
        value: colData.name
    });
    
    const save = async () => {
        const newName = input.value.trim();
        if (newName && newName !== colData.name) {
            await api.updateColumn(colData.id, newName);
            colData.name = newName;
        }
        titleEl.textContent = colData.name;
        titleEl.style.display = '';
        input.remove();
    };
    
    input.addEventListener('blur', save);
    input.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') save();
        if (e.key === 'Escape') {
            titleEl.style.display = '';
            input.remove();
        }
    });
    
    titleEl.style.display = 'none';
    titleEl.parentNode.insertBefore(input, titleEl);
    input.focus();
    input.select();
}

/**
 * Show column context menu
 */
function showColumnMenu(event, colData) {
    // Remove any existing menu
    const existing = $('.context-menu');
    if (existing) existing.remove();
    
    const menu = createElement('div', { className: 'context-menu' });
    
    const archiveItem = createElement('div', {
        className: 'context-menu__item context-menu__item--danger',
        innerHTML: `${Icons.archive} <span>Архивировать колонку</span>`
    });
    archiveItem.addEventListener('click', async () => {
        await api.archiveColumn(colData.id);
        menu.remove();
        renderBoard(currentBoardId);
        showToast('Колонка архивирована');
    });
    
    menu.appendChild(archiveItem);
    
    // Position menu
    const rect = event.target.getBoundingClientRect();
    menu.style.top = `${rect.bottom + 4}px`;
    menu.style.left = `${rect.left}px`;
    
    document.body.appendChild(menu);
    
    // Close on outside click
    const closeMenu = (e) => {
        if (!menu.contains(e.target)) {
            menu.remove();
            document.removeEventListener('click', closeMenu);
        }
    };
    setTimeout(() => document.addEventListener('click', closeMenu), 0);
}

/**
 * Create a card DOM element
 */
function createCardElement(cardData) {
    const card = createElement('div', {
        className: 'card',
        dataset: { cardId: cardData.id }
    });
    
    // Labels (if any)
    if (cardData.labels && cardData.labels.length > 0) {
        const labelsEl = createElement('div', { className: 'card__labels' });
        for (const label of cardData.labels) {
            labelsEl.appendChild(createElement('div', {
                className: 'card__label',
                style: { background: label.color }
            }));
        }
        card.appendChild(labelsEl);
    }
    
    // Title
    card.appendChild(createElement('div', {
        className: 'card__title'
    }, cardData.title));
    
    // Metadata
    const hasMeta = cardData.description || cardData.due_date;
    if (hasMeta) {
        const meta = createElement('div', { className: 'card__meta' });
        
        if (cardData.due_date) {
            const overdue = isOverdue(cardData.due_date);
            const dueEl = createElement('div', {
                className: `card__meta-item card__meta-item--due ${overdue ? 'overdue' : ''}`,
                innerHTML: `${Icons.clock} <span>${formatDate(cardData.due_date)}</span>`
            });
            meta.appendChild(dueEl);
        }
        
        if (cardData.description) {
            meta.appendChild(createElement('div', {
                className: 'card__meta-item',
                innerHTML: Icons.description
            }));
        }
        
        card.appendChild(meta);
    }
    
    // Edit button
    const editBtn = createElement('button', {
        className: 'card__edit-btn',
        innerHTML: Icons.edit
    });
    editBtn.addEventListener('click', (e) => {
        e.stopPropagation();
        showCardEditModal(cardData);
    });
    card.appendChild(editBtn);
    
    // Click to open card detail
    card.addEventListener('click', () => showCardEditModal(cardData));
    
    return card;
}

/**
 * Show card edit modal
 */
function showCardEditModal(cardData) {
    const existing = $('.modal-overlay');
    if (existing) existing.remove();
    
    const overlay = createElement('div', { className: 'modal-overlay', id: 'card-modal-overlay' });
    const modal = createElement('div', { className: 'modal', style: { width: '500px' } });
    
    // Header
    const header = createElement('div', { className: 'modal__header' });
    header.appendChild(createElement('h2', { className: 'modal__title' }, cardData.title));
    const closeBtn = createElement('button', { className: 'modal__close', innerHTML: Icons.x });
    closeBtn.addEventListener('click', () => overlay.remove());
    header.appendChild(closeBtn);
    modal.appendChild(header);
    
    // Body
    const body = createElement('div', { className: 'modal__body' });
    
    // Title
    const titleGroup = createElement('div', { className: 'form-group' });
    titleGroup.appendChild(createElement('label', { className: 'form-label' }, 'Название'));
    const titleInput = createElement('input', {
        className: 'form-input',
        value: cardData.title,
        id: 'card-edit-title'
    });
    titleGroup.appendChild(titleInput);
    body.appendChild(titleGroup);
    
    // Description
    const descGroup = createElement('div', { className: 'form-group' });
    descGroup.appendChild(createElement('label', { className: 'form-label' }, 'Описание'));
    const descInput = createElement('textarea', {
        className: 'form-textarea',
        id: 'card-edit-description'
    });
    descInput.value = cardData.description || '';
    descInput.placeholder = 'Добавьте описание...';
    descGroup.appendChild(descInput);
    body.appendChild(descGroup);
    
    // Due date
    const dueGroup = createElement('div', { className: 'form-group' });
    dueGroup.appendChild(createElement('label', { className: 'form-label' }, 'Дедлайн'));
    const dueInput = createElement('input', {
        className: 'form-input',
        type: 'date',
        id: 'card-edit-due',
        value: cardData.due_date ? cardData.due_date.split('T')[0] : ''
    });
    dueGroup.appendChild(dueInput);
    body.appendChild(dueGroup);
    
    modal.appendChild(body);
    
    // Footer
    const footer = createElement('div', { className: 'modal__footer' });
    
    const archiveBtn = createElement('button', {
        className: 'btn btn--danger',
        innerHTML: `${Icons.archive} Архивировать`
    });
    archiveBtn.addEventListener('click', async () => {
        await api.archiveCard(cardData.id);
        overlay.remove();
        renderBoard(currentBoardId);
        showToast('Карточка архивирована');
    });
    footer.appendChild(archiveBtn);
    
    const saveBtn = createElement('button', {
        className: 'btn btn--primary'
    }, 'Сохранить');
    saveBtn.addEventListener('click', async () => {
        const title = titleInput.value.trim();
        if (!title) return;
        const dueDate = dueInput.value || null;
        await api.updateCard(cardData.id, title, descInput.value, dueDate);
        overlay.remove();
        renderBoard(currentBoardId);
        showToast('Карточка обновлена');
    });
    footer.appendChild(saveBtn);
    
    modal.appendChild(footer);
    overlay.appendChild(modal);
    
    // Close on overlay click
    overlay.addEventListener('click', (e) => {
        if (e.target === overlay) overlay.remove();
    });
    
    document.body.appendChild(overlay);
    titleInput.focus();
}

/**
 * Show add card form in column
 */
function showAddCardForm(columnEl, colData) {
    // Hide the add card button
    const addBtn = $('.column__add-card', columnEl);
    if (addBtn) addBtn.classList.add('hidden');
    
    // Check if form already exists
    const existingForm = $('.add-card-form', columnEl);
    if (existingForm) {
        existingForm.querySelector('textarea').focus();
        return;
    }
    
    const form = createElement('div', { className: 'add-card-form' });
    
    const textarea = createElement('textarea', {
        className: 'add-card-form__textarea',
        placeholder: 'Введите название карточки...'
    });
    textarea.addEventListener('input', () => autoResize(textarea));
    textarea.addEventListener('keydown', async (e) => {
        if (e.key === 'Enter' && !e.shiftKey) {
            e.preventDefault();
            await submitCard();
        }
        if (e.key === 'Escape') closeForm();
    });
    form.appendChild(textarea);
    
    const actions = createElement('div', { className: 'add-card-form__actions' });
    
    const submitBtn = createElement('button', { className: 'add-card-form__submit' }, 'Добавить');
    submitBtn.addEventListener('click', submitCard);
    actions.appendChild(submitBtn);
    
    const cancelBtn = createElement('button', {
        className: 'add-card-form__cancel',
        innerHTML: Icons.x
    });
    cancelBtn.addEventListener('click', closeForm);
    actions.appendChild(cancelBtn);
    
    form.appendChild(actions);
    columnEl.appendChild(form);
    textarea.focus();
    
    async function submitCard() {
        const title = textarea.value.trim();
        if (!title) return;
        
        try {
            await api.createCard(colData.id, title);
            textarea.value = '';
            textarea.style.height = '';
            renderBoard(currentBoardId);
            showToast('Карточка создана');
        } catch (e) {
            showToast('Ошибка создания карточки', 'error');
        }
    }
    
    function closeForm() {
        form.remove();
        if (addBtn) addBtn.classList.remove('hidden');
    }
}

/**
 * Create add column element
 */
function createAddColumnElement() {
    const wrapper = createElement('div', { className: 'add-column', id: 'add-column-wrapper' });
    
    const btn = createElement('button', {
        className: 'add-column__btn',
        innerHTML: `${Icons.plus} <span>Добавьте ещё одну колонку</span>`
    });
    btn.addEventListener('click', () => showAddColumnForm(wrapper));
    
    wrapper.appendChild(btn);
    return wrapper;
}

/**
 * Show add column inline form
 */
function showAddColumnForm(wrapper) {
    wrapper.innerHTML = '';
    
    const form = createElement('div', { className: 'add-column__form' });
    
    const input = createElement('input', {
        className: 'add-column__input',
        placeholder: 'Введите название колонки...'
    });
    input.addEventListener('keydown', async (e) => {
        if (e.key === 'Enter') await submitColumn();
        if (e.key === 'Escape') resetAddColumn(wrapper);
    });
    form.appendChild(input);
    
    const actions = createElement('div', { className: 'add-column__actions' });
    
    const submitBtn = createElement('button', { className: 'add-column__submit' }, 'Добавить колонку');
    submitBtn.addEventListener('click', submitColumn);
    actions.appendChild(submitBtn);
    
    const cancelBtn = createElement('button', {
        className: 'add-column__cancel',
        innerHTML: Icons.x
    });
    cancelBtn.addEventListener('click', () => resetAddColumn(wrapper));
    actions.appendChild(cancelBtn);
    
    form.appendChild(actions);
    wrapper.appendChild(form);
    input.focus();
    
    async function submitColumn() {
        const name = input.value.trim();
        if (!name) return;
        
        try {
            await api.createColumn(currentBoardId, name);
            renderBoard(currentBoardId);
            showToast('Колонка создана');
        } catch (e) {
            showToast('Ошибка создания колонки', 'error');
        }
    }
}

function resetAddColumn(wrapper) {
    wrapper.innerHTML = '';
    const btn = createElement('button', {
        className: 'add-column__btn',
        innerHTML: `${Icons.plus} <span>Добавьте ещё одну колонку</span>`
    });
    btn.addEventListener('click', () => showAddColumnForm(wrapper));
    wrapper.appendChild(btn);
}

/**
 * Initialize Sortable.js for drag-and-drop
 */
function initSortable() {
    // Destroy existing instances
    if (columnSortable) columnSortable.destroy();
    cardSortables.forEach(s => s.destroy());
    cardSortables = [];
    
    const boardEl = $('#board-columns');
    if (!boardEl) return;
    
    // Column drag-and-drop
    columnSortable = new Sortable(boardEl, {
        animation: 200,
        handle: '.column__header',
        draggable: '.column',
        ghostClass: 'sortable-ghost',
        chosenClass: 'sortable-chosen',
        filter: '.add-column',
        onEnd: async (evt) => {
            const columnEls = $$('.column', boardEl);
            const columnIds = columnEls.map(el => parseInt(el.dataset.columnId));
            try {
                await api.reorderColumns(currentBoardId, columnIds);
            } catch (e) {
                console.error('Failed to reorder columns:', e);
            }
        }
    });
    
    // Card drag-and-drop for each column
    const cardLists = $$('.column__cards', boardEl);
    for (const list of cardLists) {
        const sortable = new Sortable(list, {
            group: 'cards',
            animation: 200,
            draggable: '.card',
            ghostClass: 'sortable-ghost',
            chosenClass: 'sortable-chosen',
            dragClass: 'sortable-drag',
            onEnd: async (evt) => {
                const cardId = parseInt(evt.item.dataset.cardId);
                const newColumnId = parseInt(evt.to.dataset.columnId);
                const newPosition = evt.newIndex;
                
                try {
                    await api.moveCard(cardId, newColumnId, newPosition);
                    
                    // Update card counts
                    updateCardCounts();
                } catch (e) {
                    console.error('Failed to move card:', e);
                    renderBoard(currentBoardId);
                }
            }
        });
        cardSortables.push(sortable);
    }
}

/**
 * Update card count badges in column headers
 */
function updateCardCounts() {
    const columns = $$('.column');
    for (const col of columns) {
        const cards = $$('.card', col);
        const count = $('.column__count', col);
        if (count) count.textContent = String(cards.length);
    }
}

/**
 * Get current board ID
 */
export function getCurrentBoardId() {
    return currentBoardId;
}
