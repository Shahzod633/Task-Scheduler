// ============================================
// TaskFlow — Favorites Page
// ============================================

import * as api from './api.js';
import { createBoardCard } from './hub.js';
import { createElement, $, showToast } from './utils.js';

export async function renderFavoritesPage(workspaceId) {
    const content = $('#content');
    content.innerHTML = '';
    content.classList.add('view-enter');

    const page = createElement('div', { className: 'page' });
    page.appendChild(createElement('h2', { className: 'page__title' }, 'Избранное'));
    page.appendChild(createElement('p', { className: 'page__subtitle' }, 'Доски, отмеченные звёздочкой'));

    try {
        const boards = await api.getBoards(workspaceId);
        const starred = boards.filter(b => b.is_starred);

        if (starred.length === 0) {
            page.appendChild(createElement('p', { className: 'page__empty' }, 'Нет избранных досок — нажмите на звёздочку на карточке доски, чтобы добавить её сюда'));
        } else {
            const grid = createElement('div', { className: 'hub__boards-grid', dataset: { boardId: '' } });
            for (const board of starred) {
                grid.appendChild(createBoardCard(board));
            }
            page.appendChild(grid);

            // Live-prune a card the moment it's unstarred, even while this page is open.
            const onStarToggled = (e) => {
                if (!document.body.contains(grid)) {
                    window.removeEventListener('board-star-toggled', onStarToggled);
                    return;
                }
                if (!e.detail.starred) {
                    const card = grid.querySelector(`[data-board-id="${e.detail.boardId}"]`);
                    if (card) card.remove();
                }
            };
            window.addEventListener('board-star-toggled', onStarToggled);
        }
    } catch (e) {
        showToast('Ошибка загрузки списка', 'error');
    }

    content.appendChild(page);
    setTimeout(() => content.classList.remove('view-enter'), 420);
}
