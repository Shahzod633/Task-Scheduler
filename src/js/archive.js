// ============================================
// TaskFlow — Archive (doubles as the trash)
// ============================================
// Archiving hides an item but keeps it recoverable; deleting removes it for
// good. Both live on the same screen so there is exactly one place to look for
// something that disappeared.
//
// Before this module, archiving a card or a column was a one-way trip: the
// backend had no restore for either, so "Архивировать" was effectively an
// unconfirmed delete.

import * as api from './api.js';
import Icons from './icons.js';
import { confirmDialog } from './dialog.js';
import { createElement, $, showToast } from './utils.js';

/**
 * One row of an archive list: what it is, plus restore and delete actions.
 * Shared by the board archive (cards/columns) and the closed-boards list.
 *
 * @param {object}   opts
 * @param {string}   opts.title
 * @param {string}  [opts.subtitle]
 * @param {Function} opts.onRestore - async
 * @param {Function} opts.onDelete  - async
 */
export function createArchiveRow({ title, subtitle, onRestore, onDelete }) {
    const row = createElement('div', { className: 'archive-row' });

    const main = createElement('div', { className: 'archive-row__main' });
    main.appendChild(createElement('span', { className: 'archive-row__title' }, title));
    if (subtitle) {
        main.appendChild(createElement('span', { className: 'archive-row__subtitle' }, subtitle));
    }
    row.appendChild(main);

    const actions = createElement('div', { className: 'archive-row__actions' });

    const restoreBtn = createElement('button', {
        className: 'btn btn--secondary btn--sm',
        innerHTML: `${Icons.rotateCcw} <span>Восстановить</span>`,
    });
    restoreBtn.addEventListener('click', () => onRestore());
    actions.appendChild(restoreBtn);

    const deleteBtn = createElement('button', {
        className: 'btn btn--danger btn--sm',
        innerHTML: `${Icons.trash} <span>Удалить</span>`,
        'data-tooltip': 'Удалить навсегда',
    });
    deleteBtn.addEventListener('click', () => onDelete());
    actions.appendChild(deleteBtn);

    row.appendChild(actions);
    return row;
}

export function createArchiveEmptyState(text) {
    return createElement('div', { className: 'archive-empty' }, text);
}

/**
 * Opens the archive of a single board: its archived cards and columns.
 *
 * @param {number}   boardId
 * @param {string}   boardName
 * @param {Function} onChange - called after anything is restored or deleted,
 *                              so the board behind the modal re-renders.
 */
export async function showBoardArchive(boardId, boardName, onChange = () => {}) {
    const existing = $('.modal-overlay');
    if (existing) existing.remove();

    const overlay = createElement('div', { className: 'modal-overlay' });
    const modal = createElement('div', { className: 'modal modal--wide' });

    const header = createElement('div', { className: 'modal__header' });
    header.appendChild(createElement('h2', { className: 'modal__title' }, `Архив доски «${boardName}»`));
    const closeBtn = createElement('button', { className: 'modal__close', innerHTML: Icons.x });
    closeBtn.addEventListener('click', () => overlay.remove());
    header.appendChild(closeBtn);
    modal.appendChild(header);

    const body = createElement('div', { className: 'modal__body' });

    const tabs = createElement('div', { className: 'archive-tabs' });
    const cardsTab = createElement('button', { className: 'archive-tab archive-tab--active' }, 'Карточки');
    const columnsTab = createElement('button', { className: 'archive-tab' }, 'Колонки');
    tabs.appendChild(cardsTab);
    tabs.appendChild(columnsTab);
    body.appendChild(tabs);

    const list = createElement('div', { className: 'archive-list' });
    body.appendChild(list);

    modal.appendChild(body);
    overlay.appendChild(modal);
    overlay.addEventListener('click', (e) => {
        if (e.target === overlay) overlay.remove();
    });
    document.body.appendChild(overlay);

    let activeTab = 'cards';

    cardsTab.addEventListener('click', () => { activeTab = 'cards'; refresh(); });
    columnsTab.addEventListener('click', () => { activeTab = 'columns'; refresh(); });

    /** Re-reads the archive from the backend and repaints the active tab. */
    async function refresh() {
        cardsTab.classList.toggle('archive-tab--active', activeTab === 'cards');
        columnsTab.classList.toggle('archive-tab--active', activeTab === 'columns');

        list.innerHTML = '';
        try {
            if (activeTab === 'cards') {
                await renderCards();
            } else {
                await renderColumns();
            }
        } catch (e) {
            console.error('Failed to load board archive:', e);
            list.appendChild(createArchiveEmptyState('Не удалось загрузить архив'));
        }
    }

    async function renderCards() {
        const cards = await api.getArchivedCards(boardId);
        if (cards.length === 0) {
            list.appendChild(createArchiveEmptyState('В архиве нет карточек'));
            return;
        }
        for (const card of cards) {
            list.appendChild(createArchiveRow({
                title: card.title,
                subtitle: card.column_name ? `Из колонки «${card.column_name}»` : '',
                onRestore: async () => {
                    try {
                        await api.restoreCard(card.id);
                        showToast(`Карточка «${card.title}» восстановлена`);
                        onChange();
                        refresh();
                    } catch (e) {
                        showToast('Не удалось восстановить карточку', 'error');
                    }
                },
                onDelete: async () => {
                    const ok = await confirmDialog({
                        title: 'Удалить карточку навсегда?',
                        message: `Карточка «${card.title}» будет удалена без возможности восстановления.`,
                        confirmText: 'Удалить навсегда',
                        danger: true,
                    });
                    if (!ok) return;
                    try {
                        await api.deleteCard(card.id);
                        showToast('Карточка удалена');
                        onChange();
                        refresh();
                    } catch (e) {
                        showToast('Не удалось удалить карточку', 'error');
                    }
                },
            }));
        }
    }

    async function renderColumns() {
        const columns = await api.getArchivedColumns(boardId);
        if (columns.length === 0) {
            list.appendChild(createArchiveEmptyState('В архиве нет колонок'));
            return;
        }
        for (const column of columns) {
            list.appendChild(createArchiveRow({
                title: column.name,
                subtitle: 'Вернётся в конец доски',
                onRestore: async () => {
                    try {
                        await api.restoreColumn(column.id);
                        showToast(`Колонка «${column.name}» восстановлена`);
                        onChange();
                        refresh();
                    } catch (e) {
                        showToast('Не удалось восстановить колонку', 'error');
                    }
                },
                onDelete: async () => {
                    const ok = await confirmDialog({
                        title: 'Удалить колонку навсегда?',
                        message: `Колонка «${column.name}» и все карточки внутри неё будут удалены без возможности восстановления.`,
                        confirmText: 'Удалить навсегда',
                        danger: true,
                    });
                    if (!ok) return;
                    try {
                        await api.deleteColumn(column.id);
                        showToast('Колонка удалена');
                        onChange();
                        refresh();
                    } catch (e) {
                        showToast('Не удалось удалить колонку', 'error');
                    }
                },
            }));
        }
    }

    refresh();
}
