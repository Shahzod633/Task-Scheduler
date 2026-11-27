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
import { renderHabitsTodayWidget } from './habits.js';

/** Сколько горящих задач показывать; остальные — в «Списке». */
const TODAY_LIMIT = 8;

/** «Доброе утро» / «Добрый день» / «Добрый вечер» / «Доброй ночи». */
function greetingWord(hour = new Date().getHours()) {
    if (hour >= 5 && hour < 12) return 'Доброе утро';
    if (hour >= 12 && hour < 18) return 'Добрый день';
    if (hour >= 18 && hour < 23) return 'Добрый вечер';
    return 'Доброй ночи';
}

/** «Пятница, 27 ноября». */
function todayTitle() {
    const text = new Date().toLocaleDateString('ru-RU', { weekday: 'long', day: 'numeric', month: 'long' });
    return text.charAt(0).toUpperCase() + text.slice(1);
}

/**
 * Сводка дня словами — цветные чипы под приветствием («Тёплое приложение»,
 * 27.11). Каждый чип ведёт туда, где с этим разбираются.
 */
function renderSummary(box, items, workspaceId) {
    box.innerHTML = '';
    items.forEach((item, i) => {
        const chip = createElement('button', {
            type: 'button',
            className: `home-summary__chip${item.tone ? ` home-summary__chip--${item.tone}` : ''}`,
            style: { '--i': String(i) },
        });
        chip.appendChild(createElement('span', { className: 'home-summary__dot', 'aria-hidden': 'true' }));
        const text = createElement('span');
        if (item.value !== undefined) {
            text.appendChild(createElement('b', {}, String(item.value)));
            text.appendChild(document.createTextNode(` ${item.label}`));
        } else {
            text.textContent = item.label;
        }
        chip.appendChild(text);
        chip.addEventListener('click', () => {
            window.dispatchEvent(new CustomEvent('navigate', { detail: { view: item.view, workspaceId } }));
        });
        box.appendChild(chip);
    });
}

/**
 * Строки виджета «Сегодня и просрочено»: срок, название, где лежит, кто
 * делает. Щелчок открывает карточку прямо здесь.
 */
function renderTodayList(list, cards, workspaceId, covers = new Map()) {
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
        // Плитка доски в её цвете — тот же оттенок, что у плитки на экране
        // «Доски» и у шапки доски («Тёплое приложение», 27.11).
        const cover = covers.get(card.board_id);
        row.appendChild(createElement('span', {
            className: 'today-row__board',
            style: cover ? { '--cover': cover } : {},
            'aria-hidden': 'true',
        }, (card.board_name || '?').trim().charAt(0).toUpperCase()));
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

    const page = createElement('div', { className: 'page page--home' });

    // Приветствие и сводка словами — вместо заголовка «Главная страница»: с
    // этого экрана начинают день, и первым делом он говорит, что горит.
    const hello = createElement('section', { className: 'home-hello' });
    const helloTitle = createElement('h2', { className: 'home-hello__title page__title' }, `${greetingWord()}!`);
    hello.appendChild(helloTitle);
    hello.appendChild(createElement('p', { className: 'home-hello__date' }, todayTitle()));
    const summary = createElement('div', { className: 'home-summary', 'aria-label': 'Сводка дня' });
    hello.appendChild(summary);
    page.appendChild(hello);

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

    // «Привычки на сегодня» — сразу за горящими задачами: отметить привычку
    // можно здесь же в один щелчок, не уходя на экран «Привычки».
    const habitsWidget = createElement('section', {
        className: 'home-widget',
        'aria-labelledby': 'home-habits-title',
    });
    widgets.appendChild(habitsWidget);
    renderHabitsTodayWidget(habitsWidget);

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
    let overdueCount = 0;
    let soonCount = 0;
    let openCount = 0;
    try {
        const [data, boards] = await Promise.all([
            api.listAllCardsInWorkspace(workspaceId),
            api.getBoards(workspaceId).catch(() => []),
        ]);
        const covers = new Map(boards.map(b => [b.id, b.gradient]));
        const finalColumns = new Set(
            (data.boards || []).flatMap(b => b.columns.filter(c => c.is_final).map(c => c.id))
        );
        const live = (data.cards || []).filter(c => !c.archived && !finalColumns.has(c.column_id));
        const urgent = live
            .filter(c => c.due_date && dueState(c.due_date))
            .sort((a, b) => a.due_date.localeCompare(b.due_date));
        overdueCount = urgent.filter(c => dueState(c.due_date) === 'overdue').length;
        soonCount = urgent.length - overdueCount;
        openCount = live.length;
        renderTodayList(todayList, urgent, workspaceId, covers);
    } catch (e) {
        console.error('Не удалось собрать задачи на сегодня:', e);
        todayList.appendChild(createElement('p', { className: 'page__empty' }, 'Ошибка загрузки'));
    }

    // Имя в приветствии и привычки на сегодня — для сводки.
    try {
        const profile = await api.getUserProfile();
        const name = (profile.display_name || '').trim().split(/\s+/)[0];
        if (name) helloTitle.textContent = `${greetingWord()}, ${name}!`;
    } catch (e) {
        console.error('Не удалось прочитать профиль для приветствия:', e);
    }
    let habitsItem = null;
    try {
        const habits = await api.listHabits(false);
        const due = habits.filter(h => h.due_today);
        const marked = due.filter(h => h.today && (h.today.status === 'completed' || h.today.status === 'fallback')).length;
        if (due.length) {
            habitsItem = marked === due.length
                ? { label: 'все привычки на сегодня отмечены', tone: 'success', view: 'habits' }
                : { value: `${marked} из ${due.length}`, label: 'привычек отмечено', tone: 'success', view: 'habits' };
        }
    } catch (e) {
        console.error('Не удалось прочитать привычки для сводки:', e);
    }
    renderSummary(summary, [
        overdueCount
            ? { value: overdueCount, label: pluralize(overdueCount, ['задача просрочена', 'задачи просрочены', 'задач просрочено']), tone: 'danger', view: 'list' }
            : { label: 'просрочек нет', tone: 'success', view: 'list' },
        soonCount
            ? { value: soonCount, label: 'со сроком сегодня или завтра', tone: 'warning', view: 'planner' }
            : null,
        habitsItem,
        { value: openCount, label: pluralize(openCount, ['открытая задача', 'открытые задачи', 'открытых задач']), view: 'list' },
    ].filter(Boolean), workspaceId);

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
        const openMistakes = cards.filter(c => !c.mistake_resolved_at).length;
        mistakeHint.textContent = `Открытых ошибок: ${openMistakes}`;
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
 * Оттенки — те же `--priority-*`, что у точек в «Списке» (`list.css`): это
 * яркость, а не цвет (высокий — светлый, низкий — едва заметный), поэтому
 * второго словаря здесь нет.
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
