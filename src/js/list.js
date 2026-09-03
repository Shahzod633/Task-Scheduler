// ============================================
// TaskFlow — "Список" (workspace-wide task table)
// ============================================
// Every task in the workspace on one screen, across all of its boards — the
// view a board cannot give you, because a board only ever shows itself.
//
// Status is not a stored field: it is the name of the column a card sits in.
// Changing it here calls the same `update_card_position` that drag-and-drop
// does, so the card physically moves to the end of the chosen column instead of
// merely relabelling itself.

import * as api from './api.js';
import Icons from './icons.js';
import { openPopover, closePopovers } from './popover.js';
import { loadMembers, createAvatar, createMemberChip, openMemberPicker, findMember } from './members.js';
import {
    createFilterState, createFilterToolbar, matchesFilter, groupKeyFor,
    priorityLabel, priorityModifier, PRIORITIES,
} from './filters.js';
import { showCardEditModal } from './board.js';
import { confirmDialog } from './dialog.js';
import { createElement, $, showToast, formatDueDate, isOverdue, pluralize } from './utils.js';

// Kept across re-renders of the same screen so a refresh after an edit does not
// throw away the filter the user just set up.
let state = createFilterState();
let currentWorkspaceId = null;
let data = { cards: [], boards: [] };

export async function renderListPage(workspaceId) {
    if (workspaceId !== currentWorkspaceId) {
        // A different workspace has different boards and columns; carrying the
        // old filter over would silently hide everything.
        state = createFilterState();
        currentWorkspaceId = workspaceId;
    }

    const content = $('#content');
    content.innerHTML = '';
    content.classList.add('view-enter');

    const page = createElement('div', { className: 'page page--list' });
    page.appendChild(createElement('h2', { className: 'page__title' }, 'Список'));
    page.appendChild(createElement('p', { className: 'page__subtitle' },
        'Все задачи рабочего пространства — со всех досок сразу.'));

    const toolbarSlot = createElement('div', { className: 'list-page__toolbar' });
    page.appendChild(toolbarSlot);

    const tableSlot = createElement('div', { className: 'list-page__table' });
    page.appendChild(tableSlot);

    const footerSlot = createElement('div', { className: 'list-page__footer' });
    page.appendChild(footerSlot);

    content.appendChild(page);
    setTimeout(() => content.classList.remove('view-enter'), 420);

    try {
        [data] = await Promise.all([
            api.listAllCardsInWorkspace(workspaceId),
            loadMembers(true),
        ]);
    } catch (e) {
        console.error('Failed to load workspace cards:', e);
        tableSlot.appendChild(createElement('div', { className: 'page__empty' }, 'Не удалось загрузить задачи'));
        return;
    }

    const draw = () => {
        drawTable(tableSlot, footerSlot);
    };

    toolbarSlot.appendChild(createFilterToolbar({
        state,
        onChange: draw,
        boards: data.boards.map(b => ({ id: b.id, name: b.name })),
        statuses: uniqueStatuses(),
        showGroup: true,
    }));

    draw();
}

/** Column names across the whole workspace, de-duplicated, in board order. */
function uniqueStatuses() {
    const seen = new Set();
    const out = [];
    for (const board of data.boards) {
        for (const col of board.columns) {
            if (!seen.has(col.name)) { seen.add(col.name); out.push(col.name); }
        }
    }
    return out;
}

/** Reloads from the database and redraws, keeping the current filter. */
async function refresh() {
    try {
        data = await api.listAllCardsInWorkspace(currentWorkspaceId);
    } catch (e) {
        showToast('Не удалось обновить список', 'error');
        return;
    }
    drawTable($('.list-page__table'), $('.list-page__footer'));
}

/** The shape `filters.js` matches against. */
function toFilterItem(card) {
    return {
        title: card.title,
        boardId: card.board_id,
        boardName: card.board_name,
        assigneeId: card.assignee ? card.assignee.id : 0,
        assignee: card.assignee,
        status: card.column_name,
        priority: card.priority,
    };
}

function drawTable(tableSlot, footerSlot) {
    if (!tableSlot) return;
    tableSlot.innerHTML = '';
    footerSlot.innerHTML = '';

    const total = data.cards.length;
    const visible = data.cards.filter(c => matchesFilter(toFilterItem(c), state));

    if (total === 0) {
        tableSlot.appendChild(createElement('div', { className: 'page__empty' },
            'В этом пространстве пока нет задач'));
        return;
    }

    if (visible.length === 0) {
        tableSlot.appendChild(createElement('div', { className: 'page__empty' },
            'Ни одна задача не подходит под фильтр'));
    } else if (state.groupBy) {
        for (const [key, cards] of groupCards(visible)) {
            tableSlot.appendChild(createGroupSection(key, cards));
        }
    } else {
        tableSlot.appendChild(createTable(visible));
    }

    footerSlot.appendChild(createElement('div', { className: 'filter-count' },
        `${visible.length} из ${total} ${pluralize(total, ['задачи', 'задач', 'задач'])}`));
}

/** Ordered map of group heading → cards, preserving the table's sort order. */
function groupCards(cards) {
    const groups = new Map();
    for (const card of cards) {
        const key = groupKeyFor(toFilterItem(card), state.groupBy);
        if (!groups.has(key)) groups.set(key, []);
        groups.get(key).push(card);
    }
    return groups;
}

function createGroupSection(title, cards) {
    const section = createElement('div', { className: 'list-group' });

    const header = createElement('div', { className: 'list-group__header' });
    const chevron = createElement('span', { className: 'list-group__chevron', innerHTML: Icons.chevronDown });
    header.appendChild(chevron);
    header.appendChild(createElement('span', { className: 'list-group__title' }, title));
    header.appendChild(createElement('span', { className: 'list-group__count' }, String(cards.length)));
    section.appendChild(header);

    const body = createElement('div', { className: 'list-group__body' });
    body.appendChild(createTable(cards));
    section.appendChild(body);

    header.addEventListener('click', () => {
        const collapsed = section.classList.toggle('list-group--collapsed');
        chevron.style.transform = collapsed ? 'rotate(-90deg)' : '';
    });

    return section;
}

const HEADERS = ['Задача', 'Доска', 'Исполнитель', 'Автор', 'Приоритет', 'Статус', 'Срок'];

function createTable(cards) {
    const table = createElement('table', { className: 'task-table' });

    const thead = createElement('thead');
    const headRow = createElement('tr');
    for (const label of HEADERS) {
        headRow.appendChild(createElement('th', { className: 'task-table__th' }, label));
    }
    thead.appendChild(headRow);
    table.appendChild(thead);

    const tbody = createElement('tbody');
    for (const card of cards) {
        tbody.appendChild(createRow(card));
    }
    table.appendChild(tbody);

    // Wrapped so a narrow window scrolls the table rather than the page.
    const wrap = createElement('div', { className: 'task-table__wrap' });
    wrap.appendChild(table);
    return wrap;
}

function createRow(card) {
    const row = createElement('tr', { className: 'task-table__row' });

    // ─── Задача ───
    const titleCell = createElement('td', { className: 'task-table__td task-table__td--title' });
    const titleBtn = createElement('button', { className: 'task-table__title' }, card.title);
    titleBtn.addEventListener('click', () => {
        // The card modal is the board's, reused as-is: one editor, one set of
        // rules about what a card is.
        showCardEditModal(card, { onChange: refresh });
    });
    titleCell.appendChild(titleBtn);
    if (card.is_mistake) {
        titleCell.appendChild(createElement('span', {
            className: 'task-table__flag',
            innerHTML: Icons.alertTriangle,
            'data-tooltip': 'Отмечена как ошибка'
        }));
    }
    row.appendChild(titleCell);

    // ─── Доска ───
    const boardCell = createElement('td', { className: 'task-table__td' });
    const boardBadge = createElement('button', {
        className: 'board-badge',
        'data-tooltip': 'Открыть доску'
    }, card.board_name);
    boardBadge.addEventListener('click', () => {
        window.dispatchEvent(new CustomEvent('navigate', {
            detail: { view: 'board', boardId: card.board_id }
        }));
    });
    boardCell.appendChild(boardBadge);
    row.appendChild(boardCell);

    // ─── Исполнитель ───
    row.appendChild(createPersonCell(card, 'assignee'));

    // ─── Автор ───
    row.appendChild(createPersonCell(card, 'author'));

    // ─── Приоритет ───
    const priorityCell = createElement('td', { className: 'task-table__td' });
    const priorityBtn = createElement('button', {
        className: `priority-pill priority-pill--${priorityModifier(card.priority)}`
    });
    priorityBtn.appendChild(createElement('span', { className: 'priority-pill__dot' }));
    priorityBtn.appendChild(createElement('span', {}, priorityLabel(card.priority)));
    priorityBtn.addEventListener('click', () => openPriorityPicker(priorityBtn, card.priority, async (value) => {
        try {
            await api.updateCardPriority(card.id, value);
            card.priority = value;
            await refresh();
        } catch (e) {
            showToast('Не удалось изменить приоритет', 'error');
        }
    }));
    priorityCell.appendChild(priorityBtn);
    row.appendChild(priorityCell);

    // ─── Статус ───
    const statusCell = createElement('td', { className: 'task-table__td' });
    const statusBtn = createElement('button', { className: 'status-pill' });
    statusBtn.appendChild(createElement('span', {}, card.column_name));
    statusBtn.appendChild(createElement('span', { className: 'status-pill__chevron', innerHTML: Icons.chevronDown }));
    statusBtn.addEventListener('click', () => openStatusPicker(statusBtn, card));
    statusCell.appendChild(statusBtn);
    row.appendChild(statusCell);

    // ─── Срок ───
    const dueCell = createElement('td', { className: 'task-table__td' });
    if (card.due_date) {
        dueCell.appendChild(createElement('span', {
            className: `task-table__due ${isOverdue(card.due_date) ? 'task-table__due--overdue' : ''}`,
            innerHTML: `${Icons.clock} <span>${formatDueDate(card.due_date)}</span>`
        }));
    } else {
        dueCell.appendChild(createElement('span', { className: 'task-table__empty' }, '—'));
    }
    row.appendChild(dueCell);

    return row;
}

/** One editable person cell — same widget for assignee and author. */
function createPersonCell(card, field) {
    const cell = createElement('td', { className: 'task-table__td' });
    const member = card[field];

    const btn = createElement('button', { className: 'person-cell' });
    btn.appendChild(createMemberChip(member));
    btn.addEventListener('click', () => {
        openMemberPicker(btn, member ? member.id : null, async (memberId) => {
            try {
                if (field === 'assignee') await api.updateCardAssignee(card.id, memberId);
                else await api.updateCardAuthor(card.id, memberId);
                card[field] = findMember(memberId);
                await refresh();
            } catch (e) {
                showToast('Не удалось изменить участника', 'error');
            }
        }, { allowNone: true, noneLabel: field === 'author' ? 'Без автора' : 'Не назначен' });
    });

    cell.appendChild(btn);
    return cell;
}

function openPriorityPicker(anchor, current, onPick) {
    const menu = createElement('div', { className: 'context-menu' });
    for (const p of PRIORITIES) {
        const isCurrent = p.value === current;
        const row = createElement('div', {
            className: `context-menu__item ${isCurrent ? 'context-menu__item--current' : ''}`
        });
        row.appendChild(createElement('span', { className: `priority-dot priority-dot--${priorityModifier(p.value)}` }));
        row.appendChild(createElement('span', {}, p.label));
        if (isCurrent) {
            row.appendChild(createElement('span', { className: 'member-picker__check', innerHTML: Icons.check }));
        }
        row.addEventListener('click', () => {
            closePopovers();
            if (isCurrent) return;
            onPick(p.value);
        });
        menu.appendChild(row);
    }
    openPopover(menu, anchor, { placement: 'bottom', align: 'start', gap: 4 });
}

/**
 * Offers the columns of *this card's* board — different boards have different
 * columns, so a single merged list would let you "move" a card to a column that
 * does not exist on its board.
 */
function openStatusPicker(anchor, card) {
    const board = data.boards.find(b => b.id === card.board_id);
    const menu = createElement('div', { className: 'context-menu' });

    if (!board || board.columns.length === 0) {
        menu.appendChild(createElement('div', { className: 'header-popover__empty' }, 'У доски нет колонок'));
        openPopover(menu, anchor, { placement: 'bottom', align: 'start', gap: 4 });
        return;
    }

    for (const col of board.columns) {
        const isCurrent = col.id === card.column_id;
        const row = createElement('div', {
            className: `context-menu__item ${isCurrent ? 'context-menu__item--current' : ''}`
        });
        row.appendChild(createElement('span', {}, col.name));
        // Финальная колонка помечена замком прямо в списке: выбор её здесь
        // так же необратим, как перенос карточки мышью на доске.
        if (col.is_final) {
            row.appendChild(createElement('span', {
                className: 'context-menu__lock',
                innerHTML: Icons.lock,
                'data-tooltip': 'Финальная колонка: вернуть задачу будет нельзя'
            }));
        }
        if (isCurrent) {
            row.appendChild(createElement('span', { className: 'member-picker__check', innerHTML: Icons.check }));
        }
        row.addEventListener('click', async () => {
            closePopovers();
            if (isCurrent) return;

            // Тот же вопрос, что и при перетаскивании на доске: смена статуса
            // здесь — это тот же перенос карточки, и запирает её так же
            // насовсем. Спрашиваем до записи, а не откатываем после.
            if (col.is_final) {
                const ok = await confirmDialog({
                    title: 'Перенести в финальную колонку?',
                    message: `Перемещение в «${col.name}» необратимо — задачу нельзя будет вернуть обратно.`,
                    confirmText: 'Подтвердить',
                    danger: true,
                });
                if (!ok) return;
            }

            try {
                // Same command drag-and-drop uses. The card lands at the end of
                // the target column: `data.cards` holds only unarchived cards,
                // so counting them gives the next free position.
                const endPosition = data.cards.filter(c => c.column_id === col.id).length;
                await api.updateCardPosition(card.id, col.id, endPosition);
                await refresh();
                showToast(`Задача перенесена в «${col.name}»`);
            } catch (e) {
                // Бэкенд отказывает, если задача уже в финальной колонке —
                // его текст объясняет причину лучше общей фразы.
                showToast(typeof e === 'string' ? e : 'Не удалось сменить статус', 'error');
            }
        });
        menu.appendChild(row);
    }

    openPopover(menu, anchor, { placement: 'bottom', align: 'start', gap: 4 });
}
