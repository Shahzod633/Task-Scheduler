// ============================================
// TaskFlow — Inbox Page
// (quick tasks not yet assigned to a board/column — backed by a hidden
// per-workspace system board, see get_inbox_column on the Rust side)
// ============================================

import * as api from './api.js';
import { createElement, $, showToast } from './utils.js';

export async function renderInboxPage(workspaceId) {
    const content = $('#content');
    content.innerHTML = '';
    content.classList.add('view-enter');

    const page = createElement('div', { className: 'page page--inbox' });
    page.appendChild(createElement('h2', { className: 'page__title' }, 'Inbox'));
    page.appendChild(createElement('p', { className: 'page__subtitle' }, 'Быстрые задачи без доски — добавляйте их сюда и распределяйте по доскам, когда будет время'));

    const addForm = createElement('div', { className: 'inbox-add-form' });
    const input = createElement('input', { className: 'form-input', placeholder: 'Новая задача...' });
    addForm.appendChild(input);
    const addBtn = createElement('button', { className: 'btn btn--primary' }, 'Добавить');
    addForm.appendChild(addBtn);
    page.appendChild(addForm);

    const list = createElement('div', { className: 'inbox-list' });
    page.appendChild(list);

    content.appendChild(page);
    setTimeout(() => content.classList.remove('view-enter'), 420);

    let inboxColumnId = null;
    let boardOptions = [];

    async function loadBoardOptions() {
        const boards = await api.getBoards(workspaceId);
        const options = [];
        for (const board of boards) {
            const columns = await api.getColumns(board.id);
            if (columns.length > 0) options.push({ board, columns });
        }
        return options;
    }

    function createInboxRow(card) {
        const row = createElement('div', { className: 'inbox-row' });
        row.appendChild(createElement('span', { className: 'inbox-row__title' }, card.title));

        const select = createElement('select', { className: 'form-input inbox-row__select' });
        select.appendChild(createElement('option', { value: '' }, 'Назначить в доску...'));
        for (const { board, columns } of boardOptions) {
            const group = createElement('optgroup', { label: board.name });
            for (const col of columns) {
                group.appendChild(createElement('option', { value: String(col.id) }, col.name));
            }
            select.appendChild(group);
        }
        select.addEventListener('change', async () => {
            const targetColumnId = parseInt(select.value, 10);
            if (!targetColumnId) return;
            try {
                const targetCards = await api.getCards(targetColumnId);
                await api.updateCardPosition(card.id, targetColumnId, targetCards.length);
                showToast('Задача назначена в доску');
                row.remove();
                if (!list.children.length) {
                    list.appendChild(createElement('p', { className: 'page__empty' }, 'Inbox пуст'));
                }
            } catch (e) {
                showToast('Ошибка назначения', 'error');
            }
        });
        row.appendChild(select);

        return row;
    }

    async function loadList() {
        list.innerHTML = '';
        try {
            const column = await api.getInboxColumn(workspaceId);
            inboxColumnId = column.id;
            const cards = await api.getCards(inboxColumnId);

            if (cards.length === 0) {
                list.appendChild(createElement('p', { className: 'page__empty' }, 'Inbox пуст'));
                return;
            }
            for (const card of cards) {
                list.appendChild(createInboxRow(card));
            }
        } catch (e) {
            showToast('Ошибка загрузки Inbox', 'error');
        }
    }

    async function submit() {
        const title = input.value.trim();
        if (!title || !inboxColumnId) return;
        try {
            await api.createCard(inboxColumnId, title);
            input.value = '';
            await loadList();
            showToast('Задача добавлена в Inbox');
        } catch (e) {
            showToast('Ошибка создания задачи', 'error');
        }
    }

    addBtn.addEventListener('click', submit);
    input.addEventListener('keydown', (e) => { if (e.key === 'Enter') submit(); });

    boardOptions = await loadBoardOptions();
    await loadList();
    input.focus();
}
