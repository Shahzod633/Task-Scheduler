// ============================================
// TaskFlow — Shared filter / group toolbar
// ============================================
// The "Список" screen and the kanban board need the same controls over
// different data, so the toolbar, the filter state and the matching rule live
// here once. Each screen supplies its own rows and decides what to do with the
// result — the list hides table rows, the board dims cards.
//
// Filtering is display-only. Nothing here writes to the database: a filtered
// card keeps its column and its position exactly as it was.

import Icons from './icons.js';
import { openPopover, closePopovers } from './popover.js';
import { createAvatar, getMembers } from './members.js';
import { createElement, debounce } from './utils.js';

/** Sentinel used in the assignee filter for "не назначен". Real ids start at 1. */
export const UNASSIGNED = 0;

export const PRIORITIES = [
    { value: 'High', label: 'Высокий' },
    { value: 'Medium', label: 'Средний' },
    { value: 'Low', label: 'Низкий' },
];

export function priorityLabel(value) {
    const found = PRIORITIES.find(p => p.value === value);
    return found ? found.label : 'Средний';
}

/** Modifier suffix for the CSS classes that colour priorities. */
export function priorityModifier(value) {
    return (value || 'Medium').toLowerCase();
}

export const GROUP_FIELDS = [
    { value: null, label: 'Без группировки' },
    { value: 'board', label: 'Доска' },
    { value: 'status', label: 'Статус' },
    { value: 'assignee', label: 'Исполнитель' },
    { value: 'priority', label: 'Приоритет' },
];

export function createFilterState() {
    return {
        search: '',
        boards: new Set(),
        assignees: new Set(),
        statuses: new Set(),
        priorities: new Set(),
        groupBy: null,
    };
}

/** True if anything is currently narrowing the view. */
export function isFilterActive(state) {
    return Boolean(state.search)
        || state.boards.size > 0
        || state.assignees.size > 0
        || state.statuses.size > 0
        || state.priorities.size > 0;
}

export function clearFilterState(state) {
    state.search = '';
    state.boards.clear();
    state.assignees.clear();
    state.statuses.clear();
    state.priorities.clear();
}

/**
 * Does one card pass the current filter?
 *
 * Within a field the selected values are OR-ed (two people means "either of
 * them"); across fields they are AND-ed, as the brief asks.
 *
 * @param {object} item - { title, boardId, assigneeId, status, priority }
 */
export function matchesFilter(item, state) {
    if (state.search) {
        const needle = state.search.trim().toLowerCase();
        if (needle && !(item.title || '').toLowerCase().includes(needle)) return false;
    }
    if (state.boards.size && !state.boards.has(item.boardId)) return false;
    if (state.assignees.size && !state.assignees.has(item.assigneeId ?? UNASSIGNED)) return false;
    if (state.statuses.size && !state.statuses.has(item.status)) return false;
    if (state.priorities.size && !state.priorities.has(item.priority || 'Medium')) return false;
    return true;
}

/** The value a card is grouped under, and the heading to print for it. */
export function groupKeyFor(item, groupBy) {
    switch (groupBy) {
        case 'board': return item.boardName || '—';
        case 'status': return item.status || '—';
        case 'assignee': return item.assignee ? item.assignee.name : 'Не назначен';
        case 'priority': return priorityLabel(item.priority);
        default: return '';
    }
}

/**
 * Builds the toolbar: search, quick assignee avatars, "Фильтр", "Группировать"
 * and a reset button.
 *
 * @param {object} cfg
 * @param {object} cfg.state - filter state, mutated in place
 * @param {() => void} cfg.onChange - called after every change
 * @param {object[]|null} cfg.boards - [{id, name}] or null to hide the board facet
 * @param {string[]} cfg.statuses - column names offered as statuses
 * @param {boolean} cfg.showGroup - whether to offer grouping
 * @param {string} cfg.searchPlaceholder
 */
export function createFilterToolbar(cfg) {
    const { state, onChange, boards = null, statuses = [], showGroup = true,
            searchPlaceholder = 'Поиск по названию задачи' } = cfg;

    const bar = createElement('div', { className: 'filter-bar' });

    // ─── Search ───
    const searchWrap = createElement('div', { className: 'filter-bar__search' });
    searchWrap.appendChild(createElement('span', { className: 'filter-bar__search-icon', innerHTML: Icons.search }));
    const searchInput = createElement('input', {
        className: 'filter-bar__search-input',
        placeholder: searchPlaceholder,
        type: 'text'
    });
    searchInput.value = state.search;
    searchInput.addEventListener('input', debounce((e) => {
        state.search = e.target.value;
        onChange();
        syncResetButton();
    }, 150));
    searchWrap.appendChild(searchInput);
    bar.appendChild(searchWrap);

    // ─── Quick assignee avatars ───
    const quick = createElement('div', { className: 'filter-bar__avatars' });
    const rebuildQuick = () => {
        quick.innerHTML = '';
        for (const member of getMembers()) {
            const avatar = createAvatar(member, { size: 'sm', tooltip: false });
            avatar.classList.add('filter-bar__avatar');
            avatar.setAttribute('data-tooltip', member.name);
            if (state.assignees.has(member.id)) avatar.classList.add('filter-bar__avatar--on');
            avatar.addEventListener('click', () => {
                toggle(state.assignees, member.id);
                avatar.classList.toggle('filter-bar__avatar--on', state.assignees.has(member.id));
                onChange();
                syncResetButton();
            });
            quick.appendChild(avatar);
        }
    };
    rebuildQuick();
    bar.appendChild(quick);

    const spacer = createElement('div', { className: 'filter-bar__spacer' });
    bar.appendChild(spacer);

    // ─── Filter panel ───
    const filterBtn = createElement('button', {
        className: 'filter-bar__btn',
        innerHTML: `${Icons.filter} <span>Фильтр</span>`
    });
    filterBtn.addEventListener('click', () => {
        openFilterPanel(filterBtn, { state, boards, statuses, onChange: () => {
            onChange();
            syncResetButton();
            syncFilterButton();
        }});
    });
    bar.appendChild(filterBtn);

    // ─── Group panel ───
    let groupBtn = null;
    if (showGroup) {
        groupBtn = createElement('button', {
            className: 'filter-bar__btn',
            innerHTML: `${Icons.layers} <span>Группировать</span>`
        });
        groupBtn.addEventListener('click', () => {
            openGroupPanel(groupBtn, state, () => {
                onChange();
                syncGroupButton();
            });
        });
        bar.appendChild(groupBtn);
    }

    // ─── Reset ───
    const resetBtn = createElement('button', {
        className: 'filter-bar__btn filter-bar__btn--reset',
        innerHTML: `${Icons.x} <span>Сбросить</span>`
    });
    resetBtn.addEventListener('click', () => {
        clearFilterState(state);
        searchInput.value = '';
        rebuildQuick();
        onChange();
        syncResetButton();
        syncFilterButton();
    });
    bar.appendChild(resetBtn);

    function syncResetButton() {
        resetBtn.classList.toggle('filter-bar__btn--hidden', !isFilterActive(state));
    }
    function syncFilterButton() {
        const n = state.boards.size + state.assignees.size + state.statuses.size + state.priorities.size;
        filterBtn.classList.toggle('filter-bar__btn--on', n > 0);
        const label = filterBtn.querySelector('span');
        if (label) label.textContent = n > 0 ? `Фильтр (${n})` : 'Фильтр';
    }
    function syncGroupButton() {
        if (!groupBtn) return;
        const field = GROUP_FIELDS.find(f => f.value === state.groupBy);
        groupBtn.classList.toggle('filter-bar__btn--on', Boolean(state.groupBy));
        const label = groupBtn.querySelector('span');
        if (label) label.textContent = state.groupBy ? `Группировка: ${field.label}` : 'Группировать';
    }

    syncResetButton();
    syncFilterButton();
    syncGroupButton();

    return bar;
}

function toggle(set, value) {
    if (set.has(value)) set.delete(value);
    else set.add(value);
}

function openFilterPanel(anchor, { state, boards, statuses, onChange }) {
    const panel = createElement('div', { className: 'context-menu filter-panel' });

    const section = (title, options, set) => {
        if (!options.length) return;
        panel.appendChild(createElement('div', { className: 'filter-panel__title' }, title));
        const group = createElement('div', { className: 'filter-panel__group' });
        for (const opt of options) {
            const row = createElement('label', { className: 'filter-panel__row' });
            const box = createElement('input', { type: 'checkbox', className: 'filter-panel__check' });
            box.checked = set.has(opt.value);
            box.addEventListener('change', () => {
                toggle(set, opt.value);
                onChange();
            });
            row.appendChild(box);
            if (opt.node) row.appendChild(opt.node);
            row.appendChild(createElement('span', { className: 'filter-panel__label' }, opt.label));
            group.appendChild(row);
        }
        panel.appendChild(group);
    };

    if (boards && boards.length > 1) {
        section('Доска', boards.map(b => ({ value: b.id, label: b.name })), state.boards);
    }

    const memberOptions = [
        { value: UNASSIGNED, label: 'Не назначен', node: createAvatar(null, { size: 'sm', tooltip: false }) },
        ...getMembers().map(m => ({
            value: m.id,
            label: m.is_self ? `${m.name} (вы)` : m.name,
            node: createAvatar(m, { size: 'sm', tooltip: false }),
        })),
    ];
    section('Исполнитель', memberOptions, state.assignees);

    section('Статус', statuses.map(s => ({ value: s, label: s })), state.statuses);

    section('Приоритет', PRIORITIES.map(p => ({
        value: p.value,
        label: p.label,
        node: createElement('span', { className: `priority-dot priority-dot--${priorityModifier(p.value)}` }),
    })), state.priorities);

    openPopover(panel, anchor, { placement: 'bottom', align: 'end', gap: 6, width: 260 });
}

function openGroupPanel(anchor, state, onChange) {
    const menu = createElement('div', { className: 'context-menu' });

    for (const field of GROUP_FIELDS) {
        const isCurrent = field.value === state.groupBy;
        const row = createElement('div', {
            className: `context-menu__item ${isCurrent ? 'context-menu__item--current' : ''}`
        });
        row.appendChild(createElement('span', {}, field.label));
        if (isCurrent) {
            row.appendChild(createElement('span', { className: 'member-picker__check', innerHTML: Icons.check }));
        }
        row.addEventListener('click', () => {
            closePopovers();
            if (isCurrent) return;
            state.groupBy = field.value;
            onChange();
        });
        menu.appendChild(row);
    }

    openPopover(menu, anchor, { placement: 'bottom', align: 'end', gap: 6 });
}

/**
 * "N из M" — how much of the workspace is currently on screen.
 */
export function createCountLabel(shown, total) {
    return createElement('div', { className: 'filter-count' },
        `${shown} из ${total}`);
}
