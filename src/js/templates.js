// ============================================
// TaskFlow — Board Templates
// (shared template defs/creation logic + the dedicated "Шаблоны" page)
// ============================================

import * as api from './api.js';
import Icons from './icons.js';
import { createElement, $, showToast, getRandomGradient } from './utils.js';

export const TEMPLATE_DEFS = {
    'Управление проектами': ['Бэклог', 'В работе', 'На проверке', 'Готово'],
    'Kanban': ['Сделать', 'В процессе', 'Готово'],
    'Отслеживание ошибок': ['Новые', 'В работе', 'Тестирование', 'Закрыто'],
    'Дизайн-процесс': ['Идеи', 'Дизайн', 'Разработка', 'Завершено'],
};

/**
 * Creates a new board pre-populated with a template's default columns
 * and navigates to it.
 */
export async function createBoardFromTemplate(workspaceId, templateName) {
    try {
        const gradient = getRandomGradient();
        const board = await api.createBoard(workspaceId, templateName, gradient);

        const columns = TEMPLATE_DEFS[templateName] || ['Сделать', 'В процессе', 'Готово'];
        for (const colName of columns) {
            await api.createColumn(board.id, colName);
        }

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
    page.appendChild(createElement('p', { className: 'page__subtitle' }, 'Выберите шаблон, чтобы создать доску с готовым набором колонок'));

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
    setTimeout(() => content.classList.remove('view-enter'), 250);
}
