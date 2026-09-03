// ============================================
// TaskFlow — раздел «Требуют внимания»
// (cross-board analytics for cards marked is_mistake = 1)
// ============================================
// Раздел назывался «Отслеживание ошибок». Переименован только на экране:
// поля в базе (`is_mistake`, `mistake_marked_at`) и имя маршрута остались
// прежними — это внутренние технические имена, и переименовывать их ради
// подписи значит платить регрессией без всякой пользы.

import * as api from './api.js';
import Icons from './icons.js';
import { showCardEditModal } from './board.js';
import { confirmDialog } from './dialog.js';
import { renderBarChart, renderLineChart, chartColors } from './charts.js';
import { createElement, $, showToast, formatDate, lastNDays, parseTimestamp, toDateKey } from './utils.js';

/// Столько раз можно попросить ещё одну попытку. То же число — в
/// `commands::RETRY_LIMIT`; бэкенд проверяет его сам, здесь оно только решает,
/// рисовать ли кнопку.
const RETRY_LIMIT = 3;

export async function renderMistakesPage(workspaceId) {
    const content = $('#content');
    content.innerHTML = '';
    content.classList.add('view-enter');

    const page = createElement('div', { className: 'page page--mistakes' });
    page.appendChild(createElement('h2', { className: 'page__title' }, 'Требуют внимания'));
    page.appendChild(createElement('p', { className: 'page__subtitle' }, 'Сквозная аналитика по всем карточкам, требующим внимания, во всех досках пространства'));

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
    setTimeout(() => content.classList.remove('view-enter'), 420);

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
        .map(c => (parseTimestamp(c.mistake_resolved_at) - parseTimestamp(c.mistake_marked_at)) / 86400000);
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

    // Отметки в базе — в UTC, а столбцы графика подписаны местными датами.
    // Резать строку через .slice(0, 10) нельзя: событие, случившееся вечером,
    // попадало бы в следующий день. Поэтому сначала разбор, потом местный ключ.
    const dayOf = (timestamp) => toDateKey(parseTimestamp(timestamp));
    const countOn = (day, field) => cards.filter(c => c[field] && dayOf(c[field]) === day).length;

    const newCounts = days.map(day => countOn(day, 'mistake_marked_at'));
    renderBarChart(barCanvas, labels, newCounts, { color: chartColors.danger, label: 'Новые ошибки' });

    let openedSoFar = 0;
    let closedSoFar = 0;
    const openedSeries = [];
    const closedSeries = [];
    for (const day of days) {
        openedSoFar += countOn(day, 'mistake_marked_at');
        closedSoFar += countOn(day, 'mistake_resolved_at');
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

        // ─── Попытки ───
        //
        // Счётчик рисуется всегда, кнопка — пока попытки есть. Кнопка без
        // счётчика не говорила бы, сколько их осталось, а последняя попытка и
        // первая — решения разного веса.
        const retries = card.retry_count || 0;
        const attempts = createElement('span', {
            className: 'mistake-row__retries',
            'data-tooltip': retries >= RETRY_LIMIT
                ? 'Попытки исчерпаны: при следующей просрочке задача уйдёт в архив'
                : 'Потрачено попыток продления срока'
        }, `${retries} из ${RETRY_LIMIT}`);
        row.appendChild(attempts);

        if (retries < RETRY_LIMIT) {
            const retryBtn = createElement('button', {
                className: 'btn btn--secondary mistake-row__retry',
                innerHTML: `${Icons.rotateCcw} <span>Запросить ещё одну попытку</span>`
            });
            retryBtn.addEventListener('click', async (e) => {
                // Иначе клик дойдёт до строки и поверх подтверждения
                // откроется окно карточки.
                e.stopPropagation();

                const left = RETRY_LIMIT - retries;
                const ok = await confirmDialog({
                    title: 'Продлить срок на неделю?',
                    message: left === 1
                        ? `Это последняя попытка для «${card.title}». Если срок пройдёт снова, задача уйдёт в архив.`
                        : `Срок «${card.title}» сдвинется на 7 дней вперёд, задача вернётся в первую рабочую колонку. Останется попыток: ${left - 1}.`,
                    confirmText: 'Продлить',
                });
                if (!ok) return;

                try {
                    await api.requestCardRetry(card.id);
                    showToast(`Срок продлён, попытка ${retries + 1} из ${RETRY_LIMIT}`);
                    renderMistakesPage(workspaceId);
                } catch (err) {
                    // Текст приходит с бэкенда: он различает исчерпанные
                    // попытки и задачу в финальной колонке.
                    showToast(typeof err === 'string' ? err : 'Не удалось продлить срок', 'error');
                }
            });
            row.appendChild(retryBtn);
        }

        row.addEventListener('click', () => {
            showCardEditModal(card, { onChange: () => renderMistakesPage(workspaceId) });
        });

        container.appendChild(row);
    }
}
