// ============================================
// TaskFlow — Recent Boards Page
// ============================================

import * as api from './api.js';
import { createBoardCard } from './hub.js';
import { createElement, $, showToast } from './utils.js';

export async function renderRecentPage(workspaceId) {
    const content = $('#content');
    content.innerHTML = '';
    content.classList.add('view-enter');

    const page = createElement('div', { className: 'page' });
    page.appendChild(createElement('h2', { className: 'page__title' }, 'Недавние'));
    page.appendChild(createElement('p', { className: 'page__subtitle' }, 'Доски, которые вы недавно открывали, в хронологическом порядке'));

    try {
        const boards = await api.getRecentBoards(workspaceId, 20);
        if (boards.length === 0) {
            page.appendChild(createElement('p', { className: 'page__empty' }, 'Вы ещё не открывали ни одной доски'));
        } else {
            const grid = createElement('div', { className: 'hub__boards-grid' });
            for (const board of boards) {
                grid.appendChild(createBoardCard(board));
            }
            page.appendChild(grid);
        }
    } catch (e) {
        showToast('Ошибка загрузки списка', 'error');
    }

    content.appendChild(page);
    setTimeout(() => content.classList.remove('view-enter'), 420);
}
