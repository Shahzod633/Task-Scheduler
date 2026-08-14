// ============================================
// TaskFlow — Home Dashboard Page
// ============================================

import * as api from './api.js';
import { createBoardCard } from './hub.js';
import { renderBarChart, chartColors } from './charts.js';
import { createElement, $, showToast, lastNDays } from './utils.js';

export async function renderHomePage(workspaceId) {
    const content = $('#content');
    content.innerHTML = '';
    content.classList.add('view-enter');

    const page = createElement('div', { className: 'page' });
    page.appendChild(createElement('h2', { className: 'page__title' }, 'Главная страница'));

    const widgets = createElement('div', { className: 'home-widgets' });

    const recentWidget = createElement('div', { className: 'home-widget' });
    recentWidget.appendChild(createElement('h3', { className: 'home-widget__title' }, 'Недавние доски'));
    widgets.appendChild(recentWidget);

    const mistakeWidget = createElement('div', { className: 'home-widget' });
    mistakeWidget.appendChild(createElement('h3', { className: 'home-widget__title' }, 'Мои задачи-ошибки за неделю'));
    const chartWrap = createElement('div', { className: 'home-widget__chart' });
    const chartCanvas = createElement('canvas');
    chartWrap.appendChild(chartCanvas);
    mistakeWidget.appendChild(chartWrap);
    const mistakeHint = createElement('p', { className: 'home-widget__hint' });
    mistakeWidget.appendChild(mistakeHint);
    widgets.appendChild(mistakeWidget);

    page.appendChild(widgets);
    content.appendChild(page);
    setTimeout(() => content.classList.remove('view-enter'), 420);

    // Recent boards
    try {
        const boards = await api.getRecentBoards(workspaceId, 5);
        if (boards.length === 0) {
            recentWidget.appendChild(createElement('p', { className: 'page__empty' }, 'Пока нет открытых досок'));
        } else {
            const grid = createElement('div', { className: 'home-widget__boards' });
            boards.forEach((board, i) => grid.appendChild(createBoardCard(board, i)));
            recentWidget.appendChild(grid);
        }
    } catch (e) {
        recentWidget.appendChild(createElement('p', { className: 'page__empty' }, 'Ошибка загрузки'));
    }

    // Mistake-tracking mini chart
    try {
        const cards = await api.getMistakeCards(workspaceId);
        const days = lastNDays(7);
        const counts = days.map(day => cards.filter(c => (c.mistake_marked_at || '').slice(0, 10) === day).length);
        const labels = days.map(d => d.slice(5));
        const openCount = cards.filter(c => !c.mistake_resolved_at).length;
        mistakeHint.textContent = `Открытых ошибок: ${openCount}`;
        renderBarChart(chartCanvas, labels, counts, { color: chartColors.danger, label: 'Новые ошибки' });
    } catch (e) {
        mistakeHint.textContent = 'Ошибка загрузки графика';
    }
}
