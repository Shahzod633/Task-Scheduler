// ============================================
// TaskFlow — Home Dashboard Page
// ============================================

import * as api from './api.js';
import { createBoardCard } from './hub.js';
import { renderBarChart } from './charts.js';
import { createAvatar } from './members.js';
import { priorityLabel } from './filters.js';
import { createElement, $, showToast, lastNDays, pluralize, formatShortDay, dueState, formatDueDate } from './utils.js';
import Icons from './icons.js';
import { showCardEditModal } from './board.js';

/** Сколько горящих задач показывать; остальные — в «Списке». */
const TODAY_LIMIT = 8;

/**
 * Строки виджета «Сегодня и просрочено»: срок, название, где лежит, кто
 * делает. Щелчок открывает карточку прямо здесь.
 */
function renderTodayList(list, cards, workspaceId) {
    list.innerHTML = '';
    if (!cards.length) {
        list.appendChild(createElement('p', { className: 'page__empty' },
            'Просроченных задач нет, на сегодня и завтра сроков тоже.'));
        return;
    }
    for (const card of cards.slice(0, TODAY_LIMIT)) {
        const state = dueState(card.due_date);
        const row = createElement('button', { className: 'today-row', type: 'button' });
        row.appendChild(createElement('span', {
            className: `due-chip due-chip--${state}`,
            innerHTML: `${Icons.clock}<span>${formatDueDate(card.due_date)}</span>`,
        }));
        const text = createElement('span', { className: 'today-row__text' });
        text.appendChild(createElement('span', { className: 'today-row__title' }, card.title));
        text.appendChild(createElement('span', { className: 'today-row__where' }, `${card.board_name} · ${card.column_name}`));
        row.appendChild(text);
        if (card.assignee) row.appendChild(createAvatar(card.assignee, { size: 'sm' }));
        row.addEventListener('click', () => {
            showCardEditModal(card, { onChange: () => renderHomePage(workspaceId), workspaceId });
        });
        list.appendChild(row);
    }
    if (cards.length > TODAY_LIMIT) {
        const rest = cards.length - TODAY_LIMIT;
        const more = createElement('button', { className: 'btn btn--ghost btn--sm today-list__more', type: 'button' },
            `Ещё ${rest} ${pluralize(rest, ['задача', 'задачи', 'задач'])} — в «Списке»`);
        more.addEventListener('click', () => {
            window.dispatchEvent(new CustomEvent('navigate', { detail: { view: 'list', workspaceId } }));
        });
        list.appendChild(more);
    }
}

export async function renderHomePage(workspaceId) {
    const content = $('#content');
    content.innerHTML = '';
    content.classList.add('view-enter');

    const page = createElement('div', { className: 'page' });
    page.appendChild(createElement('h2', { className: 'page__title' }, 'Главная страница'));

    const widgets = createElement('div', { className: 'home-widgets' });

    // «Сегодня и просрочено» — первым и во всю ширину (12.11): с этого экрана
    // начинают день, и первый вопрос — что горит.
    const todayWidget = createElement('section', {
        className: 'home-widget home-widget--wide',
        'aria-labelledby': 'home-today-title',
    });
    todayWidget.appendChild(createElement('h3', { className: 'home-widget__title', id: 'home-today-title' }, 'Сегодня и просрочено'));
    const todayList = createElement('div', { className: 'today-list' });
    todayWidget.appendChild(todayList);
    widgets.appendChild(todayWidget);

    const recentWidget = createElement('div', { className: 'home-widget' });
    recentWidget.appendChild(createElement('h3', { className: 'home-widget__title' }, 'Недавние доски'));
    widgets.appendChild(recentWidget);

    const workloadWidget = createElement('div', { className: 'home-widget' });
    workloadWidget.appendChild(createElement('h3', { className: 'home-widget__title' }, 'Нагрузка'));
    widgets.appendChild(workloadWidget);

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

    // Сегодня и просрочено. Тот же список, что у «Списка» и палитры: в нём
    // есть доска, колонка и исполнитель каждой задачи, а у досок — какие
    // колонки финальные. Закрытые и архивные задачи не горят.
    try {
        const data = await api.listAllCardsInWorkspace(workspaceId);
        const finalColumns = new Set(
            (data.boards || []).flatMap(b => b.columns.filter(c => c.is_final).map(c => c.id))
        );
        const urgent = (data.cards || [])
            .filter(c => !c.archived && c.due_date && !finalColumns.has(c.column_id) && dueState(c.due_date))
            .sort((a, b) => a.due_date.localeCompare(b.due_date));
        renderTodayList(todayList, urgent, workspaceId);
    } catch (e) {
        console.error('Не удалось собрать задачи на сегодня:', e);
        todayList.appendChild(createElement('p', { className: 'page__empty' }, 'Ошибка загрузки'));
    }

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

    // Нагрузка по исполнителям
    try {
        const rows = await api.getWorkloadSummary(workspaceId);
        workloadWidget.appendChild(createWorkloadList(rows));
    } catch (e) {
        console.error('Не удалось посчитать нагрузку:', e);
        workloadWidget.appendChild(createElement('p', { className: 'page__empty' }, 'Ошибка загрузки'));
    }

    // Mistake-tracking mini chart
    try {
        const cards = await api.getMistakeCards(workspaceId);
        const days = lastNDays(7);
        const counts = days.map(day => cards.filter(c => (c.mistake_marked_at || '').slice(0, 10) === day).length);
        const labels = days.map(formatShortDay);
        const openCount = cards.filter(c => !c.mistake_resolved_at).length;
        mistakeHint.textContent = `Открытых ошибок: ${openCount}`;
        renderBarChart(chartCanvas, labels, counts, { color: 'danger', label: 'Новые ошибки' });
    } catch (e) {
        mistakeHint.textContent = 'Ошибка загрузки графика';
    }
}

/**
 * Виджет «Нагрузка»: кто сколько тянет прямо сейчас.
 *
 * Считается на бэкенде одним запросом (`get_workload_summary`); здесь только
 * отрисовка. Участники приходят уже отсортированными по убыванию нагрузки, а
 * те, у кого открытых задач нет, — в конце списка: «у кого сейчас пусто» такой
 * же ответ, как «у кого завал», и прятать их незачем.
 */
function createWorkloadList(rows) {
    if (!rows.length) {
        return createElement('p', { className: 'page__empty' }, 'В справочнике пока нет участников');
    }

    const list = createElement('div', { className: 'workload' });

    for (const row of rows) {
        const item = createElement('div', { className: 'workload-row' });
        item.appendChild(createAvatar(row.member, { size: 'md' }));

        const body = createElement('div', { className: 'workload-row__body' });

        const head = createElement('div', { className: 'workload-row__head' });
        head.appendChild(createElement('span', { className: 'workload-row__name' }, row.member.name));
        head.appendChild(createElement('span', { className: 'workload-row__count' },
            row.total
                ? `${row.total} ${pluralize(row.total, ['задача', 'задачи', 'задач'])}`
                : 'свободен'));
        body.appendChild(head);
        body.appendChild(createWorkloadBar(row));

        item.appendChild(body);
        list.appendChild(item);
    }

    return list;
}

/**
 * Полоса разбивки по приоритету.
 *
 * Оттенки — те же `--priority-*`, что у точек в «Списке» (`list.css`): с
 * палитры «Сигнал» это яркость, а не цвет (высокий — светлый, низкий — едва
 * заметный), поэтому второго словаря здесь нет.
 * Сначала «Высокий»: полосу читают слева направо, и срочное должно попадаться
 * первым.
 */
function createWorkloadBar(row) {
    const bar = createElement('div', { className: 'workload-bar' });

    if (!row.total) {
        bar.classList.add('workload-bar--empty');
        return bar;
    }

    // `value` — то, что лежит в `cards.priority` (для подписи через
    // `priorityLabel`), `key` — суффикс класса, он же строчный вариант из
    // `priorityModifier`.
    const segments = [
        { key: 'high', value: 'High', count: row.high },
        { key: 'medium', value: 'Medium', count: row.medium },
        { key: 'low', value: 'Low', count: row.low },
    ];

    for (const seg of segments) {
        if (!seg.count) continue;
        // Доля от общего, а не фиксированная ширина: полоса показывает состав
        // нагрузки, а не её объём — объём стоит числом рядом.
        bar.appendChild(createElement('span', {
            className: `workload-bar__seg workload-bar__seg--${seg.key}`,
            style: { flexGrow: String(seg.count) },
            'data-tooltip': `${priorityLabel(seg.value)}: ${seg.count}`,
        }));
    }

    return bar;
}
