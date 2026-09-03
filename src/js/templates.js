// ============================================
// TaskFlow — Board Templates
// (shared template defs/creation logic + the dedicated "Шаблоны" page)
// ============================================

import * as api from './api.js';
import Icons from './icons.js';
import { createElement, $, showToast, getRandomGradient } from './utils.js';

// Костяк колонок теперь одинаков на всех досках и создаётся бэкендом вместе
// с доской, поэтому свои наборы колонок у шаблонов отменены: раньше каждый
// шаблон предлагал собственную структуру, и именно из-за этого одинаковые по
// смыслу этапы назывались на разных досках по-разному.
//
// Шаблон теперь отличается только названием доски. Показываем это честно —
// в галерее у каждого шаблона стоит один и тот же набор колонок, а не тот,
// которого больше не будет.
export const REQUIRED_COLUMNS = ['Новые', 'В работе', 'Тестирование', 'Закрыто'];

export const TEMPLATE_DEFS = {
    'Управление проектами': REQUIRED_COLUMNS,
    'Kanban': REQUIRED_COLUMNS,
    'Отслеживание ошибок': REQUIRED_COLUMNS,
    'Дизайн-процесс': REQUIRED_COLUMNS,
};

/**
 * Creates a new board and navigates to it.
 *
 * Колонки не дописываются: обязательный костяк заводит `create_board`.
 */
export async function createBoardFromTemplate(workspaceId, templateName) {
    try {
        const gradient = getRandomGradient();
        const board = await api.createBoard(workspaceId, templateName, gradient);

        showToast(`Доска "${templateName}" создана`);
        window.dispatchEvent(new CustomEvent('navigate', { detail: { view: 'board', boardId: board.id } }));
        return board;
    } catch (e) {
        showToast('Ошибка создания доски', 'error');
    }
}

/**
 * Full "Шаблоны" page — a gallery of all available templates.
 */
export async function renderTemplatesPage(workspaceId) {
    const content = $('#content');
    content.innerHTML = '';
    content.classList.add('view-enter');

    const page = createElement('div', { className: 'page' });
    page.appendChild(createElement('h2', { className: 'page__title' }, 'Шаблоны'));
    page.appendChild(createElement('p', { className: 'page__subtitle' },
        'Быстрое создание доски с обязательной структурой колонок — она одинакова на всех досках'));

    const grid = createElement('div', { className: 'template-gallery' });
    for (const name of Object.keys(TEMPLATE_DEFS)) {
        const card = createElement('div', { className: 'template-gallery__card' });
        card.appendChild(createElement('span', { className: 'template-gallery__icon', innerHTML: Icons.template }));
        card.appendChild(createElement('span', { className: 'template-gallery__name' }, name));
        card.appendChild(createElement('span', { className: 'template-gallery__cols' }, TEMPLATE_DEFS[name].join(' → ')));
        card.addEventListener('click', () => createBoardFromTemplate(workspaceId, name));
        grid.appendChild(card);
    }
    page.appendChild(grid);

    content.appendChild(page);
    setTimeout(() => content.classList.remove('view-enter'), 420);
}
