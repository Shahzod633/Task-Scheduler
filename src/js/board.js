// ============================================
// TaskFlow — Board View (Kanban Board - Screen В)
// ============================================

import * as api from './api.js';
import Icons from './icons.js';
import { openPopover, closePopovers } from './popover.js';
import { confirmDialog } from './dialog.js';
import { showBoardArchive } from './archive.js';
import { renderChecklist } from './checklist.js';
import { renderComments } from './comments.js';
import { loadMembers, findMember, createAvatar, createMemberChip, openMemberPicker } from './members.js';
import {
    createFilterState, createFilterToolbar, matchesFilter, isFilterActive,
    groupKeyFor, priorityLabel, priorityModifier, PRIORITIES,
} from './filters.js';
import { createElement, $, $$, showToast, autoResize, escapeHtml, formatDueDate, isOverdue, staggerIn, pluralize, debounce, formatDate, parseDueDate } from './utils.js';
import { renderMarkdown } from './markdown.js';
import { renderDependencies, confirmFinalColumnMove } from './dependencies.js';
import { attachTimerButton, renderTimeLog, refreshTimer } from './timer.js';
import { renderFieldSettings, renderCardFields } from './fields.js';

let currentBoardId = null;
// Пространство открытой доски. Нужно выбору карточки для зависимости: тот
// ищет по всему пространству, а `get_cards` про него ничего не знает.
let currentWorkspaceId = null;
let columnsData = [];
let columnSortable = null;
let cardSortables = [];
// Set while a card drag is in flight, so the click that ends a drag gesture
// doesn't also open the card modal.
let isDraggingCard = false;
// Filter/group state for the board currently on screen. Reset when a different
// board is opened: another board has other columns, and a stale status filter
// would hide everything for no visible reason.
let boardFilter = createFilterState();
// Какой из двух видов доски открыт: канбан или заметки. Живёт здесь, а не в
// маршруте, потому что это не отдельный экран — шапка, название и адрес те же,
// меняется только то, что под шапкой. Сбрасывается вместе с фильтром при
// переходе на другую доску: заметки чужой доски на месте канбана выглядели бы
// как потерянные колонки.
let boardView = 'board';

/**
 * Initialize board view for a given board
 */
export async function renderBoard(boardId) {
    if (boardId !== currentBoardId) {
        boardFilter = createFilterState();
        boardView = 'board';
    }
    currentBoardId = boardId;
    const content = $('#content');

    try {
        const board = await api.getBoard(boardId);
        currentWorkspaceId = board.workspace_id;

        // Заметкам не нужны ни колонки, ни карточки, ни справочник участников.
        // Выходим до их загрузки, а не прячем готовый канбан: переключение вида
        // не повод обойти базу по разу на колонку.
        if (boardView === 'notes') {
            const notes = await api.getBoardNotes(boardId);
            content.innerHTML = '';
            content.classList.add('view-enter');
            content.appendChild(createBoardHeader(board));
            content.appendChild(createNotesPane(boardId, notes));
            setTimeout(() => content.classList.remove('view-enter'), 420);
            return;
        }

        const columns = await api.getColumns(boardId);

        // The member directory backs the avatars on cards and the quick filter
        // in the toolbar; one call per board render, not one per card.
        await loadMembers();

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

        // Filter / group toolbar — the same component the "Список" screen uses,
        // scoped to this one board.
        content.appendChild(createBoardToolbar());

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
        applyBoardFilter();

        setTimeout(() => content.classList.remove('view-enter'), 420);
    } catch (error) {
        console.error('Error loading board:', error);
        showToast('Ошибка загрузки доски', 'error');
    }
}

/**
 * The board's copy of the shared toolbar.
 *
 * Scope differs from the "Список" screen on purpose: that screen spans every
 * board in the workspace, this one only ever sees the board it is on, so the
 * "Доска" facet is omitted.
 */
function createBoardToolbar() {
    const wrap = createElement('div', { className: 'board-toolbar' });

    wrap.appendChild(createFilterToolbar({
        state: boardFilter,
        onChange: applyBoardFilter,
        boards: null,
        statuses: columnsData.map(c => c.name),
        showGroup: true,
        searchPlaceholder: 'Поиск по карточкам доски',
    }));

    wrap.appendChild(createElement('div', { className: 'board-toolbar__count', id: 'board-filter-count' }));
    return wrap;
}

/** The shape `filters.js` matches against. */
function toFilterItem(cardData, colData) {
    const assignee = findMember(cardData.assignee_id);
    return {
        title: cardData.title,
        boardId: currentBoardId,
        boardName: '',
        assigneeId: cardData.assignee_id ?? 0,
        assignee,
        status: colData.name,
        priority: cardData.priority,
    };
}

/**
 * Applies the current filter and grouping to what is already on screen.
 *
 * Purely visual: cards are hidden and re-ordered in the DOM, never moved in the
 * database. `position` and `column_id` are untouched, so switching the filter
 * off restores exactly the board that was there before.
 */
function applyBoardFilter() {
    const grouping = Boolean(boardFilter.groupBy);
    let shown = 0;
    let total = 0;

    for (const colData of columnsData) {
        const columnEl = $(`.column[data-column-id="${colData.id}"]`);
        if (!columnEl) continue;
        const list = $('.column__cards', columnEl);
        if (!list) continue;

        // Group headings from a previous pass are rebuilt from scratch.
        $$('.column__group-heading', list).forEach(el => el.remove());

        const visible = [];
        for (const cardData of colData.cards) {
            total++;
            const cardEl = $(`.card[data-card-id="${cardData.id}"]`, list);
            if (!cardEl) continue;
            const match = matchesFilter(toFilterItem(cardData, colData), boardFilter);
            cardEl.classList.toggle('card--filtered-out', !match);
            if (match) { shown++; visible.push({ cardData, cardEl }); }
        }

        if (grouping) applyColumnGrouping(list, colData, visible);
        else restoreCardOrder(list, colData);

        const countEl = $('.column__count', columnEl);
        if (countEl) {
            countEl.textContent = isFilterActive(boardFilter)
                ? `${visible.length}/${colData.cards.length}`
                : String(colData.cards.length);
        }
    }

    // Drag-and-drop is suspended whenever the visible order stops matching the
    // stored one — Sortable derives the drop index from the card's position
    // among its siblings and counts the hidden ones too, so a drop during
    // filtering or grouping would silently write the wrong position.
    const filtering = isFilterActive(boardFilter);
    setCardDragEnabled(!grouping && !filtering);

    const countEl = $('#board-filter-count');
    if (countEl) {
        countEl.innerHTML = '';
        if (filtering) {
            countEl.appendChild(createElement('span', { className: 'filter-count' },
                `${shown} из ${total} ${pluralize(total, ['карточка', 'карточки', 'карточек'])}`));
        }
        if (grouping || filtering) {
            countEl.appendChild(createElement('span', { className: 'board-toolbar__note' },
                grouping
                    ? 'Пока включена группировка, карточки нельзя перетаскивать'
                    : 'Пока включён фильтр, карточки нельзя перетаскивать'));
        }
    }
}

/** Re-orders the visible cards into groups with a small heading each. */
function applyColumnGrouping(list, colData, visible) {
    const groups = new Map();
    for (const entry of visible) {
        const key = groupKeyFor(toFilterItem(entry.cardData, colData), boardFilter.groupBy);
        if (!groups.has(key)) groups.set(key, []);
        groups.get(key).push(entry);
    }

    for (const [key, entries] of groups) {
        list.appendChild(createElement('div', { className: 'column__group-heading' },
            `${key} · ${entries.length}`));
        for (const entry of entries) list.appendChild(entry.cardEl);
    }
}

/** Puts the cards back in their stored order after grouping is switched off. */
function restoreCardOrder(list, colData) {
    for (const cardData of colData.cards) {
        const cardEl = $(`.card[data-card-id="${cardData.id}"]`, list);
        if (cardEl) list.appendChild(cardEl);
    }
}

function setCardDragEnabled(enabled) {
    for (const sortable of cardSortables) {
        if (sortable && typeof sortable.option === 'function') {
            sortable.option('disabled', !enabled);
        }
    }
}

/**
 * Create board header element
 */
/**
 * A header button for a feature that does not exist yet.
 *
 * These used to be silent no-ops that looked identical to working controls.
 * They are kept (the layout was designed around them) but are now visibly
 * dimmed and say so when clicked, instead of pretending to do something.
 */
function createSoonButton(label, icon) {
    const btn = createElement('button', {
        className: 'board-header__btn board-header__btn--soon',
        innerHTML: label ? `${icon} <span>${label}</span>` : icon,
        'data-tooltip': `${label || 'Эта функция'} — появится в следующих версиях`
    });
    btn.addEventListener('click', () => {
        showToast(`«${label || 'Эта функция'}» появится в следующих версиях`, 'info');
    });
    return btn;
}

/**
 * Переключатель «Доска / Заметки» у названия доски.
 *
 * На этом месте раньше стояла заглушка `createSoonButton('', Icons.grid)` —
 * серая кнопка с иконкой сетки, которая на щелчок отвечала «появится в
 * следующих версиях». Теперь она стала рабочей.
 */
function createViewSwitch() {
    const wrap = createElement('div', { className: 'board-view-switch' });

    const views = [
        { id: 'board', label: 'Доска', icon: Icons.grid },
        { id: 'notes', label: 'Заметки', icon: Icons.description },
    ];

    for (const view of views) {
        const btn = createElement('button', {
            className: 'board-view-switch__option'
                + (boardView === view.id ? ' board-view-switch__option--active' : ''),
            innerHTML: `${view.icon} <span>${view.label}</span>`,
        });
        btn.addEventListener('click', () => {
            if (boardView === view.id) return;
            boardView = view.id;
            renderBoard(currentBoardId);
        });
        wrap.appendChild(btn);
    }

    return wrap;
}

function createBoardHeader(board) {
    const header = createElement('div', { className: 'board-header' });

    const left = createElement('div', { className: 'board-header__left' });
    const title = createElement('span', {
        className: 'board-header__title',
        id: 'board-title'
    }, board.name);
    title.addEventListener('click', () => editBoardTitle(board));
    left.appendChild(title);

    left.appendChild(createViewSwitch());

    const right = createElement('div', { className: 'board-header__right' });

    right.appendChild(createSoonButton('Power-Ups', Icons.puzzle));
    right.appendChild(createSoonButton('Автоматизация', Icons.zap));
    // The "Фильтры" placeholder that used to sit here is gone — filtering is
    // real now and lives in the toolbar below the header.

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

    // Archive — the way back for archived cards and columns.
    const archiveBtn = createElement('button', {
        className: 'board-header__btn',
        innerHTML: Icons.archive,
        'data-tooltip': 'Архив доски'
    });
    archiveBtn.addEventListener('click', () => {
        showBoardArchive(board.id, board.name, () => renderBoard(currentBoardId));
    });
    right.appendChild(archiveBtn);

    right.appendChild(createSoonButton('Приватная', Icons.lock));

    // Export — writes a real file, unlike the "Поделиться" stub it replaces.
    const exportBtn = createElement('button', {
        className: 'board-header__btn board-header__btn--share',
        innerHTML: `${Icons.download} <span>Экспорт</span>`,
        'data-tooltip': 'Сохранить доску в JSON-файл'
    });
    exportBtn.addEventListener('click', () => exportCurrentBoard(board));
    right.appendChild(exportBtn);

    // Настройки доски. На этом месте стояла заглушка «⋯» — `createSoonButton`,
    // которая на щелчок отвечала «появится в следующих версиях»: настроек у
    // доски не было вовсе. Значок теперь шестерёнка — кнопка открывает ровно
    // настройки, а не меню «ещё».
    const settingsBtn = createElement('button', {
        className: 'board-header__btn',
        innerHTML: Icons.settings,
        'data-tooltip': 'Настройки доски',
    });
    settingsBtn.addEventListener('click', () => showBoardSettings(board));
    right.appendChild(settingsBtn);

    header.appendChild(left);
    header.appendChild(right);
    return header;
}

/**
 * Окно «Настройки доски». Пока в нём один раздел — пользовательские поля.
 *
 * Поля пишутся сразу, по кнопкам внутри раздела, поэтому у окна нет своей
 * «Сохранить». Доску после закрытия перерисовывать не нужно: на лицевой
 * стороне карточек поля не показываются.
 */
function showBoardSettings(board) {
    closePopovers();
    const existing = $('.modal-overlay');
    if (existing) existing.remove();

    const overlay = createElement('div', { className: 'modal-overlay' });
    const modal = createElement('div', { className: 'modal board-settings' });

    const header = createElement('div', { className: 'modal__header' });
    const heading = createElement('div', { className: 'board-settings__heading' });
    heading.appendChild(createElement('h2', { className: 'modal__title' }, 'Настройки доски'));
    heading.appendChild(createElement('span', { className: 'board-settings__board' }, board.name));
    header.appendChild(heading);
    const closeBtn = createElement('button', { className: 'modal__close', innerHTML: Icons.x });
    closeBtn.addEventListener('click', () => overlay.remove());
    header.appendChild(closeBtn);
    modal.appendChild(header);

    const body = createElement('div', { className: 'modal__body' });
    renderFieldSettings(body, board.id);
    modal.appendChild(body);

    overlay.appendChild(modal);
    overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
    document.body.appendChild(overlay);
}

// ─── Заметки доски ───
//
// Одна markdown-страница на доску: место для контекста, который никуда не
// помещается на карточке, — договорённости, ссылки, план. Сознательно не вики:
// ни списка страниц, ни истории версий.

/** Через сколько после последнего нажатия клавиши уходит сохранение. */
const NOTES_SAVE_DELAY = 1000;

/**
 * Панель заметок: текстовое поле, предпросмотр и строка состояния.
 *
 * @param {number} boardId
 * @param {{content: string, updated_at: string|null}} notes уже прочитанные заметки
 */
function createNotesPane(boardId, notes) {
    const pane = createElement('div', { className: 'board-notes' });

    // Текущий текст держим здесь, а не читаем из textarea по месту: предпросмотр
    // должен показывать то, что человек набрал секунду назад, даже если
    // сохранение ещё не ушло.
    let current = notes.content || '';
    let savedAt = notes.updated_at;
    // Открываем на просмотре, если есть что читать: на «Заметки» нажимают,
    // чтобы прочитать их, а не чтобы увидеть разметку. Пустую заметку читать
    // нечего — там сразу поле ввода.
    let mode = current.trim() ? 'view' : 'edit';

    const status = createElement('div', { className: 'board-notes__status' });
    const body = createElement('div', { className: 'board-notes__body' });

    const textarea = createElement('textarea', {
        className: 'board-notes__editor',
        placeholder: 'Заметки доски. Поддерживается markdown: # заголовок, **жирный**, - список, > цитата, ```код```',
        spellcheck: 'false',
    });
    textarea.value = current;

    const preview = createElement('div', { className: 'board-notes__preview markdown-body' });

    const setStatus = (text, modifier = '') => {
        status.textContent = text;
        status.className = 'board-notes__status' + (modifier ? ` board-notes__status--${modifier}` : '');
    };

    const showSaved = () => {
        // `formatDate` — тот же относительный формат, что у комментариев
        // («только что», «5 мин. назад»), и он же разбирает время из базы как UTC.
        setStatus(savedAt ? `Сохранено · ${formatDate(savedAt)}` : 'Пока пусто');
    };

    const save = async (value) => {
        setStatus('Сохранение…');
        try {
            const saved = await api.saveBoardNotes(boardId, value);
            savedAt = saved.updated_at;
            showSaved();
        } catch (error) {
            console.error('Не удалось сохранить заметки:', error);
            // Тост здесь был бы навязчив — печать продолжается, и он всплывал бы
            // на каждую неудачную попытку. Строка состояния держит ошибку перед
            // глазами ровно столько, сколько она длится.
            setStatus('Не удалось сохранить', 'error');
        }
    };

    const saveSoon = debounce(save, NOTES_SAVE_DELAY);

    textarea.addEventListener('input', () => {
        current = textarea.value;
        setStatus('Черновик…');
        saveSoon(current);
    });

    // ─── Переключатель «Редактировать» / «Просмотр» ───
    const modeSwitch = createElement('div', { className: 'board-notes__modes' });
    const modes = [
        { id: 'edit', label: 'Редактировать' },
        { id: 'view', label: 'Просмотр' },
    ];

    // `focus` просят только при щелчке по переключателю: на первой отрисовке
    // панель ещё не в документе, и `focus()` на открепленном элементе молча
    // ничего не делает.
    const applyMode = (focus = false) => {
        textarea.classList.toggle('hidden', mode !== 'edit');
        preview.classList.toggle('hidden', mode !== 'view');
        if (mode === 'view') {
            // Предпросмотр рисуется из `current`, а не из сохранённого текста:
            // между последним нажатием клавиши и записью в базу проходит секунда,
            // и всё это время они расходятся.
            preview.innerHTML = current.trim()
                ? renderMarkdown(current)
                : '<p class="board-notes__empty">Заметок пока нет.</p>';
        }
        for (const btn of modeSwitch.children) {
            btn.classList.toggle('board-notes__mode--active', btn.dataset.mode === mode);
        }
        if (focus && mode === 'edit') textarea.focus();
    };

    for (const item of modes) {
        const btn = createElement('button', {
            className: 'board-notes__mode',
            'data-mode': item.id,
        }, item.label);
        btn.addEventListener('click', () => {
            if (mode === item.id) return;
            mode = item.id;
            applyMode(true);
        });
        modeSwitch.appendChild(btn);
    }

    const toolbar = createElement('div', { className: 'board-notes__toolbar' });
    toolbar.appendChild(modeSwitch);
    toolbar.appendChild(status);

    body.appendChild(textarea);
    body.appendChild(preview);
    pane.appendChild(toolbar);
    pane.appendChild(body);

    showSaved();
    applyMode();
    return pane;
}

/**
 * Asks where to save, then writes the board's JSON there.
 *
 * The old implementation called a Rust stub that returned
 * `{"status": "exported"}` and reported success — no file was ever produced.
 */
async function exportCurrentBoard(board) {
    // A filename the OS will accept: strip anything Windows forbids in paths.
    const safeName = board.name.replace(/[\\/:*?"<>|]/g, '_').trim() || 'board';

    try {
        const path = await api.showSaveDialog(`${safeName}.json`);
        if (!path) return; // cancelled

        await api.exportBoardToFile(board.id, path);
        showToast('Доска сохранена в файл');
    } catch (e) {
        console.error('Board export failed:', e);
        showToast('Не удалось экспортировать доску', 'error');
    }
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
        className: `column ${colData.is_final ? 'column--final' : ''}`,
        dataset: { columnId: colData.id }
    });
    
    // Header
    const header = createElement('div', { className: 'column__header' });
    
    const titleWrapper = createElement('div', { className: 'column__title-wrapper' });
    // Замок стоит перед названием, а не после счётчика: правило колонки важно
    // прочитать до того, как тащишь в неё карточку, а не после.
    if (colData.is_final) {
        titleWrapper.appendChild(createElement('span', {
            className: 'column__final-badge',
            innerHTML: Icons.lock,
            'data-tooltip': 'Финальная колонка: карточку отсюда уже не вернуть'
        }));
    }
    
    const title = createElement('span', {
        className: `column__title ${colData.is_required ? 'column__title--locked' : ''}`
    }, colData.name);
    if (colData.is_required) {
        title.dataset.tooltip = 'Обязательная колонка — название менять нельзя';
    } else {
        title.addEventListener('click', () => editColumnTitle(colData, title));
    }
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
        // `final` читает `initSortable` и обработчик броска: DOM-узел знает
        // про свою колонку сам, без похода в `columnsData` на каждое событие.
        dataset: { columnId: colData.id, final: colData.is_final ? '1' : '' }
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

    // Обязательную колонку нельзя ни архивировать, ни переименовать, поэтому
    // меню у неё пустое — вместо кнопки, которая всё равно откажет, честная
    // строчка о том, почему её нет (см. правило про заглушки в §5 заметок).
    if (colData.is_required) {
        menu.appendChild(createElement('div', {
            className: 'context-menu__item context-menu__item--info',
            innerHTML: `${Icons.lock} <span>Обязательная колонка — её нельзя убрать или переименовать</span>`
        }));
        openPopover(menu, event.currentTarget, { placement: 'bottom', align: 'start', gap: 4 });
        return;
    }

    const archiveItem = createElement('div', {
        className: 'context-menu__item context-menu__item--danger',
        innerHTML: `${Icons.archive} <span>Архивировать колонку</span>`
    });
    archiveItem.addEventListener('click', async () => {
        menu.remove();
        const cardCount = colData.cards.length;
        const ok = await confirmDialog({
            title: 'Архивировать колонку?',
            message: cardCount > 0
                ? `Колонка «${colData.name}» и ${cardCount} ${pluralize(cardCount, ['карточка', 'карточки', 'карточек'])} внутри неё пропадут с доски. Вернуть их можно через «Архив доски».`
                : `Колонка «${colData.name}» пропадёт с доски. Вернуть её можно через «Архив доски».`,
            confirmText: 'Архивировать',
            danger: true,
        });
        if (!ok) return;

        try {
            await api.archiveColumn(colData.id);
            renderBoard(currentBoardId);
            showToast('Колонка архивирована');
        } catch (e) {
            showToast('Не удалось архивировать колонку', 'error');
        }
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
        // The priority modifier paints the thin stripe down the left edge.
        className: `card card--priority-${priorityModifier(cardData.priority)}`,
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
    const assignee = findMember(cardData.assignee_id);
    const checklistTotal = cardData.checklist_total || 0;
    const blockedOpen = cardData.blocked_open || 0;
    const hasMeta = cardData.description || cardData.due_date || assignee
        || checklistTotal > 0 || blockedOpen > 0;
    if (hasMeta) {
        const meta = createElement('div', { className: 'card__meta' });

        // Only shown when the card actually has a checklist — an empty "0 из 0"
        // on every card would be noise.
        if (checklistTotal > 0) {
            const done = cardData.checklist_done || 0;
            meta.appendChild(createElement('div', {
                className: `card__meta-item card__checklist ${done === checklistTotal ? 'card__checklist--complete' : ''}`,
                innerHTML: `${Icons.checkSquare} <span>${done} из ${checklistTotal}</span>`,
                'data-tooltip': 'Выполнено пунктов чек-листа'
            }));
        }

        // Значок звена — только когда блокировки ещё не сняты. Доведённая до
        // конца блокирующая задача уже ничего не держит, и напоминать о ней на
        // лицевой стороне значит держать в поле зрения решённый вопрос.
        if (blockedOpen > 0) {
            meta.appendChild(createElement('div', {
                className: 'card__meta-item card__blocked',
                innerHTML: `${Icons.link} <span>${blockedOpen}</span>`,
                'data-tooltip': `Ждёт ${blockedOpen} ${pluralize(blockedOpen, ['задачу', 'задачи', 'задач'])}`,
            }));
        }

        if (cardData.due_date) {
            const overdue = isOverdue(cardData.due_date);
            const dueEl = createElement('div', {
                className: `card__meta-item card__meta-item--due ${overdue ? 'overdue' : ''}`,
                innerHTML: `${Icons.clock} <span>${formatDueDate(cardData.due_date)}</span>`
            });
            meta.appendChild(dueEl);
        }

        if (cardData.description) {
            meta.appendChild(createElement('div', {
                className: 'card__meta-item',
                innerHTML: Icons.description
            }));
        }

        // Assignee sits at the far right of the meta row — the corner Trello
        // and ClickUp both use, so it reads without explanation.
        if (assignee) {
            const avatar = createAvatar(assignee, { size: 'sm' });
            avatar.classList.add('card__assignee');
            meta.appendChild(avatar);
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

    // Таймер — третьей кнопкой в том же ряду, что появляется по наведению.
    attachTimerButton(card, cardData.id);

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

/** Варианты «Повторять» в окне карточки — значения `cards.recurrence_rule`. */
const RECURRENCE_OPTIONS = [
    ['', 'Нет'],
    ['daily', 'Каждый день'],
    ['weekly', 'Каждую неделю'],
    ['monthly', 'Каждый месяц'],
];

/**
 * Тост о следующей карточке повторяющейся задачи — общий на все три двери в
 * финальную колонку (доска, «Список», Inbox). `spawn` — то, что вернул
 * `api.updateCardPosition`; null — ничего не создано, говорить не о чем.
 *
 * «Открыть» ведёт на доску новой карточки и открывает её окно: карточка
 * перечитывается целиком, как у перехода к связанной задаче, — окно правки
 * сохраняет все поля, и собранное из обрывков затёрло бы настоящие.
 *
 * @returns {boolean} был ли тост
 */
export function announceRecurringSpawn(spawn, workspaceId) {
    if (!spawn) return false;
    const due = spawn.due_date
        ? `, срок ${parseDueDate(spawn.due_date).toLocaleDateString('ru-RU')}`
        : ', без срока';
    showToast(`Создана следующая задача: «${spawn.title}»${due}`, 'success', {
        label: 'Открыть',
        onClick: async () => {
            window.dispatchEvent(new CustomEvent('navigate', { detail: { view: 'board', boardId: spawn.board_id } }));
            try {
                const data = await api.listAllCardsInWorkspace(workspaceId);
                const card = (data.cards || []).find(c => c.id === spawn.card_id);
                if (card) showCardEditModal(card, { workspaceId });
            } catch (e) {
                console.error('Не удалось открыть следующую задачу:', e);
            }
        },
    });
    return true;
}

/**
 * Show card edit modal
 */
export function showCardEditModal(cardData, options = {}) {
    const onChange = options.onChange || (() => renderBoard(currentBoardId));
    // Пространство нужно выбору карточки для зависимости. Экраны, открывающие
    // это окно не с доски («Список», палитра, планировщик, «Требуют внимания»),
    // знают его сами и передают явно; для доски оно взято при её отрисовке.
    const workspaceId = options.workspaceId || currentWorkspaceId;
    const existing = $('.modal-overlay');
    if (existing) existing.remove();

    const overlay = createElement('div', { className: 'modal-overlay', id: 'card-modal-overlay' });
    const modal = createElement('div', { className: 'modal', style: { width: '500px' } });

    // Checklist edits save as they happen rather than on "Сохранить", so
    // closing the window still has to refresh the board — otherwise the "2 из 5"
    // on the card face would keep showing the count from before.
    let checklistDirty = false;
    const dismiss = () => {
        overlay.remove();
        if (checklistDirty) onChange();
    };

    // Header
    const header = createElement('div', { className: 'modal__header' });
    header.appendChild(createElement('h2', { className: 'modal__title' }, cardData.title));
    const closeBtn = createElement('button', { className: 'modal__close', innerHTML: Icons.x });
    closeBtn.addEventListener('click', dismiss);
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
    
    // ─── Дедлайн ───
    //
    // Срок задаётся один раз. Пока его нет — поле обычное: задачу заводят
    // раньше, чем узнают дату сдачи. Как только дата появилась, поле
    // становится нередактируемым навсегда. То же правило продублировано в
    // `update_card` на бэкенде — окно можно и обойти.
    const hasDeadline = Boolean(cardData.due_date);
    const dueGroup = createElement('div', { className: 'form-group' });
    dueGroup.appendChild(createElement('label', { className: 'form-label' }, 'Дедлайн'));
    const dueInput = createElement('input', {
        className: `form-input ${hasDeadline ? 'form-input--locked' : ''}`,
        type: 'date',
        id: 'card-edit-due',
        value: cardData.due_date ? cardData.due_date.split('T')[0] : ''
    });
    if (hasDeadline) {
        // Именно `disabled`, а не `readonly`: у `<input type="date">` readonly
        // не мешает открыть календарь и выбрать в нём другую дату.
        dueInput.disabled = true;
    }
    dueGroup.appendChild(dueInput);
    dueGroup.appendChild(createElement('p', { className: 'form-hint' },
        hasDeadline
            ? 'Дедлайн задан и больше не меняется.'
            : 'Дедлайн задаётся один раз — после сохранения изменить его будет нельзя.'));
    body.appendChild(dueGroup);

    // ─── Исполнитель / Автор / Приоритет ───
    //
    // The card face shows an assignee avatar, so there has to be somewhere to
    // set one without going to the "Список" screen. Accepts both card shapes:
    // the board sends ids, the list screen sends whole member records.
    let assigneeId = cardData.assignee ? cardData.assignee.id : (cardData.assignee_id ?? null);
    let authorId = cardData.author ? cardData.author.id : (cardData.author_id ?? null);
    let priority = cardData.priority || 'Medium';

    const peopleRow = createElement('div', { className: 'card-modal__people' });

    const personField = (label, getId, setId, noneLabel) => {
        const group = createElement('div', { className: 'form-group card-modal__field' });
        group.appendChild(createElement('label', { className: 'form-label' }, label));
        const btn = createElement('button', { className: 'person-cell person-cell--wide', type: 'button' });
        const paint = () => {
            btn.innerHTML = '';
            btn.appendChild(createMemberChip(findMember(getId())));
        };
        paint();
        btn.addEventListener('click', () => {
            openMemberPicker(btn, getId(), (memberId) => { setId(memberId); paint(); },
                { allowNone: true, noneLabel });
        });
        group.appendChild(btn);
        return group;
    };

    peopleRow.appendChild(personField('Исполнитель', () => assigneeId, (v) => { assigneeId = v; }, 'Не назначен'));
    peopleRow.appendChild(personField('Автор', () => authorId, (v) => { authorId = v; }, 'Без автора'));

    const priorityGroup = createElement('div', { className: 'form-group card-modal__field' });
    priorityGroup.appendChild(createElement('label', { className: 'form-label' }, 'Приоритет'));
    const priorityBtn = createElement('button', { className: 'priority-pill', type: 'button' });
    const paintPriority = () => {
        priorityBtn.className = `priority-pill priority-pill--${priorityModifier(priority)}`;
        priorityBtn.innerHTML = '';
        priorityBtn.appendChild(createElement('span', { className: 'priority-pill__dot' }));
        priorityBtn.appendChild(createElement('span', {}, priorityLabel(priority)));
    };
    paintPriority();
    priorityBtn.addEventListener('click', () => {
        const menu = createElement('div', { className: 'context-menu' });
        for (const p of PRIORITIES) {
            const item = createElement('div', { className: 'context-menu__item' });
            item.appendChild(createElement('span', { className: `priority-dot priority-dot--${priorityModifier(p.value)}` }));
            item.appendChild(createElement('span', {}, p.label));
            item.addEventListener('click', () => {
                closePopovers();
                priority = p.value;
                paintPriority();
            });
            menu.appendChild(item);
        }
        openPopover(menu, priorityBtn, { placement: 'bottom', align: 'start', gap: 4 });
    });
    priorityGroup.appendChild(priorityBtn);
    peopleRow.appendChild(priorityGroup);

    body.appendChild(peopleRow);

    // «Повторять» — после переноса в финальную колонку появится следующая
    // задача (§36). Правило читается отдельно: в карточке, которую приносят
    // экраны, его нет. Пока не прочитано, список выключен, и «Сохранить» его
    // не трогает — иначе закрытое до ответа окно стёрло бы правило.
    const recurrenceGroup = createElement('div', { className: 'form-group card-modal__recurrence' });
    recurrenceGroup.appendChild(createElement('label', { className: 'form-label' }, 'Повторять'));
    const recurrenceSelect = createElement('select', { className: 'form-input', disabled: 'disabled' });
    for (const [value, label] of RECURRENCE_OPTIONS) {
        recurrenceSelect.appendChild(createElement('option', { value }, label));
    }
    recurrenceGroup.appendChild(recurrenceSelect);
    recurrenceGroup.appendChild(createElement('p', { className: 'form-hint' },
        'Когда задача попадёт в финальную колонку, появится следующая — с тем же названием и чек-листом ' +
        'и сроком на день, неделю или месяц позже.'));
    body.appendChild(recurrenceGroup);

    let savedRecurrence;  // undefined — ещё не прочитано
    api.getCardRecurrence(cardData.id)
        .then((rule) => {
            savedRecurrence = rule || '';
            recurrenceSelect.value = savedRecurrence;
            recurrenceSelect.disabled = false;
        })
        .catch((e) => console.error('Не удалось прочитать правило повторения:', e));

    // Пользовательские поля доски — сразу под стандартными. Доску карточки
    // знают все экраны, кроме самой доски: «Список», палитра, планировщик и
    // «Требуют внимания» приносят `board_id` в карточке, а `get_cards` его не
    // отдаёт — для доски он взят при её отрисовке.
    const customFields = renderCardFields(body, cardData.id, cardData.board_id ?? currentBoardId);

    // Loads asynchronously; the modal is already on screen by then.
    renderChecklist(body, cardData.id, () => { checklistDirty = true; });
    // Зависимости, как и чек-лист, пишутся сразу и видны на лицевой стороне
    // карточки значком звена — значит, доску после правок надо обновить.
    renderDependencies(body, cardData, {
        workspaceId,
        onChange: () => { checklistDirty = true; },
        // Открытие связанной карточки приходит отсюда, а не изнутри модуля
        // зависимостей: тот не импортирует `board.js`, иначе вышло бы кольцо.
        // Карточку он передаёт уже прочитанной целиком.
        onOpenCard: (linked) => showCardEditModal(linked, { onChange: () => {}, workspaceId }),
    });
    // Время, вписанное руками, пишется сразу. Лицевая сторона карточки его не
    // показывает, поэтому доску после этого обновлять не нужно.
    renderTimeLog(body, cardData.id);
    // Комментарии сохраняются сразу, поэтому окно про них ничего не помнит и
    // обновлять доску из-за них не нужно: на лицевой стороне карточки их нет.
    renderComments(body, cardData.id);

    modal.appendChild(body);
    
    // Footer
    const footer = createElement('div', { className: 'modal__footer' });
    
    const archiveBtn = createElement('button', {
        className: 'btn btn--danger',
        innerHTML: `${Icons.archive} Архивировать`
    });
    archiveBtn.addEventListener('click', async () => {
        const ok = await confirmDialog({
            title: 'Архивировать карточку?',
            message: `Карточка «${cardData.title}» пропадёт с доски. Вернуть её можно через «Архив доски».`,
            confirmText: 'Архивировать',
            danger: true,
        });
        if (!ok) return;

        try {
            await api.archiveCard(cardData.id);
            // Архивация останавливает таймер карточки на бэкенде — шапка должна
            // узнать об этом сейчас, а не через полминуты сверки.
            refreshTimer();
            overlay.remove();
            onChange();
            showToast('Карточка архивирована');
        } catch (e) {
            showToast('Не удалось архивировать карточку', 'error');
        }
    });
    footer.appendChild(archiveBtn);
    
    const saveBtn = createElement('button', {
        className: 'btn btn--primary'
    }, 'Сохранить');
    saveBtn.addEventListener('click', async () => {
        const title = titleInput.value.trim();
        if (!title) return;
        // Заданный срок уходит обратно как есть: поле выключено, менять
        // нечего, а бэкенд всё равно принимает дату только вместо пустой.
        const dueDate = hasDeadline ? cardData.due_date : (dueInput.value || null);
        // Поля проверяются до первой записи: иначе карточка сохранилась бы
        // наполовину — название уже в базе, а поле с ошибкой нет.
        let fieldChanges;
        try {
            fieldChanges = customFields.changes();
        } catch (e) {
            showToast(e.message, 'error');
            return;
        }
        try {
            await api.updateCard(cardData.id, title, descInput.value, dueDate);

            // Only what actually changed is written — each of these is its own
            // command, and a no-op UPDATE is still a write.
            const wasAssignee = cardData.assignee ? cardData.assignee.id : (cardData.assignee_id ?? null);
            const wasAuthor = cardData.author ? cardData.author.id : (cardData.author_id ?? null);
            if (assigneeId !== wasAssignee) await api.updateCardAssignee(cardData.id, assigneeId);
            if (authorId !== wasAuthor) await api.updateCardAuthor(cardData.id, authorId);
            if (priority !== (cardData.priority || 'Medium')) await api.updateCardPriority(cardData.id, priority);
            if (savedRecurrence !== undefined && recurrenceSelect.value !== savedRecurrence) {
                await api.setCardRecurrence(cardData.id, recurrenceSelect.value || null);
            }
            for (const { field, value } of fieldChanges) {
                await api.setCustomFieldValue(cardData.id, field.id, value);
            }

            overlay.remove();
            onChange();
            showToast('Карточка обновлена');
        } catch (e) {
            // Отказ по значению поля («Такого варианта у поля нет») приходит с
            // бэкенда строкой и объясняет, что не так, — его и показываем.
            showToast(typeof e === 'string' ? e : 'Не удалось сохранить карточку', 'error');
        }
    });
    footer.appendChild(saveBtn);
    
    modal.appendChild(footer);
    overlay.appendChild(modal);

    // Close on overlay click
    overlay.addEventListener('click', (e) => {
        if (e.target === overlay) dismiss();
    });
    // Picked up by the global Escape handler in initModalEscape().
    overlay.__onClose = dismiss;

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
        // Финальную колонку не сдвинуть с последнего места — она конец доски.
        filter: '.add-column, .column--final',
        // ...и правее неё ничего не встаёт. Без этого «Закрыто» саму никто не
        // трогает, но её легко обойти справа любой другой колонкой.
        onMove: (evt) => {
            if (evt.related
                && evt.related.classList.contains('column--final')
                && evt.willInsertAfter) {
                return false;
            }
            return true;
        },
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
                // Бэкенд отклонит порядок, в котором «Закрыто» не последняя.
                // Порядок на экране в этот момент уже не тот, что в базе, —
                // перерисовываем, иначе доска врала бы до перехода на неё.
                showToast(typeof e === 'string' ? e : 'Не удалось изменить порядок колонок', 'error');
                renderBoard(currentBoardId);
            }
        }
    });
    
    // Card drag-and-drop for each column
    const cardLists = $$('.column__cards', boardEl);
    for (const list of cardLists) {
        const isFinal = list.dataset.final === '1';
        const sortable = new Sortable(list, {
            // `pull: false` у финальной колонки — это и есть запрет вытащить
            // карточку: Sortable просто не отдаёт её другим спискам, а внутри
            // самой колонки порядок менять по-прежнему можно. Тот же запрет
            // продублирован в `update_card_position`: интерфейс обойти легко.
            group: { name: 'cards', pull: !isFinal, put: true },
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

                // Переезд в финальную колонку необратим, поэтому спрашиваем
                // ДО записи в базу, а не откатываем после. Отказ возвращает
                // карточку на прежнее место в DOM — в базу при этом не уходит
                // ничего, откатывать нечего.
                if (evt.to !== evt.from && evt.to.dataset.final === '1') {
                    // Общий вопрос на все три двери (доска, «Список», Inbox) —
                    // он же дописывает строку про незавершённые блокировки.
                    const target = columnsData.find(c => c.id === newColumnId);
                    const ok = await confirmFinalColumnMove({
                        cardId,
                        columnName: target ? target.name : '',
                    });
                    if (!ok) {
                        returnCardToItsPlace(evt);
                        return;
                    }
                }

                try {
                    const spawn = await api.updateCardPosition(cardId, newColumnId, newPosition);

                    // Update card counts
                    updateCardCounts();

                    if (spawn) {
                        // Следующая карточка повторяющейся задачи встала на эту
                        // же доску — перерисовка её покажет, тост скажет, где.
                        announceRecurringSpawn(spawn, currentWorkspaceId);
                        renderBoard(currentBoardId);
                    } else if (evt.to !== evt.from && evt.to.dataset.final === '1') {
                        showToast('Карточка в финальной колонке — вернуть её нельзя');
                    }
                } catch (e) {
                    console.error('Failed to move card:', e);
                    // Текст ошибки приходит с бэкенда (например, отказ выпустить
                    // карточку из финальной колонки) — показываем его, а не
                    // общую фразу: человек должен понять, почему не вышло.
                    showToast(typeof e === 'string' ? e : 'Не удалось переместить карточку', 'error');
                    renderBoard(currentBoardId);
                }
            }
        });
        cardSortables.push(sortable);
    }
}

/**
 * Возвращает карточку туда, откуда её потащили.
 *
 * Sortable уже перенёс узел в другой список, и `oldIndex` — это его индекс в
 * исходном списке ДО переноса. Раз узел оттуда исчез, вставка перед элементом
 * с тем же индексом кладёт его ровно на прежнее место.
 *
 * Работает только потому, что во время перетаскивания в списке нет ничего,
 * кроме карточек: при группировке и фильтрации drag-and-drop выключен
 * (`setCardDragEnabled`).
 */
function returnCardToItsPlace(evt) {
    evt.from.insertBefore(evt.item, evt.from.children[evt.oldIndex] || null);
    updateCardCounts();
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
