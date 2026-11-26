// ============================================
// TaskFlow — Привычки
// ============================================
// Экран «Привычки» (список, форма, тепловая карта, ИИ-аналитика) и виджет
// «Привычки на сегодня» для главной страницы.
//
// Привычки общие для всех пространств — это свойство человека, а не проекта,
// поэтому ни один вызов здесь не берёт workspaceId. Стрик считает бэкенд
// (`commands/habits.rs`): после каждой отметки он возвращает привычку уже со
// свежим стриком, и интерфейс ничего не пересчитывает сам.

import * as api from './api.js';
import Icons from './icons.js';
import { confirmDialog } from './dialog.js';
import { openPopover, closePopovers } from './popover.js';
import { renderLineChart } from './charts.js';
import {
    createElement, $, showToast, pluralize, todayKey, formatShortDay, parseTimestamp,
} from './utils.js';

export const TIMES_OF_DAY = [
    { value: 'morning', label: 'Утро' },
    { value: 'afternoon', label: 'День' },
    { value: 'evening', label: 'Вечер' },
    { value: 'anytime', label: 'В любое время' },
];

const WEEKDAYS = [
    ['mon', 'Пн'], ['tue', 'Вт'], ['wed', 'Ср'], ['thu', 'Чт'], ['fri', 'Пт'], ['sat', 'Сб'], ['sun', 'Вс'],
];

const FREQUENCIES = [
    { value: 'daily', label: 'Каждый день' },
    { value: 'weekly_days', label: 'По дням недели' },
    { value: 'x_per_week', label: 'N раз в неделю' },
];

const EFFORT_HINTS = {
    1: 'почти без усилий',
    2: 'легко',
    3: 'средне',
    4: 'тяжело',
    5: 'на пределе',
};

const CONFIDENCE_LABELS = { high: 'Высокая', medium: 'Средняя', low: 'Низкая' };

/** Тепловая карта — целыми неделями, сколько влезает по ширине: от квартала до года. */
const HEATMAP_MIN_WEEKS = 13;
const HEATMAP_MAX_WEEKS = 53;
/** Ширина недели (клетка 13 px + зазор 3 px) и колонки подписей дней — как в habits.css. */
const HEATMAP_WEEK_PX = 16;
const HEATMAP_LABELS_PX = 27;
/** Внутренний отступ панели (`--space-lg`) — ширина карты до первой отрисовки. */
const HEATMAP_PANEL_PADDING_PX = 16;
/** Сколько идёт волна по всей карте, мс. */
const HEATMAP_WAVE_MS = 640;

function timeLabel(value) {
    return TIMES_OF_DAY.find(t => t.value === value)?.label ?? value;
}

/** «Каждый день», «Пн, Ср, Пт», «3 раза в неделю». */
export function describeFrequency(habit) {
    if (habit.frequency_type === 'weekly_days') {
        return WEEKDAYS.filter(([key]) => habit.weekdays.includes(key)).map(([, label]) => label).join(', ');
    }
    if (habit.frequency_type === 'x_per_week') {
        const n = habit.target_per_week || 1;
        return `${n} ${pluralize(n, ['раз', 'раза', 'раз'])} в неделю`;
    }
    return 'Каждый день';
}

function streakWord(habit, n) {
    return habit.streak_unit === 'week'
        ? pluralize(n, ['неделя', 'недели', 'недель'])
        : pluralize(n, ['день', 'дня', 'дней']);
}

// ─── Движение: «сигнал времени» ───
// Отметка — стрелка обходит циферблат кружка и оставляет галочку, по строке
// к огоньку пробегает свет, цифра стрика перекатывается, как на счётчике, а
// огонёк вспыхивает (при срыве — гаснет). Разыгрывается один раз: сразу после
// отметки строка перерисовывается, поэтому переход ставится в очередь здесь и
// забирается той строкой, что рисуется следующей. Сами кадры — в habits.css.

/** id привычки → что с ней только что случилось: { kind, from, to }. */
const pendingMotion = new Map();
/** Отметка была — на экране «Привычки» сегодняшняя клетка карты мигнёт. */
let pendingHeatPing = false;

const isDoneLog = (log) => Boolean(log) && (log.status === 'completed' || log.status === 'fallback');

function queueMotion(before, after) {
    const was = isDoneLog(before.today);
    const now = isDoneLog(after.today);
    let kind = null;
    if (now && !was) kind = 'done';
    else if (!now && was) kind = 'undo';
    else if (after.today && !now && !before.today) kind = 'missed';
    if (!kind && before.current_streak === after.current_streak) return;
    pendingMotion.set(after.id, { kind, from: before.current_streak, to: after.current_streak });
    pendingHeatPing = true;
}

function takeMotion(id) {
    const motion = pendingMotion.get(id) || null;
    pendingMotion.delete(id);
    return motion;
}

/** Галочка с `pathLength="1"`: её прорисовку задаёт `stroke-dashoffset` в CSS. */
const CHECK_MARK = `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4"
    stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path pathLength="1" d="M6 12.5l4 4L18 8.5"/></svg>`;

/**
 * Огонёк со стриком; погашен, пока стрик нулевой. Если стрик только что
 * изменился, цифра перекатывается со старой на новую.
 */
function createStreakBadge(habit, motion = null) {
    const n = habit.current_streak;
    const badge = createElement('span', {
        className: `habit-streak${n ? '' : ' habit-streak--cold'}`,
        'data-tooltip': n
            ? `Стрик: ${n} ${streakWord(habit, n)} подряд · рекорд ${habit.longest_streak}`
            : `Стрика нет · рекорд ${habit.longest_streak}`,
        'aria-label': `Стрик ${n} ${streakWord(habit, n)}`,
    });
    badge.appendChild(createElement('span', { className: 'habit-streak__flame', innerHTML: Icons.flame }));
    const num = createElement('span', { className: 'habit-streak__num' });
    if (motion && motion.from !== motion.to) {
        badge.classList.add(motion.to > motion.from ? 'habit-streak--up' : 'habit-streak--down');
        num.append(
            createElement('span', { className: 'habit-streak__digit habit-streak__digit--old', 'aria-hidden': 'true' }, String(motion.from)),
            createElement('span', { className: 'habit-streak__digit habit-streak__digit--new' }, String(n)),
        );
    } else {
        num.textContent = String(n);
    }
    badge.appendChild(num);
    return badge;
}

/** Нагрузка точками: ●●●○○. */
function createEffortDots(level) {
    const dots = createElement('span', {
        className: 'habit-effort',
        'data-tooltip': `Нагрузка ${level} из 5 — ${EFFORT_HINTS[level] || ''}`,
        'aria-label': `Нагрузка ${level} из 5`,
    });
    for (let i = 1; i <= 5; i++) {
        dots.appendChild(createElement('span', { className: `habit-effort__dot${i <= level ? ' habit-effort__dot--on' : ''}` }));
    }
    return dots;
}

// ─── Отметка за сегодня (общая для виджета и списка) ───

async function mark(habit, status, reason, onChange) {
    try {
        const updated = await api.logHabit(habit.id, todayKey(), status, reason);
        queueMotion(habit, updated);
        onChange(updated);
    } catch (e) {
        showToast(String(e), 'error');
    }
}

async function unmark(habit, onChange) {
    try {
        const updated = await api.clearHabitLog(habit.id, todayKey());
        queueMotion(habit, updated);
        onChange(updated);
    } catch (e) {
        showToast(String(e), 'error');
    }
}

/**
 * Окошко пропуска: причина и два исхода. «Пропустить» замораживает стрик
 * (болезнь, поездка), «Сорвалось» — сбрасывает. Причина необязательна, но
 * именно она потом нужна ИИ-анализу.
 */
function openSkipPopover(anchor, habit, onChange) {
    const box = createElement('div', { className: 'context-menu habit-skip' });
    box.appendChild(createElement('div', { className: 'habit-skip__title' }, `Не получилось: ${habit.title}`));
    const input = createElement('input', {
        className: 'form-input',
        placeholder: 'Причина (необязательно)',
        maxlength: '300',
        'aria-label': 'Причина пропуска',
    });
    box.appendChild(input);

    const actions = createElement('div', { className: 'habit-skip__actions' });
    const skipBtn = createElement('button', {
        className: 'btn btn--secondary btn--sm',
        type: 'button',
        'data-tooltip': 'Стрик заморожен: не растёт и не сгорает',
    }, 'Пропустить');
    const failBtn = createElement('button', {
        className: 'btn btn--ghost btn--sm',
        type: 'button',
        'data-tooltip': 'Стрик сбросится в 0',
    }, 'Сорвалось');
    const submit = (status) => {
        const reason = input.value.trim() || null;
        closePopovers();
        mark(habit, status, reason, onChange);
    };
    skipBtn.addEventListener('click', () => submit('skipped'));
    failBtn.addEventListener('click', () => submit('failed'));
    input.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') submit('skipped');
    });
    actions.append(skipBtn, failBtn);
    box.appendChild(actions);

    openPopover(box, anchor, { placement: 'bottom', align: 'end', gap: 6, width: 280 });
    input.focus();
}

const STATUS_TEXT = {
    completed: 'Выполнено',
    fallback: 'Лайт-версия',
    skipped: 'Пропущено',
    failed: 'Сорвалось',
};

/**
 * Строка «сегодня»: кружок-галочка, название, что сделано, стрик и кнопки.
 * `onChange(habit)` получает привычку после отметки.
 */
function createTodayRow(habit, onChange, { compact = false } = {}) {
    const log = habit.today;
    const done = isDoneLog(log);
    const motion = takeMotion(habit.id);
    const row = createElement('div', {
        className: [
            'habit-today',
            done ? 'habit-today--done' : '',
            log?.status === 'fallback' ? 'habit-today--light' : '',
            log && !done ? 'habit-today--missed' : '',
            motion?.kind ? `habit-today--just-${motion.kind}` : '',
        ].filter(Boolean).join(' '),
    });

    const check = createElement('button', {
        className: 'habit-check',
        type: 'button',
        'aria-pressed': String(Boolean(done)),
        'aria-label': done ? `Снять отметку «${habit.title}»` : `Отметить «${habit.title}» выполненной`,
        'data-tooltip': done ? 'Снять отметку' : 'Выполнено',
        innerHTML: CHECK_MARK,
    });
    check.addEventListener('click', () => (done ? unmark(habit, onChange) : mark(habit, 'completed', null, onChange)));
    row.appendChild(check);

    const text = createElement('div', { className: 'habit-today__text' });
    const title = log?.status === 'fallback' && habit.fallback_title ? habit.fallback_title : habit.title;
    text.appendChild(createElement('span', { className: 'habit-today__title' }, title));

    const meta = [];
    if (log) {
        meta.push(log.status === 'fallback' ? `Лайт-версия «${habit.title}»` : STATUS_TEXT[log.status]);
        if (log.skip_reason) meta.push(log.skip_reason);
    } else {
        if (!compact || habit.time_of_day !== 'anytime') meta.push(timeLabel(habit.time_of_day));
        if (habit.frequency_type === 'x_per_week') meta.push(`${habit.week_done} из ${habit.target_per_week} на неделе`);
    }
    if (meta.length) text.appendChild(createElement('span', { className: 'habit-today__meta' }, meta.join(' · ')));
    row.appendChild(text);

    row.appendChild(createStreakBadge(habit, motion));

    const actions = createElement('div', { className: 'habit-today__actions' });
    if (!log) {
        if (habit.fallback_title) {
            const light = createElement('button', {
                className: 'btn btn--ghost btn--sm',
                type: 'button',
                'data-tooltip': `Лайт-версия: ${habit.fallback_title}`,
            }, 'Лайт');
            light.addEventListener('click', () => mark(habit, 'fallback', null, onChange));
            actions.appendChild(light);
        }
        const skip = createElement('button', {
            className: 'icon-btn',
            type: 'button',
            'data-tooltip': 'Пропуск или срыв',
            'aria-label': 'Пропуск или срыв',
            innerHTML: Icons.skip,
        });
        skip.addEventListener('click', () => openSkipPopover(skip, habit, onChange));
        actions.appendChild(skip);
    } else if (!done) {
        const undo = createElement('button', { className: 'btn btn--ghost btn--sm', type: 'button' }, 'Отменить');
        undo.addEventListener('click', () => unmark(habit, onChange));
        actions.appendChild(undo);
    }
    row.appendChild(actions);
    return row;
}

// ─── Виджет «Привычки на сегодня» (главная) ───

function goToHabits() {
    window.dispatchEvent(new CustomEvent('navigate', { detail: { view: 'habits' } }));
}

/**
 * Наполняет виджет главной: привычки, которые нужно сделать сегодня, и уже
 * отмеченные за сегодня. Заголовок с прогрессом обновляется вместе со
 * списком.
 */
export async function renderHabitsTodayWidget(widget) {
    widget.innerHTML = '';
    const head = createElement('div', { className: 'home-widget__head' });
    head.appendChild(createElement('h3', { className: 'home-widget__title', id: 'home-habits-title' }, 'Привычки на сегодня'));
    const progress = createElement('span', { className: 'home-widget__counter' });
    head.appendChild(progress);
    const all = createElement('button', { className: 'btn btn--ghost btn--sm', type: 'button' }, 'Все привычки');
    all.addEventListener('click', goToHabits);
    head.appendChild(all);
    widget.appendChild(head);

    // Полоска дня: отрезок на каждую привычку, в порядке строк. Отмеченная
    // заливает свой отрезок, а последняя пускает по всей полоске свет.
    const bar = createElement('div', {
        className: 'habit-progress',
        role: 'progressbar',
        'aria-valuemin': '0',
        'aria-label': 'Выполнено за сегодня',
    });
    widget.appendChild(bar);

    const list = createElement('div', { className: 'habit-today-list' });
    widget.appendChild(list);

    let habits;
    try {
        habits = await api.listHabits(false);
    } catch (e) {
        console.error('Не удалось загрузить привычки:', e);
        list.appendChild(createElement('p', { className: 'page__empty' }, 'Ошибка загрузки'));
        return;
    }

    const draw = () => {
        list.innerHTML = '';
        const today = habits.filter(h => h.due_today || h.today);
        const done = today.filter(h => isDoneLog(h.today)).length;
        const justDone = today.find(h => pendingMotion.get(h.id)?.kind === 'done');
        const complete = Boolean(justDone) && done === today.length;
        progress.textContent = today.length ? (complete ? 'Всё на сегодня' : `${done} из ${today.length}`) : '';
        progress.classList.toggle('home-widget__counter--complete', complete);

        bar.innerHTML = '';
        bar.hidden = !today.length;
        bar.classList.toggle('habit-progress--complete', complete);
        bar.setAttribute('aria-valuemax', String(today.length));
        bar.setAttribute('aria-valuenow', String(done));
        for (const h of today) {
            bar.appendChild(createElement('span', {
                className: [
                    'habit-progress__seg',
                    isDoneLog(h.today) ? 'habit-progress__seg--on' : '',
                    h === justDone ? 'habit-progress__seg--fill' : '',
                ].filter(Boolean).join(' '),
            }));
        }

        if (!habits.length) {
            const empty = createElement('div', { className: 'page__empty habit-empty' });
            empty.appendChild(createElement('p', {}, 'Привычек пока нет.'));
            const add = createElement('button', { className: 'btn btn--secondary btn--sm', type: 'button' }, 'Завести привычку');
            add.addEventListener('click', goToHabits);
            empty.appendChild(add);
            list.appendChild(empty);
            return;
        }
        if (!today.length) {
            list.appendChild(createElement('p', { className: 'page__empty' }, 'На сегодня по расписанию ничего нет.'));
            return;
        }
        for (const habit of today) {
            list.appendChild(createTodayRow(habit, (updated) => {
                habits = habits.map(h => (h.id === updated.id ? updated : h));
                draw();
            }, { compact: true }));
        }
    };
    draw();
}

// ─── Экран «Привычки» ───

let filter = 'all';
let showArchived = false;

export async function renderHabitsPage() {
    const content = $('#content');
    content.innerHTML = '';
    content.classList.add('view-enter');

    const page = createElement('div', { className: 'page page--habits' });
    page.appendChild(createElement('h2', { className: 'page__title' }, 'Привычки'));
    page.appendChild(createElement('p', { className: 'page__subtitle' },
        'Общие для всех пространств. Отметка за день — здесь или в виджете на главной; ' +
        'у каждой привычки может быть лайт-версия на перегруженные дни.'));

    const listSection = createElement('section', { className: 'habits-section', 'aria-labelledby': 'habits-list-title' });
    const toolbar = createElement('div', { className: 'habits-toolbar' });
    listSection.appendChild(createElement('h3', { className: 'habits-sr-only', id: 'habits-list-title' }, 'Список привычек'));
    listSection.appendChild(toolbar);
    const list = createElement('div', { className: 'habit-list' });
    listSection.appendChild(list);
    page.appendChild(listSection);

    const heatSection = createElement('section', { className: 'habits-section habits-panel', 'aria-labelledby': 'habits-heat-title' });
    page.appendChild(heatSection);

    const aiSection = createElement('section', { className: 'habits-section habits-panel', 'aria-labelledby': 'habits-ai-title' });
    page.appendChild(aiSection);

    content.appendChild(page);
    setTimeout(() => content.classList.remove('view-enter'), 420);

    let habits = [];
    let firstPaint = true;
    const state = {
        reload: async () => {
            try {
                habits = await api.listHabits(true);
            } catch (e) {
                console.error('Не удалось загрузить привычки:', e);
                list.innerHTML = '';
                list.appendChild(createElement('p', { className: 'page__empty' }, 'Ошибка загрузки'));
                return;
            }
            renderToolbar();
            renderList();
            renderHeatmapSection(heatSection, habits.filter(h => !h.is_archived), { wave: firstPaint });
            firstPaint = false;
        },
    };

    const renderToolbar = () => {
        toolbar.innerHTML = '';
        const chips = createElement('div', { className: 'theme-toggle', role: 'group', 'aria-label': 'Время суток' });
        for (const option of [{ value: 'all', label: 'Все' }, ...TIMES_OF_DAY]) {
            const count = option.value === 'all'
                ? habits.filter(h => !h.is_archived).length
                : habits.filter(h => !h.is_archived && h.time_of_day === option.value).length;
            const btn = createElement('button', {
                className: `theme-toggle__option${filter === option.value ? ' theme-toggle__option--active' : ''}`,
                type: 'button',
                'aria-pressed': String(filter === option.value),
            }, count ? `${option.label} · ${count}` : option.label);
            btn.addEventListener('click', () => {
                filter = option.value;
                renderToolbar();
                renderList();
            });
            chips.appendChild(btn);
        }
        toolbar.appendChild(chips);

        const archivedCount = habits.filter(h => h.is_archived).length;
        if (archivedCount) {
            const label = createElement('label', { className: 'habits-toolbar__archived' });
            const box = createElement('input', { type: 'checkbox' });
            box.checked = showArchived;
            box.addEventListener('change', () => {
                showArchived = box.checked;
                renderList();
            });
            label.append(box, document.createTextNode(` Архив (${archivedCount})`));
            toolbar.appendChild(label);
        }

        const add = createElement('button', {
            className: 'btn btn--primary habits-toolbar__add',
            type: 'button',
            innerHTML: `${Icons.plus}<span>Новая привычка</span>`,
        });
        add.addEventListener('click', () => openHabitForm(null, state.reload));
        toolbar.appendChild(add);
    };

    const renderList = () => {
        list.innerHTML = '';
        const visible = habits.filter(h =>
            (showArchived || !h.is_archived) && (filter === 'all' || h.time_of_day === filter));
        if (!habits.length) {
            const empty = createElement('div', { className: 'page__empty habit-empty' });
            empty.appendChild(createElement('p', {}, 'Привычек пока нет. Начните с одной — маленькой и на каждый день.'));
            const add = createElement('button', { className: 'btn btn--primary btn--sm', type: 'button' }, 'Новая привычка');
            add.addEventListener('click', () => openHabitForm(null, state.reload));
            empty.appendChild(add);
            list.appendChild(empty);
            return;
        }
        if (!visible.length) {
            list.appendChild(createElement('p', { className: 'page__empty' }, 'В этом времени суток привычек нет.'));
            return;
        }
        for (const habit of visible) list.appendChild(createHabitRow(habit, state.reload));
    };

    await state.reload();
    renderAiSection(aiSection);
}

/** Строка списка: сегодняшняя отметка, описание расписания и меню. */
function createHabitRow(habit, reload) {
    const row = createElement('div', { className: `habit-row${habit.is_archived ? ' habit-row--archived' : ''}` });

    if (!habit.is_archived && (habit.due_today || habit.today)) {
        row.appendChild(createTodayRow(habit, () => reload()));
    } else {
        const line = createElement('div', { className: 'habit-today habit-today--off' });
        line.appendChild(createElement('span', { className: 'habit-check habit-check--off', 'aria-hidden': 'true' }));
        const text = createElement('div', { className: 'habit-today__text' });
        text.appendChild(createElement('span', { className: 'habit-today__title' }, habit.title));
        text.appendChild(createElement('span', { className: 'habit-today__meta' },
            habit.is_archived ? 'В архиве' : 'Сегодня не по расписанию'));
        line.appendChild(text);
        line.appendChild(createStreakBadge(habit));
        row.appendChild(line);
    }

    const details = createElement('div', { className: 'habit-row__details' });
    details.appendChild(createElement('span', {}, describeFrequency(habit)));
    details.appendChild(createElement('span', {}, timeLabel(habit.time_of_day)));
    details.appendChild(createEffortDots(habit.effort_level));
    if (habit.fallback_title) {
        details.appendChild(createElement('span', { className: 'habit-row__fallback' }, `Лайт: ${habit.fallback_title}`));
    }
    details.appendChild(createElement('span', { className: 'habit-row__record' },
        `Рекорд: ${habit.longest_streak} ${streakWord(habit, habit.longest_streak)}`));
    if (habit.description) {
        details.appendChild(createElement('span', { className: 'habit-row__description' }, habit.description));
    }
    row.appendChild(details);

    const more = createElement('button', {
        className: 'icon-btn habit-row__more',
        type: 'button',
        'aria-label': `Действия с «${habit.title}»`,
        'data-tooltip': 'Действия',
        innerHTML: Icons.moreHorizontal,
    });
    more.addEventListener('click', () => openRowMenu(more, habit, reload));
    row.appendChild(more);
    return row;
}

function openRowMenu(anchor, habit, reload) {
    const menu = createElement('div', { className: 'context-menu' });
    const item = (label, icon, onClick, danger = false) => {
        const el = createElement('button', {
            className: `context-menu__item${danger ? ' context-menu__item--danger' : ''}`,
            type: 'button',
            innerHTML: `${icon}<span>${label}</span>`,
        });
        el.addEventListener('click', () => {
            closePopovers();
            onClick();
        });
        menu.appendChild(el);
    };

    if (!habit.is_archived) item('Изменить', Icons.edit, () => openHabitForm(habit, reload));
    item(habit.is_archived ? 'Вернуть из архива' : 'В архив', habit.is_archived ? Icons.rotateCcw : Icons.archive, async () => {
        try {
            await api.setHabitArchived(habit.id, !habit.is_archived);
            showToast(habit.is_archived ? 'Привычка возвращена' : 'Привычка в архиве — стрик на паузе');
            reload();
        } catch (e) {
            showToast(String(e), 'error');
        }
    });
    item('Удалить', Icons.trash, async () => {
        const ok = await confirmDialog({
            title: 'Удалить привычку?',
            message: `«${habit.title}» удалится вместе со всеми отметками и стриком. Это не отменить — ` +
                     'если привычка просто на паузе, лучше убрать её в архив.',
            confirmText: 'Удалить',
            danger: true,
        });
        if (!ok) return;
        try {
            await api.deleteHabit(habit.id);
            showToast('Привычка удалена');
            reload();
        } catch (e) {
            showToast(String(e), 'error');
        }
    }, true);

    openPopover(menu, anchor, { placement: 'bottom', align: 'end', gap: 4 });
}

// ─── Форма привычки ───

/** Переключатель из `.theme-toggle` — тот же рисунок, что у темы и вида доски. */
function createChoice(options, current, onPick, { multiple = false, label } = {}) {
    const group = createElement('div', { className: 'theme-toggle habit-choice', role: 'group', 'aria-label': label });
    const selected = new Set(multiple ? current : [current]);
    const paint = () => {
        for (const btn of group.children) {
            const on = selected.has(btn.dataset.value);
            btn.classList.toggle('theme-toggle__option--active', on);
            btn.setAttribute('aria-pressed', String(on));
        }
    };
    for (const { value, label: text } of options) {
        const btn = createElement('button', { className: 'theme-toggle__option', type: 'button', dataset: { value } }, text);
        btn.addEventListener('click', () => {
            if (multiple) {
                if (selected.has(value)) selected.delete(value);
                else selected.add(value);
                onPick([...selected]);
            } else {
                selected.clear();
                selected.add(value);
                onPick(value);
            }
            paint();
        });
        group.appendChild(btn);
    }
    paint();
    return group;
}

function formGroup(labelText, control, hint, forId) {
    const group = createElement('div', { className: 'form-group' });
    group.appendChild(createElement(forId ? 'label' : 'div', { className: 'form-label', ...(forId ? { for: forId } : {}) }, labelText));
    group.appendChild(control);
    if (hint) group.appendChild(createElement('p', { className: 'form-hint' }, hint));
    return group;
}

/**
 * Создание (`habit = null`) и правка привычки в модальном окне. Проверяет
 * бэкенд; его ответ об ошибке показывается под формой, окно не закрывается.
 */
function openHabitForm(habit, onSaved) {
    const existing = $('.modal-overlay');
    if (existing) existing.remove();

    const draft = {
        title: habit?.title ?? '',
        description: habit?.description ?? '',
        frequency_type: habit?.frequency_type ?? 'daily',
        weekdays: habit?.weekdays?.length ? [...habit.weekdays] : ['mon', 'wed', 'fri'],
        target_per_week: habit?.target_per_week ?? 3,
        time_of_day: habit?.time_of_day ?? 'morning',
        effort_level: habit?.effort_level ?? 2,
        fallback_title: habit?.fallback_title ?? '',
    };

    const overlay = createElement('div', { className: 'modal-overlay' });
    const modal = createElement('div', {
        className: 'modal habit-form',
        role: 'dialog',
        'aria-modal': 'true',
        'aria-labelledby': 'habit-form-title',
    });

    const header = createElement('div', { className: 'modal__header' });
    header.appendChild(createElement('h2', { className: 'modal__title', id: 'habit-form-title' }, habit ? 'Изменить привычку' : 'Новая привычка'));
    const closeBtn = createElement('button', { className: 'modal__close', type: 'button', 'aria-label': 'Закрыть', innerHTML: Icons.x });
    closeBtn.addEventListener('click', () => overlay.remove());
    header.appendChild(closeBtn);
    modal.appendChild(header);

    const body = createElement('div', { className: 'modal__body' });

    const titleInput = createElement('input', {
        className: 'form-input', id: 'habit-title', maxlength: '120', placeholder: 'Например, «Медитация 15 минут»',
    });
    titleInput.value = draft.title;
    body.appendChild(formGroup('Название', titleInput, null, 'habit-title'));

    const descInput = createElement('textarea', {
        className: 'form-textarea', id: 'habit-description', rows: '2', maxlength: '1000', placeholder: 'Зачем она и как её делать',
    });
    descInput.value = draft.description;
    body.appendChild(formGroup('Описание', descInput, null, 'habit-description'));

    // Периодичность и её параметры: дни недели или норма.
    const freqExtra = createElement('div', { className: 'habit-form__extra' });
    const paintFreqExtra = () => {
        freqExtra.innerHTML = '';
        if (draft.frequency_type === 'weekly_days') {
            freqExtra.appendChild(createChoice(
                WEEKDAYS.map(([value, label]) => ({ value, label })),
                draft.weekdays,
                (days) => { draft.weekdays = days; },
                { multiple: true, label: 'Дни недели' },
            ));
        } else if (draft.frequency_type === 'x_per_week') {
            freqExtra.appendChild(createChoice(
                [1, 2, 3, 4, 5, 6, 7].map(n => ({ value: String(n), label: String(n) })),
                String(draft.target_per_week),
                (n) => { draft.target_per_week = Number(n); },
                { label: 'Раз в неделю' },
            ));
            freqExtra.appendChild(createElement('span', { className: 'form-hint habit-form__inline-hint' }, 'раз в неделю, в любые дни'));
        }
    };
    const freqWrap = createElement('div');
    freqWrap.appendChild(createChoice(FREQUENCIES, draft.frequency_type, (value) => {
        draft.frequency_type = value;
        paintFreqExtra();
    }, { label: 'Периодичность' }));
    freqWrap.appendChild(freqExtra);
    paintFreqExtra();
    body.appendChild(formGroup('Периодичность', freqWrap,
        'Стрик у «N раз в неделю» считается неделями, у остальных — днями по расписанию.'));

    body.appendChild(formGroup('Время суток', createChoice(TIMES_OF_DAY, draft.time_of_day, (value) => {
        draft.time_of_day = value;
    }, { label: 'Время суток' }), 'По нему ИИ-аналитика ищет конфликты с нагрузкой по задачам.'));

    const effortHint = createElement('p', { className: 'form-hint' });
    const paintEffortHint = () => {
        effortHint.textContent = `${draft.effort_level} — ${EFFORT_HINTS[draft.effort_level]}. Когнитивная или физическая нагрузка.`;
    };
    const effortGroup = formGroup('Нагрузка', createChoice(
        [1, 2, 3, 4, 5].map(n => ({ value: String(n), label: String(n) })),
        String(draft.effort_level),
        (n) => { draft.effort_level = Number(n); paintEffortHint(); },
        { label: 'Нагрузка от 1 до 5' },
    ));
    effortGroup.appendChild(effortHint);
    paintEffortHint();
    body.appendChild(effortGroup);

    const fallbackInput = createElement('input', {
        className: 'form-input', id: 'habit-fallback', maxlength: '120', placeholder: 'Например, «3 минуты дыхания»',
    });
    fallbackInput.value = draft.fallback_title;
    body.appendChild(formGroup('Лайт-версия', fallbackInput,
        'Упрощённый вариант на перегруженные дни. Выполненная лайт-версия сохраняет стрик.', 'habit-fallback'));

    const error = createElement('p', { className: 'habit-form__error', role: 'alert' });
    body.appendChild(error);
    modal.appendChild(body);

    const footer = createElement('div', { className: 'modal__footer' });
    const cancel = createElement('button', { className: 'btn btn--secondary', type: 'button' }, 'Отмена');
    cancel.addEventListener('click', () => overlay.remove());
    const save = createElement('button', { className: 'btn btn--primary', type: 'button' }, habit ? 'Сохранить' : 'Создать');
    footer.append(cancel, save);
    modal.appendChild(footer);

    const submit = async () => {
        const input = {
            title: titleInput.value,
            description: descInput.value || null,
            frequency_type: draft.frequency_type,
            weekdays: draft.frequency_type === 'weekly_days' ? draft.weekdays : [],
            target_per_week: draft.frequency_type === 'x_per_week' ? draft.target_per_week : null,
            time_of_day: draft.time_of_day,
            effort_level: draft.effort_level,
            fallback_title: fallbackInput.value || null,
        };
        save.disabled = true;
        error.textContent = '';
        try {
            if (habit) await api.updateHabit(habit.id, input);
            else await api.createHabit(input);
            overlay.remove();
            showToast(habit ? 'Привычка сохранена' : `Привычка «${input.title.trim()}» создана`);
            onSaved();
        } catch (e) {
            error.textContent = String(e);
            save.disabled = false;
        }
    };
    save.addEventListener('click', submit);
    titleInput.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') submit();
    });

    overlay.appendChild(modal);
    overlay.addEventListener('click', (e) => {
        if (e.target === overlay) overlay.remove();
    });
    document.body.appendChild(overlay);
    titleInput.focus();
}

// ─── Тепловая карта ───

let heatmapHabitId = null;

/** Уровень заливки 0–4 по доле выполненного от того, что стояло по расписанию. */
function heatLevel(cell) {
    const done = cell.completed + cell.fallback;
    if (!done) return 0;
    return Math.min(4, Math.max(1, Math.ceil((done / Math.max(cell.due, 1)) * 4)));
}

function heatTooltip(cell) {
    const done = cell.completed + cell.fallback;
    const parts = [];
    if (cell.due || done) parts.push(`выполнено ${done} из ${Math.max(cell.due, done)}`);
    if (cell.fallback) parts.push(`лайт ${cell.fallback}`);
    if (cell.skipped) parts.push(`пропущено ${cell.skipped}`);
    if (cell.failed) parts.push(`сорвалось ${cell.failed}`);
    return `${formatShortDay(cell.date)}: ${parts.length ? parts.join(', ') : 'ничего по расписанию'}`;
}

/**
 * Карта активности. `wave` — первый показ (или другая привычка в списке):
 * клетки загораются волной от старых недель к сегодняшней, как будто время
 * прокручивается до «сейчас», и сегодняшняя клетка дважды вспыхивает. После
 * отметки карта перерисовывается без волны — мигает только сегодняшний день.
 */
async function renderHeatmapSection(section, activeHabits, { wave = false } = {}) {
    // Данные — до очистки: иначе при каждой отметке панель на миг
    // схлопывалась бы до заголовка и прыгала по высоте.
    const width = section.querySelector('.heatmap-holder')?.clientWidth
        || section.clientWidth - 2 * HEATMAP_PANEL_PADDING_PX;
    const weekCount = Math.max(HEATMAP_MIN_WEEKS, Math.min(HEATMAP_MAX_WEEKS,
        Math.floor((width - HEATMAP_LABELS_PX) / HEATMAP_WEEK_PX)));
    // Целыми неделями с понедельника: последняя колонка — текущая неделя до
    // сегодняшнего дня.
    const weekday = (new Date().getDay() + 6) % 7;
    const days = (weekCount - 1) * 7 + weekday + 1;
    let cells = null;
    try {
        cells = await api.getHabitHeatmap(heatmapHabitId, days);
    } catch (e) {
        console.error('Не удалось построить карту:', e);
    }
    const ping = !wave && pendingHeatPing;
    pendingHeatPing = false;

    section.innerHTML = '';
    const head = createElement('div', { className: 'habits-panel__head' });
    head.appendChild(createElement('h3', { className: 'habits-section__title', id: 'habits-heat-title' }, 'Активность'));

    if (heatmapHabitId && !activeHabits.some(h => h.id === heatmapHabitId)) heatmapHabitId = null;
    const select = createElement('select', { className: 'form-input habits-panel__select', 'aria-label': 'Привычка на карте' });
    select.appendChild(createElement('option', { value: '' }, 'Все привычки'));
    for (const h of activeHabits) select.appendChild(createElement('option', { value: String(h.id) }, h.title));
    select.value = heatmapHabitId ? String(heatmapHabitId) : '';
    select.addEventListener('change', () => {
        heatmapHabitId = select.value ? Number(select.value) : null;
        renderHeatmapSection(section, activeHabits, { wave: true });
    });
    head.appendChild(select);
    section.appendChild(head);

    const holder = createElement('div', { className: 'heatmap-holder' });
    section.appendChild(holder);
    if (!cells) {
        holder.appendChild(createElement('p', { className: 'page__empty' }, 'Не удалось построить карту'));
        return;
    }

    const grid = createElement('div', { className: `heatmap${wave ? ' heatmap--wave' : ''}`, role: 'img' });
    // Шаг волны — так, чтобы вся она уложилась в HEATMAP_WAVE_MS при любой ширине.
    const step = HEATMAP_WAVE_MS / (weekCount + 3);
    grid.style.setProperty('--heat-step', `${step.toFixed(1)}ms`);
    grid.style.setProperty('--heat-wave-end', `${Math.round(HEATMAP_WAVE_MS + 200)}ms`);
    const months = createElement('div', { className: 'heatmap__months', 'aria-hidden': 'true' });
    const body = createElement('div', { className: 'heatmap__body' });
    const labels = createElement('div', { className: 'heatmap__weekdays', 'aria-hidden': 'true' });
    ['Пн', '', 'Ср', '', 'Пт', '', ''].forEach(l => labels.appendChild(createElement('span', {}, l)));
    const weeks = createElement('div', { className: 'heatmap__weeks' });

    let lastMonth = -1;
    let total = 0;
    let activeDays = 0;
    for (let w = 0; w < weekCount; w++) {
        const col = createElement('div', { className: 'heatmap__week' });
        const first = cells[w * 7];
        const month = first ? Number(first.date.slice(5, 7)) : -1;
        const monthLabel = createElement('span', { className: 'heatmap__month' });
        if (first && month !== lastMonth) {
            monthLabel.textContent = new Date(Number(first.date.slice(0, 4)), month - 1, 1)
                .toLocaleDateString('ru-RU', { month: 'short' }).replace('.', '');
            lastMonth = month;
        }
        months.appendChild(monthLabel);
        for (let d = 0; d < 7; d++) {
            const cell = cells[w * 7 + d];
            if (!cell) {
                col.appendChild(createElement('span', { className: 'heatmap__cell heatmap__cell--future' }));
                continue;
            }
            const done = cell.completed + cell.fallback;
            total += done;
            if (done) activeDays++;
            const idle = !cell.due && !done && !cell.skipped && !cell.failed;
            const isToday = cell.date === todayKey();
            const el = createElement('span', {
                className: [
                    'heatmap__cell',
                    `heatmap__cell--l${heatLevel(cell)}`,
                    idle ? 'heatmap__cell--idle' : '',
                    isToday ? 'heatmap__cell--today' : '',
                    isToday && ping ? 'heatmap__cell--ping' : '',
                ].filter(Boolean).join(' '),
                'data-tooltip': heatTooltip(cell),
            });
            // Фронт волны идёт по неделям с лёгким наклоном по дням.
            if (wave) el.style.setProperty('--i', String(w + d * 0.45));
            col.appendChild(el);
        }
        weeks.appendChild(col);
    }
    body.append(labels, weeks);
    grid.append(months, body);
    // Год — когда карта начинается в прошлом году: «с 24 нояб.» иначе читалось бы как этот ноябрь.
    const firstDay = cells[0]?.date ?? '';
    const since = firstDay
        ? `с ${formatShortDay(firstDay)}${firstDay.slice(0, 4) !== todayKey().slice(0, 4) ? ` ${firstDay.slice(0, 4)}` : ''}`
        : '';
    grid.setAttribute('aria-label', `Тепловая карта ${since}: ${total} ${pluralize(total, ['выполнение', 'выполнения', 'выполнений'])}`);
    holder.appendChild(grid);

    const foot = createElement('div', { className: 'heatmap__foot' });
    foot.appendChild(createElement('span', {},
        `${total} ${pluralize(total, ['выполнение', 'выполнения', 'выполнений'])} · ` +
        `${activeDays} ${pluralize(activeDays, ['активный день', 'активных дня', 'активных дней'])} · ${since}`));
    const legend = createElement('span', { className: 'heatmap__legend', 'aria-hidden': 'true' });
    legend.appendChild(document.createTextNode('Меньше'));
    for (let l = 0; l <= 4; l++) legend.appendChild(createElement('span', { className: `heatmap__cell heatmap__cell--l${l}` }));
    legend.appendChild(document.createTextNode('Больше'));
    foot.appendChild(legend);
    holder.appendChild(foot);
}

// ─── ИИ-аналитика ───

let analysisRunning = null; // промис идущего анализа: переживает уход со страницы

function formatGenerated(report) {
    const at = parseTimestamp(report.generated_at);
    const when = at
        ? at.toLocaleString('ru-RU', { day: 'numeric', month: 'short', hour: '2-digit', minute: '2-digit' })
        : report.generated_at;
    return `Сделан ${when} · ${report.model} · ${formatShortDay(report.period_from)} — ${formatShortDay(report.period_to)}`;
}

/**
 * `fresh` — отчёт только что пришёл от модели: вердикт «прописывается» лучом
 * слева направо (тот же луч, что читал график), пункты встают по одному.
 * Отчёт из кэша показывается сразу — его уже видели.
 */
function renderReport(holder, report, { fresh = false } = {}) {
    holder.innerHTML = '';
    holder.classList.toggle('habit-ai__report--fresh', fresh);
    if (!report) {
        holder.appendChild(createElement('p', { className: 'habit-ai__placeholder' },
            'Анализ ещё не запускался. Модель получит сводку за 14 дней — задачи со сроком, закрытые и просроченные, ' +
            'учтённое время и отметки привычек с причинами срывов — и найдёт, где одно мешает другому. ' +
            'Нужна запущенная Ollama и модель, выбранная в Настройках.'));
        return;
    }
    const { analysis } = report;

    holder.appendChild(createElement('p', { className: 'habit-ai__meta' }, formatGenerated(report)));
    if (report.stale) {
        holder.appendChild(createElement('p', { className: 'habit-ai__stale' },
            'Данные изменились после этого анализа — запустите заново, чтобы учесть новые отметки и задачи.'));
    }

    holder.appendChild(createElement('p', { className: 'habit-ai__verdict' }, analysis.workload_verdict));
    let order = 0;
    const cue = (el) => {
        el.style.setProperty('--i', String(Math.min(order++, 10)));
        return el;
    };

    holder.appendChild(cue(createElement('h4', { className: 'habit-ai__subtitle' }, 'Закономерности')));
    if (!analysis.correlations.length) {
        holder.appendChild(cue(createElement('p', { className: 'habit-ai__none' }, 'Связей между нагрузкой и срывами не найдено.')));
    } else {
        const list = createElement('ul', { className: 'habit-ai__list' });
        for (const c of analysis.correlations) {
            const item = cue(createElement('li', { className: 'habit-ai__item' }));
            item.appendChild(createElement('span', {
                className: `habit-ai__confidence habit-ai__confidence--${c.confidence}`,
                'data-tooltip': 'Уверенность',
            }, CONFIDENCE_LABELS[c.confidence] || c.confidence));
            const text = createElement('div', { className: 'habit-ai__text' });
            text.appendChild(createElement('span', { className: 'habit-ai__pattern' }, c.pattern));
            text.appendChild(createElement('span', { className: 'habit-ai__evidence' }, c.evidence));
            item.appendChild(text);
            list.appendChild(item);
        }
        holder.appendChild(list);
    }

    holder.appendChild(cue(createElement('h4', { className: 'habit-ai__subtitle' }, 'Что изменить')));
    if (!analysis.actionable_adjustments.length) {
        holder.appendChild(cue(createElement('p', { className: 'habit-ai__none' }, 'Предложений нет.')));
    } else {
        const list = createElement('ul', { className: 'habit-ai__list' });
        for (const a of analysis.actionable_adjustments) {
            const item = cue(createElement('li', { className: 'habit-ai__item' }));
            item.appendChild(createElement('span', { className: 'habit-ai__habit' }, a.habit || 'Общее'));
            const text = createElement('div', { className: 'habit-ai__text' });
            text.appendChild(createElement('span', { className: 'habit-ai__pattern' }, a.change));
            if (a.reason) text.appendChild(createElement('span', { className: 'habit-ai__evidence' }, a.reason));
            item.appendChild(text);
            list.appendChild(item);
        }
        holder.appendChild(list);
    }
}

/** График «задачи и привычки за 14 дней» — то же, что уходит модели. */
async function renderSummaryChart(wrap) {
    try {
        const days = await api.getHabitTaskSummary();
        const canvas = createElement('canvas', { 'aria-label': 'Задачи и привычки за 14 дней', role: 'img' });
        wrap.appendChild(canvas);
        renderLineChart(canvas, days.map(d => formatShortDay(d.date)), [
            { label: 'Задач со сроком', data: days.map(d => d.tasks_due), color: 'accent' },
            { label: 'Просрочено', data: days.map(d => d.tasks_overdue), color: 'danger' },
            { label: 'Привычек выполнено', data: days.map(d => d.habits_completed + d.habits_fallback), color: 'success' },
            { label: 'Привычек сорвано', data: days.map(d => d.habits_failed + d.habits_missed), color: 'warning' },
        ]);
    } catch (e) {
        console.error('Не удалось построить сводку за 14 дней:', e);
        wrap.appendChild(createElement('p', { className: 'form-hint' }, 'Сводку за 14 дней построить не удалось'));
    }
}

async function renderAiSection(section) {
    section.innerHTML = '';
    const head = createElement('div', { className: 'habits-panel__head' });
    head.appendChild(createElement('h3', { className: 'habits-section__title', id: 'habits-ai-title' }, 'ИИ-аналитика привычек и нагрузки'));
    const run = createElement('button', { className: 'btn btn--primary', type: 'button' });
    head.appendChild(run);
    section.appendChild(head);

    const chartWrap = createElement('div', { className: 'habit-ai__chart' });
    section.appendChild(chartWrap);
    renderSummaryChart(chartWrap);

    const status = createElement('div', { className: 'habit-ai__status', role: 'status', 'aria-live': 'polite' });
    section.appendChild(status);
    const holder = createElement('div', { className: 'habit-ai__report' });
    section.appendChild(holder);

    let report = null;
    const paintButton = (busy) => {
        run.disabled = busy;
        run.innerHTML = busy
            ? '<span class="spinner habit-ai__spinner"></span><span>Анализирую…</span>'
            : `${Icons.sparkles}<span>${report ? 'Обновить анализ' : 'Запустить анализ'}</span>`;
    };

    // Пока модель думает, по графику за 14 дней ходит луч: модель читает
    // именно эти дни, слева направо.
    const beam = createElement('span', { className: 'habit-ai__beam', 'aria-hidden': 'true' });
    const reading = (on) => {
        chartWrap.classList.toggle('habit-ai__chart--reading', on);
        if (on) chartWrap.appendChild(beam);
        else beam.remove();
    };

    const start = async () => {
        paintButton(true);
        reading(true);
        status.innerHTML = '';
        status.appendChild(createElement('p', { className: 'form-hint' },
            'Модель читает сводку за 14 дней. На локальной Ollama это минута-две, первый запуск дольше.'));
        try {
            analysisRunning = analysisRunning || api.runHabitAnalysis();
            report = await analysisRunning;
            if (!section.isConnected) return;
            status.innerHTML = '';
            renderReport(holder, report, { fresh: true });
        } catch (e) {
            if (!section.isConnected) return;
            status.innerHTML = '';
            status.appendChild(createElement('p', { className: 'habit-ai__error' }, String(e)));
        } finally {
            analysisRunning = null;
            if (section.isConnected) {
                reading(false);
                paintButton(false);
            }
        }
    };
    run.addEventListener('click', start);

    try {
        report = await api.getHabitAnalysis();
    } catch (e) {
        console.error('Не удалось прочитать кэш анализа:', e);
    }
    renderReport(holder, report);
    paintButton(false);
    // Анализ, начатый до ухода со страницы, ещё идёт — показываем его исход.
    if (analysisRunning) start();
}
