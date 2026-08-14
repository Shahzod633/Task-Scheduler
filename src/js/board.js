// ============================================
// TaskFlow — Board View (Kanban Board - Screen В)
// ============================================

import * as api from './api.js';
import Icons from './icons.js';
import { openPopover } from './popover.js';
import { createElement, $, $$, showToast, autoResize, escapeHtml, formatDate, isOverdue, staggerIn } from './utils.js';

let currentBoardId = null;
let columnsData = [];
let columnSortable = null;
let cardSortables = [];
// Set while a card drag is in flight, so the click that ends a drag gesture
// doesn't also open the card modal.
let isDraggingCard = false;

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
        
        // Render columns — каскадом, чтобы доска «собиралась» слева направо
        columnsData.forEach((col, i) => {
            boardEl.appendChild(staggerIn(createColumnElement(col), i));
        });

        // Add column button
        boardEl.appendChild(staggerIn(createAddColumnElement(), columnsData.length));
        
        content.appendChild(boardEl);
        
        // Initialize Sortable.js
        initSortable();
        
        setTimeout(() => content.classList.remove('view-enter'), 420);
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

    // Меню уезжает в общий слой: колонка скроллится и лежит внутри
    // overflow: hidden, поэтому внутри неё меню было бы обрезано.
    openPopover(menu, event.currentTarget, { placement: 'bottom', align: 'start', gap: 4 });
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

    // Mistake-tracking indicator
    if (cardData.is_mistake) {
        card.appendChild(createElement('div', {
            className: `card__mistake-tag ${cardData.mistake_resolved_at ? 'card__mistake-tag--resolved' : ''}`,
            innerHTML: `${cardData.mistake_resolved_at ? Icons.checkCircle : Icons.alertTriangle} <span>${cardData.mistake_resolved_at ? 'Ошибка исправлена' : 'Ошибка'}</span>`
        }));
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

    // Card context menu (mistake tracking)
    const menuBtn = createElement('button', {
        className: 'card__menu-btn',
        innerHTML: Icons.moreHorizontal
    });
    menuBtn.addEventListener('click', (e) => {
        e.stopPropagation();
        showCardMenu(e, cardData);
    });
    card.appendChild(menuBtn);

    // Click to open card detail
    card.addEventListener('click', () => {
        if (isDraggingCard) return;
        showCardEditModal(cardData);
    });

    return card;
}

/**
 * Show a card's context menu (currently: mistake-tracking toggle)
 */
function showCardMenu(event, cardData) {
    const existing = $('.context-menu');
    if (existing) existing.remove();

    const menu = createElement('div', { className: 'context-menu' });

    if (!cardData.is_mistake) {
        const markItem = createElement('div', {
            className: 'context-menu__item',
            innerHTML: `${Icons.alertTriangle} <span>Отметить как ошибку</span>`
        });
        markItem.addEventListener('click', async () => {
            try {
                await api.markCardMistake(cardData.id);
                menu.remove();
                renderBoard(currentBoardId);
                showToast('Карточка отмечена как ошибка');
            } catch (e) {
                showToast('Ошибка операции', 'error');
            }
        });
        menu.appendChild(markItem);
    } else if (!cardData.mistake_resolved_at) {
        const resolveItem = createElement('div', {
            className: 'context-menu__item',
            innerHTML: `${Icons.checkCircle} <span>Ошибка исправлена</span>`
        });
        resolveItem.addEventListener('click', async () => {
            try {
                await api.resolveCardMistake(cardData.id);
                menu.remove();
                renderBoard(currentBoardId);
                showToast('Ошибка отмечена как исправленная');
            } catch (e) {
                showToast('Ошибка операции', 'error');
            }
        });
        menu.appendChild(resolveItem);
    } else {
        menu.appendChild(createElement('div', {
            className: 'context-menu__item context-menu__item--info',
            innerHTML: `${Icons.checkCircle} <span>Ошибка уже исправлена</span>`
        }));
    }

    openPopover(menu, event.currentTarget, { placement: 'bottom', align: 'start', gap: 4 });
}

/**
 * Show card edit modal
 */
export function showCardEditModal(cardData, options = {}) {
    const onChange = options.onChange || (() => renderBoard(currentBoardId));
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
        onChange();
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
        onChange();
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
        easing: 'cubic-bezier(0.2, 0, 0, 1)',
        swapThreshold: 0.65,
        fallbackTolerance: 3,
        handle: '.column__header',
        draggable: '.column',
        // Автопрокрутка доски, когда колонку тянут к её краю
        scroll: true,
        scrollSensitivity: 90,
        scrollSpeed: 14,
        ghostClass: 'sortable-ghost',
        chosenClass: 'sortable-chosen',
        filter: '.add-column',
        // See the note on the card Sortable below — the WebView2 host makes
        // native HTML5 drag unreliable, so drive drags from pointer events.
        forceFallback: true,
        fallbackOnBody: true,
        fallbackClass: 'sortable-fallback',
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
            // Sortable animates the neighbouring cards itself, writing an
            // inline `transition: transform 200ms <easing>` on each one, so
            // this easing is what actually smooths them apart (not any CSS
            // transition on .card — see the note in board.css).
            easing: 'cubic-bezier(0.2, 0, 0, 1)',
            // A card must cover 65% of a neighbour before they swap, which
            // stops the list flickering on tiny mouse movements.
            swapThreshold: 0.65,
            // Ignore sub-pixel jitter so a click isn't read as a drag.
            fallbackTolerance: 3,
            draggable: '.card',
            // Карточку можно донести до края доски или списка — вид
            // подкручивается сам, без отпускания кнопки мыши.
            scroll: true,
            scrollSensitivity: 90,
            scrollSpeed: 14,
            bubbleScroll: true,
            ghostClass: 'sortable-ghost',
            chosenClass: 'sortable-chosen',
            dragClass: 'sortable-drag',
            // Tauri's webview intercepts native HTML5 drag-and-drop on Windows
            // (hence dragDropEnabled: false in tauri.conf.json). forceFallback
            // makes Sortable drive the drag from pointer events instead, so it
            // no longer depends on the host webview's HTML5 DnD at all.
            forceFallback: true,
            // The drag clone must live on <body>: .column__cards scrolls and the
            // content area is overflow:hidden, which would otherwise clip it.
            fallbackOnBody: true,
            fallbackClass: 'sortable-fallback',
            onStart: (evt) => {
                isDraggingCard = true;
                // Гасит hover-подсветку карточек на время перетаскивания,
                // чтобы она не спорила с инлайновыми стилями Sortable.
                document.body.classList.add('is-dragging-card');
                evt.from.classList.add('sortable-drag-over');
            },
            // Highlight the column the card is currently hovering over — with
            // empty columns this is the only cue that a drop will land there.
            onMove: (evt) => {
                clearDropHighlight();
                if (evt.to) evt.to.classList.add('sortable-drag-over');
                return true;
            },
            onEnd: async (evt) => {
                clearDropHighlight();
                document.body.classList.remove('is-dragging-card');
                // Let the trailing click land before re-enabling card clicks.
                setTimeout(() => { isDraggingCard = false; }, 0);

                const cardId = parseInt(evt.item.dataset.cardId);
                const newColumnId = parseInt(evt.to.dataset.columnId);
                const newPosition = evt.newIndex;

                // Dropped back where it started — nothing to persist.
                if (evt.to === evt.from && evt.newIndex === evt.oldIndex) return;

                try {
                    await api.updateCardPosition(cardId, newColumnId, newPosition);

                    // Update card counts
                    updateCardCounts();
                } catch (e) {
                    console.error('Failed to move card:', e);
                    showToast('Не удалось переместить карточку', 'error');
                    renderBoard(currentBoardId);
                }
            }
        });
        cardSortables.push(sortable);
    }
}

/**
 * Remove the drop-target highlight from every column
 */
function clearDropHighlight() {
    $$('.column__cards').forEach(el => el.classList.remove('sortable-drag-over'));
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
