// ============================================
// TaskFlow — Members (local directory of people)
// ============================================
// Not accounts. There is no login, no password and no server: a member is a
// name and a coloured circle that a card can point at. The app remains a single
// offline SQLite file on one machine, exactly as it was.
//
// This module owns three things everyone else reuses: the cached member list,
// the avatar element, and the "pick a person" dropdown.

import * as api from './api.js';
import Icons from './icons.js';
import { openPopover, closePopovers } from './popover.js';
import { confirmDialog } from './dialog.js';
import { createElement, $, showToast, pluralize } from './utils.js';

/** Palette offered when recolouring a member — mirrors MEMBER_COLORS in Rust. */
const MEMBER_COLORS = [
    '#6366f1', '#ec4899', '#f59e0b', '#10b981',
    '#3b82f6', '#8b5cf6', '#ef4444', '#14b8a6',
];

// The directory is small and read on almost every screen, so it is fetched once
// and re-used. Every mutation below refreshes it, and `loadMembers(true)`
// forces a reload for callers that cannot be sure.
let cache = null;

export async function loadMembers(force = false) {
    if (cache && !force) return cache;
    cache = await api.listMembers();
    return cache;
}

/** The cached list without a round trip. Empty until `loadMembers` has run. */
export function getMembers() {
    return cache || [];
}

export function findMember(id) {
    if (id === null || id === undefined) return null;
    return getMembers().find(m => m.id === id) || null;
}

export function invalidateMembers() {
    cache = null;
}

// ─── Avatar ───

/**
 * The coloured circle with initials shown wherever a person appears.
 *
 * @param {object|null} member - a member record, or null for "не назначен"
 * @param {object} opts - { size: 'sm'|'md'|'lg', tooltip: boolean }
 */
export function createAvatar(member, opts = {}) {
    const { size = 'md', tooltip = true } = opts;

    if (!member) {
        return createElement('div', {
            className: `member-avatar member-avatar--${size} member-avatar--empty`,
            innerHTML: Icons.user,
            ...(tooltip ? { 'data-tooltip': 'Не назначен' } : {}),
        });
    }

    return createElement('div', {
        className: `member-avatar member-avatar--${size}`,
        style: { background: member.color },
        ...(tooltip ? { 'data-tooltip': member.name } : {}),
    }, member.initials || '?');
}

/** Avatar plus name, the form used in table cells and dropdown rows. */
export function createMemberChip(member, opts = {}) {
    const chip = createElement('div', { className: 'member-chip' });
    chip.appendChild(createAvatar(member, { size: 'sm', tooltip: false, ...opts }));
    chip.appendChild(createElement('span', {
        className: `member-chip__name ${member ? '' : 'member-chip__name--empty'}`
    }, member ? member.name : 'Не назначен'));
    return chip;
}

// ─── "Pick a person" dropdown ───

/**
 * Opens the member picker anchored to `anchor`.
 *
 * @param {HTMLElement} anchor
 * @param {number|null} currentId - currently selected member
 * @param {(memberId: number|null) => void} onPick
 * @param {object} opts - { allowNone: boolean, noneLabel: string }
 */
export function openMemberPicker(anchor, currentId, onPick, opts = {}) {
    const { allowNone = true, noneLabel = 'Не назначен' } = opts;

    const menu = createElement('div', { className: 'context-menu member-picker' });

    const addRow = (member, label) => {
        const isCurrent = member ? member.id === currentId : currentId == null;
        const row = createElement('div', {
            className: `context-menu__item member-picker__row ${isCurrent ? 'member-picker__row--current' : ''}`
        });
        row.appendChild(createAvatar(member, { size: 'sm', tooltip: false }));
        row.appendChild(createElement('span', {}, label));
        if (isCurrent) {
            row.appendChild(createElement('span', { className: 'member-picker__check', innerHTML: Icons.check }));
        }
        row.addEventListener('click', () => {
            closePopovers();
            // Re-picking the same person is a no-op rather than a pointless write.
            if (isCurrent) return;
            onPick(member ? member.id : null);
        });
        menu.appendChild(row);
    };

    if (allowNone) addRow(null, noneLabel);
    for (const member of getMembers()) {
        addRow(member, member.is_self ? `${member.name} (вы)` : member.name);
    }

    openPopover(menu, anchor, { placement: 'bottom', align: 'start', gap: 4 });
}

// ─── Members screen ───

export async function renderMembersPage(workspaceId) {
    const content = $('#content');
    content.innerHTML = '';
    content.classList.add('view-enter');

    const page = createElement('div', { className: 'page page--members' });
    page.appendChild(createElement('h2', { className: 'page__title' }, 'Участники'));
    page.appendChild(createElement('p', { className: 'page__subtitle' },
        'Список имён для полей «Исполнитель» и «Автор» на карточках. Это не учётные записи: ' +
        'ни входа, ни паролей, ни синхронизации — всё хранится в том же локальном файле, что и доски.'));

    const card = createElement('div', { className: 'settings-card' });
    page.appendChild(card);

    content.appendChild(page);
    setTimeout(() => content.classList.remove('view-enter'), 420);

    await renderMembersList(card);
}

async function renderMembersList(container) {
    container.innerHTML = '';

    let members;
    try {
        members = await loadMembers(true);
    } catch (e) {
        container.appendChild(createElement('p', { className: 'form-hint' }, 'Не удалось загрузить участников'));
        return;
    }

    const refresh = () => renderMembersList(container);

    const list = createElement('div', { className: 'member-list' });
    for (const member of members) {
        list.appendChild(createMemberRow(member, refresh));
    }
    container.appendChild(list);

    container.appendChild(createElement('div', { className: 'member-list__count' },
        `${members.length} ${pluralize(members.length, ['участник', 'участника', 'участников'])}`));

    const addBtn = createElement('button', {
        className: 'btn btn--secondary',
        innerHTML: `${Icons.plus} <span>Добавить участника</span>`
    });
    addBtn.addEventListener('click', () => showAddMemberForm(container, addBtn, refresh));
    container.appendChild(addBtn);
}

function createMemberRow(member, refresh) {
    const row = createElement('div', { className: 'member-row' });

    const swatch = createAvatar(member, { size: 'md', tooltip: false });
    swatch.classList.add('member-row__avatar');
    swatch.setAttribute('data-tooltip', 'Сменить цвет');
    swatch.addEventListener('click', () => openColorPicker(swatch, member, refresh));
    row.appendChild(swatch);

    const info = createElement('div', { className: 'member-row__info' });
    const nameEl = createElement('span', { className: 'member-row__name' }, member.name);
    info.appendChild(nameEl);
    if (member.is_self) {
        info.appendChild(createElement('span', { className: 'member-row__badge' }, 'это вы'));
    }
    row.appendChild(info);

    const actions = createElement('div', { className: 'member-row__actions' });

    const renameBtn = createElement('button', {
        className: 'icon-btn',
        innerHTML: Icons.edit,
        'data-tooltip': 'Переименовать'
    });
    renameBtn.addEventListener('click', () => startRename(row, nameEl, member, refresh));
    actions.appendChild(renameBtn);

    // The user's own row has no delete button: every card's author points at it,
    // and there is no second person to hand the app over to.
    if (!member.is_self) {
        const deleteBtn = createElement('button', {
            className: 'icon-btn icon-btn--danger',
            innerHTML: Icons.trash,
            'data-tooltip': 'Удалить участника'
        });
        deleteBtn.addEventListener('click', () => removeMember(member, refresh));
        actions.appendChild(deleteBtn);
    }

    row.appendChild(actions);
    return row;
}

function startRename(row, nameEl, member, refresh) {
    if (row.querySelector('.member-row__input')) return;

    const input = createElement('input', { className: 'form-input member-row__input' });
    input.value = member.name;
    nameEl.style.display = 'none';
    nameEl.parentNode.insertBefore(input, nameEl);
    input.focus();
    input.select();

    let finished = false;
    const finish = async (save) => {
        if (finished) return;
        finished = true;
        const name = input.value.trim();
        input.remove();
        nameEl.style.display = '';

        if (!save || !name || name === member.name) return;
        try {
            // Initials are re-derived from the new name — passing null lets the
            // backend do it, so the two never drift apart.
            await api.updateMember(member.id, name, member.color, null);
            invalidateMembers();
            showToast('Участник переименован');
            refresh();
        } catch (e) {
            showToast('Не удалось переименовать участника', 'error');
        }
    };

    input.addEventListener('blur', () => finish(true));
    input.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') finish(true);
        if (e.key === 'Escape') finish(false);
    });
}

function openColorPicker(anchor, member, refresh) {
    const menu = createElement('div', { className: 'context-menu color-picker' });
    const grid = createElement('div', { className: 'color-picker__grid' });

    for (const color of MEMBER_COLORS) {
        const dot = createElement('button', {
            className: `color-picker__dot ${color === member.color ? 'color-picker__dot--current' : ''}`,
            style: { background: color },
            type: 'button'
        });
        dot.addEventListener('click', async () => {
            closePopovers();
            if (color === member.color) return;
            try {
                await api.updateMember(member.id, member.name, color, member.initials);
                invalidateMembers();
                refresh();
            } catch (e) {
                showToast('Не удалось сменить цвет', 'error');
            }
        });
        grid.appendChild(dot);
    }

    menu.appendChild(grid);
    openPopover(menu, anchor, { placement: 'bottom', align: 'start', gap: 4 });
}

async function removeMember(member, refresh) {
    const ok = await confirmDialog({
        title: 'Удалить участника?',
        message: `«${member.name}» пропадёт из списка, а карточки, где он назначен исполнителем ` +
                 `или автором, останутся без него. Сами карточки не пострадают.`,
        confirmText: 'Удалить',
        danger: true,
    });
    if (!ok) return;

    try {
        await api.deleteMember(member.id);
        invalidateMembers();
        showToast('Участник удалён');
        refresh();
    } catch (e) {
        showToast(String(e), 'error');
    }
}

function showAddMemberForm(container, addBtn, refresh) {
    if (container.querySelector('.member-add-form')) return;
    addBtn.style.display = 'none';

    const form = createElement('div', { className: 'member-add-form' });
    const input = createElement('input', {
        className: 'form-input',
        placeholder: 'Имя участника...'
    });
    form.appendChild(input);

    const submit = async () => {
        const name = input.value.trim();
        if (!name) { cancel(); return; }
        try {
            await api.createMember(name);
            invalidateMembers();
            showToast(`Участник «${name}» добавлен`);
            refresh();
        } catch (e) {
            showToast(String(e), 'error');
        }
    };

    const cancel = () => {
        form.remove();
        addBtn.style.display = '';
    };

    const confirmBtn = createElement('button', { className: 'btn btn--primary btn--sm' }, 'Добавить');
    confirmBtn.addEventListener('click', submit);
    form.appendChild(confirmBtn);

    const cancelBtn = createElement('button', { className: 'btn btn--ghost btn--sm' }, 'Отмена');
    cancelBtn.addEventListener('click', cancel);
    form.appendChild(cancelBtn);

    container.insertBefore(form, addBtn);
    input.focus();
    input.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') submit();
        if (e.key === 'Escape') cancel();
    });
}
