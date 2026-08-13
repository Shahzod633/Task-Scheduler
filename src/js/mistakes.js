// ============================================
// TaskFlow — Mistake Tracking Dashboard
// (cross-board analytics for cards marked is_mistake = 1)
// ============================================

import * as api from './api.js';
import { showCardEditModal } from './board.js';
import { renderBarChart, renderLineChart, chartColors } from './charts.js';
import { createElement, $, showToast, formatDate, lastNDays } from './utils.js';

export async function renderMistakesPage(workspaceId) {
    const content = $('#content');
    content.innerHTML = '';
    content.classList.add('view-enter');

    const page = createElement('div', { className: 'page page--mistakes' });
    page.appendChild(createElement('h2', { className: 'page__title' }, 'Отслеживание ошибок'));
    page.appendChild(createElement('p', { className: 'page__subtitle' }, 'Сквозная аналитика по всем карточкам, отмеченным как ошибка, во всех досках пространства'));

    const statsRow = createElement('div', { className: 'mistake-stats' });
    page.appendChild(statsRow);

    const chartsRow = createElement('div', { className: 'mistake-charts' });

    const barPanel = createElement('div', { className: 'mistake-chart-panel' });
    barPanel.appendChild(createElement('h3', { className: 'mistake-chart-panel__title' }, 'Новые ошибки по дням (30 дней)'));
    const barCanvas = createElement('canvas');
    const barWrap = createElement('div', { className: 'mistake-chart-panel__canvas' }, barCanvas);
    barPanel.appendChild(barWrap);
    chartsRow.appendChild(barPanel);

    const linePanel = createElement('div', { className: 'mistake-chart-panel' });
    linePanel.appendChild(createElement('h3', { className: 'mistake-chart-panel__title' }, 'Открыто / исправлено (накопительно)'));
    const lineCanvas = createElement('canvas');
    const lineWrap = createElement('div', { className: 'mistake-chart-panel__canvas' }, lineCanvas);
    linePanel.appendChild(lineWrap);
    chartsRow.appendChild(linePanel);

    page.appendChild(chartsRow);

    const listSection = createElement('div', { className: 'mistake-list-section' });
    listSection.appendChild(createElement('h3', { className: 'mistake-chart-panel__title' }, 'Все отмеченные карточки'));
    const list = createElement('div', { className: 'mistake-list' });
    listSection.appendChild(list);
    page.appendChild(listSection);

    content.appendChild(page);
    setTimeout(() => content.classList.remove('view-enter'), 250);

    try {
        const cards = await api.getMistakeCards(workspaceId);
        renderStats(statsRow, cards);
        renderCharts(barCanvas, lineCanvas, cards);
        renderList(list, cards, workspaceId);
    } catch (e) {
        showToast('Ошибка загрузки данных', 'error');
    }
}

function renderStats(container, cards) {
    container.innerHTML = '';
    const total = cards.length;
    const open = cards.filter(c => !c.mistake_resolved_at).length;
    const resolved = total - open;

    const resolvedDurations = cards
        .filter(c => c.mistake_resolved_at && c.mistake_marked_at)
        .map(c => (new Date(c.mistake_resolved_at) - new Date(c.mistake_marked_at)) / 86400000);
    const avgDays = resolvedDurations.length
        ? resolvedDurations.reduce((a, b) => a + b, 0) / resolvedDurations.length
        : null;

    const tiles = [
        ['Всего ошибок', String(total)],
        ['Открыто', String(open)],
        ['Исправлено', String(resolved)],
        ['Среднее время исправления', avgDays === null ? '—' : `${avgDays.toFixed(1)} дн.`],
    ];
    for (const [label, value] of tiles) {
        const tile = createElement('div', { className: 'mistake-stat-tile' });
        tile.appendChild(createElement('div', { className: 'mistake-stat-tile__value' }, value));
        tile.appendChild(createElement('div', { className: 'mistake-stat-tile__label' }, label));
        container.appendChild(tile);
    }
}

function renderCharts(barCanvas, lineCanvas, cards) {
    const days = lastNDays(30);
    const labels = days.map(d => d.slice(5));

    const newCounts = days.map(day => cards.filter(c => (c.mistake_marked_at || '').slice(0, 10) === day).length);
    renderBarChart(barCanvas, labels, newCounts, { color: chartColors.danger, label: 'Новые ошибки' });

    let openedSoFar = 0;
    let closedSoFar = 0;
    const openedSeries = [];
    const closedSeries = [];
    for (const day of days) {
        openedSoFar += cards.filter(c => (c.mistake_marked_at || '').slice(0, 10) === day).length;
        closedSoFar += cards.filter(c => (c.mistake_resolved_at || '').slice(0, 10) === day).length;
        openedSeries.push(openedSoFar);
        closedSeries.push(closedSoFar);
    }
    renderLineChart(lineCanvas, labels, [
        { label: 'Открыто (всего)', data: openedSeries, color: chartColors.danger },
        { label: 'Исправлено (всего)', data: closedSeries, color: chartColors.success },
    ]);
}

function renderList(container, cards, workspaceId) {
    container.innerHTML = '';
    if (cards.length === 0) {
        container.appendChild(createElement('p', { className: 'page__empty' }, 'Пока нет карточек, отмеченных как ошибка'));
        return;
    }

    for (const card of cards) {
        const row = createElement('div', { className: 'mistake-row' });

        const info = createElement('div', { className: 'mistake-row__info' });
        info.appendChild(createElement('span', { className: 'mistake-row__title' }, card.title));
        const location = [card.board_name, card.column_name].filter(Boolean).join(' → ');
        if (location) info.appendChild(createElement('span', { className: 'mistake-row__location' }, location));
        row.appendChild(info);

        row.appendChild(createElement('span', { className: 'mistake-row__date' }, `Отмечена: ${formatDate(card.mistake_marked_at)}`));
        row.appendChild(createElement('span', { className: 'mistake-row__date' },
            card.mistake_resolved_at ? `Исправлена: ${formatDate(card.mistake_resolved_at)}` : ''));

        const status = createElement('span', {
            className: `mistake-row__status ${card.mistake_resolved_at ? 'mistake-row__status--resolved' : 'mistake-row__status--open'}`
        }, card.mistake_resolved_at ? 'Исправлена' : 'Открыта');
        row.appendChild(status);

        row.addEventListener('click', () => {
            showCardEditModal(card, { onChange: () => renderMistakesPage(workspaceId) });
        });

        container.appendChild(row);
    }
}
