// ============================================
// TaskFlow — Учёт времени
// ============================================
// Кнопка таймера на карточке в канбане, индикатор идущего таймера в шапке,
// блок «Время» в окне карточки и вопрос про забытый таймер при запуске.
//
// Таймер ведёт «я» — строка `is_self` справочника участников, та же, что
// подписывает комментарии: входа в приложение нет, и другого человека за
// клавиатурой не бывает.
//
// Идущий таймер хранится здесь, один на всё приложение. Шапка, карточки и окно
// карточки своей копии не держат и перерисовываются, когда он меняется: иначе
// таймер, остановленный в шапке, продолжал бы «идти» на карточке.
//
// Модуль не импортирует `board.js` — тот импортирует этот (то же правило, что
// у `dependencies.js`).

import * as api from './api.js';
import Icons from './icons.js';
import { loadMembers, createAvatar } from './members.js';
import { refreshTooltip } from './popover.js';
import {
    createElement, $, $$, showToast, pluralize, parseTimestamp, toDateKey, todayKey,
    toTimestamp, formatDueDate, formatDuration, formatClock, manualSessionStart,
} from './utils.js';

/** Раз во столько тиков (секунд) идущий таймер сверяется с базой. */
const RESYNC_TICKS = 30;

let selfId = null;
/** Идущий таймер «меня» — `ActiveTimer` с бэкенда — или `null`. */
let active = null;
/**
 * Растёт при каждой смене `active`. Сверка с базой, отправленная до клика
 * «старт» или «стоп», по нему узнаёт, что её ответ устарел.
 */
let version = 0;
/** Старт или стоп ещё в полёте: второй клик в это время не отправляется. */
let busy = false;
let ticker = 0;
let ticksToResync = RESYNC_TICKS;
const listeners = new Set();

const errorText = (e, fallback) => (typeof e === 'string' ? e : fallback);

// ─── Состояние ───────────────────────────────────────────────────────────

/**
 * Подключает индикатор в шапке и читает идущий таймер. Вызывается один раз из
 * `app.js`, после отрисовки макета.
 */
export async function initTimer() {
    const indicator = $('#header-timer');
    if (indicator) {
        indicator.addEventListener('click', () => exclusive(stopCurrent));
    }
    try {
        await syncActiveTimer();
    } catch (e) {
        console.error('Не удалось прочитать идущий таймер:', e);
    }
}

async function resolveSelf() {
    if (selfId !== null) return selfId;
    const me = (await loadMembers()).find(m => m.is_self);
    // Строку «я» миграция создаёт при каждом запуске, а удалить её нельзя, так
    // что её отсутствие — сломанная база, а не обычная ситуация.
    if (!me) throw new Error('В справочнике участников нет строки «я»');
    selfId = me.id;
    return selfId;
}

async function syncActiveTimer() {
    const me = await resolveSelf();
    const seen = version;
    const fresh = await api.getActiveTimer(me);
    // Пока шёл запрос, человек успел нажать «старт» или «стоп» — его ответ новее.
    if (seen !== version) return;
    setActive(fresh);
}

function resync() {
    syncActiveTimer().catch(e => console.error('Не удалось сверить таймер с базой:', e));
}

function setActive(timer) {
    const changed = (timer ? timer.entry_id : null) !== (active ? active.entry_id : null);
    active = timer;
    version++;
    paintIndicator();
    paintCardButtons();
    ensureTicker();
    if (changed) {
        // Копия набора: слушатель может отписаться прямо во время обхода.
        for (const fn of [...listeners]) fn();
    }
}

function onTimerChange(fn) {
    listeners.add(fn);
    return () => listeners.delete(fn);
}

function isRunning(cardId) {
    return active !== null && active.card_id === cardId;
}

async function exclusive(action) {
    if (busy) return;
    busy = true;
    try {
        await action();
    } finally {
        busy = false;
    }
}

/** Запускает таймер на карточке или останавливает, если он на ней уже идёт. */
function toggleTimer(cardId) {
    return exclusive(() => (isRunning(cardId) ? stopCurrent() : startOn(cardId)));
}

async function startOn(cardId) {
    try {
        const { timer, stopped } = await api.startTimer(cardId, await resolveSelf());
        setActive(timer);
        // Два таймера у одного человека не идут: прежний бэкенд остановил сам.
        // Молча закрытая сессия выглядела бы как пропавшее время.
        if (stopped) {
            showToast(`Таймер «${stopped.card_title}» остановлен: записано ${formatDuration(stopped.duration_seconds)}`, 'info');
        }
    } catch (e) {
        showToast(errorText(e, 'Не удалось запустить таймер'), 'error');
        resync();
    }
}

async function stopCurrent() {
    const current = active;
    if (!current) return;
    try {
        const entry = await api.stopTimer(current.entry_id);
        setActive(null);
        showToast(`Записано ${formatDuration(entry.duration_seconds)} — «${current.card_title}»`);
    } catch (e) {
        showToast(errorText(e, 'Не удалось остановить таймер'), 'error');
        // Запись могла исчезнуть вместе с карточкой, колонкой или доской —
        // сверка уберёт из шапки таймер, которого больше нет.
        resync();
    }
}

// ─── Тикающие счётчики ───────────────────────────────────────────────────
//
// Любой элемент с `data-timer-start` (миллисекунды запуска) показывает, сколько
// прошло, и обновляется одним общим интервалом: счётчик в шапке и строка идущей
// сессии в окне карточки. Интервал живёт, пока такие элементы есть в документе.

function startMs(startedAt) {
    return parseTimestamp(startedAt).getTime();
}

function liveClock(className, startedAt) {
    const el = createElement('span', {
        className,
        dataset: { timerStart: String(startMs(startedAt)) },
    });
    paintLive(el, Date.now());
    return el;
}

function paintLive(el, now) {
    el.textContent = formatClock((now - Number(el.dataset.timerStart)) / 1000);
}

function ensureTicker() {
    if (ticker || !document.querySelector('[data-timer-start]')) return;
    ticker = setInterval(tick, 1000);
}

function tick() {
    const live = $$('[data-timer-start]');
    if (!live.length) {
        clearInterval(ticker);
        ticker = 0;
        return;
    }
    const now = Date.now();
    for (const el of live) paintLive(el, now);

    // Раз в полминуты — сверка с базой: таймер мог исчезнуть вместе с
    // удалённой доской, и шапка не должна показывать его до следующего клика.
    if (active && !busy && --ticksToResync <= 0) {
        ticksToResync = RESYNC_TICKS;
        resync();
    }
}

// ─── Индикатор в шапке ───────────────────────────────────────────────────

function paintIndicator() {
    const indicator = $('#header-timer');
    if (!indicator) return;
    const time = $('.header-timer__time', indicator);

    indicator.hidden = !active;
    if (!active) {
        // Без отметки запуска скрытый счётчик не держит интервал впустую.
        delete time.dataset.timerStart;
        return;
    }
    $('.header-timer__title', indicator).textContent = active.card_title;
    time.dataset.timerStart = String(startMs(active.started_at));
    paintLive(time, Date.now());
    // Название в шапке обрезается многоточием — в подсказке оно целиком.
    indicator.dataset.tooltip = `Остановить таймер «${active.card_title}»`;
}

// ─── Кнопка на карточке в канбане ────────────────────────────────────────

/**
 * Добавляет кнопку таймера на лицевую сторону карточки. Пока на карточке идёт
 * таймер, кнопка показывает «стоп», а сама карточка подсвечена.
 */
export function attachTimerButton(card, cardId) {
    const btn = createElement('button', {
        className: 'card__timer-btn',
        type: 'button',
        dataset: { timerCard: String(cardId) },
    });
    btn.addEventListener('click', (e) => {
        // Как у соседних кнопок: щелчок по таймеру не открывает окно карточки.
        e.stopPropagation();
        toggleTimer(cardId);
    });
    card.appendChild(btn);
    paintCardButton(btn);
}

function paintCardButton(btn) {
    const running = isRunning(Number(btn.dataset.timerCard));
    // Сверка с базой перерисовывает все кнопки раз в полминуты; менять разметку
    // стоит только там, где состояние действительно сменилось.
    if (btn.firstChild && btn.classList.contains('card__timer-btn--running') === running) return;

    btn.innerHTML = running ? Icons.stop : Icons.clock;
    btn.dataset.tooltip = running ? 'Остановить таймер' : 'Запустить таймер';
    refreshTooltip(btn);
    btn.classList.toggle('card__timer-btn--running', running);
    const card = btn.closest('.card');
    if (card) card.classList.toggle('card--timing', running);
}

function paintCardButtons() {
    for (const btn of $$('.card__timer-btn')) paintCardButton(btn);
}

// ─── Блок «Время» в окне карточки ────────────────────────────────────────

/**
 * Рисует блок «Время»: сумму, список сессий и форму ручного ввода.
 *
 * Устроен как `renderChecklist` и `renderComments`: контейнер и карточка
 * снаружи, загрузка асинхронная, окно к этому моменту уже на экране. Сессия,
 * вписанная руками, сохраняется сразу, а не по «Сохранить» окна.
 *
 * @param {HTMLElement} container
 * @param {number} cardId
 */
export async function renderTimeLog(container, cardId) {
    const group = createElement('div', { className: 'form-group time-log' });

    const header = createElement('div', { className: 'time-log__header' });
    header.appendChild(createElement('label', { className: 'form-label' }, 'Время'));
    const total = createElement('span', { className: 'time-log__total' });
    header.appendChild(total);
    group.appendChild(header);

    const list = createElement('div', { className: 'time-log__list' });
    group.appendChild(list);

    const addBtn = createElement('button', {
        className: 'time-log__add',
        type: 'button',
    }, '+ Добавить вручную');
    group.appendChild(addBtn);

    container.appendChild(group);

    let entries = [];
    let totalSeconds = 0;

    const paint = () => {
        // Идущая сессия в сумму не входит: её длительности ещё нет, и число в
        // окне не должно меняться само по себе.
        total.textContent = totalSeconds > 0 ? `Всего ${formatDuration(totalSeconds)}` : '';
        list.innerHTML = '';
        if (!entries.length) {
            list.appendChild(createElement('div', { className: 'time-log__empty' },
                'Время по задаче ещё не записывалось'));
            return;
        }
        for (const entry of entries) list.appendChild(createSessionRow(entry));
        ensureTicker();
    };

    const reload = async () => {
        try {
            [entries, totalSeconds] = await Promise.all([
                api.listTimeEntries(cardId),
                api.getTotalTime(cardId),
            ]);
        } catch (e) {
            showToast('Не удалось загрузить время по задаче', 'error');
        }
        paint();
    };

    addBtn.addEventListener('click', () => {
        addBtn.hidden = true;
        openManualForm(group, cardId, async (added) => {
            addBtn.hidden = false;
            if (added) await reload();
        });
    });

    // Таймер, запущенный или остановленный не отсюда, меняет и этот список.
    // Отписка ленивая: окно закрыли — слушатель заметит это при первом же
    // изменении, увидев, что его блока в документе больше нет.
    const unsubscribe = onTimerChange(() => {
        if (!group.isConnected) {
            unsubscribe();
            return;
        }
        reload();
    });

    await reload();
}

function createSessionRow(entry) {
    const running = !entry.ended_at;
    const row = createElement('div', {
        className: `time-log__row ${running ? 'time-log__row--running' : ''}`,
    });

    // Участника могли удалить из справочника — время при этом остаётся.
    row.appendChild(createAvatar(entry.member, { size: 'sm', tooltip: false }));

    const main = createElement('div', { className: 'time-log__main' });
    main.appendChild(createElement('span', { className: 'time-log__who' },
        entry.member ? entry.member.name : 'Участник удалён'));
    if (entry.note) {
        // Текстом, а не разметкой: подпись пишет человек.
        main.appendChild(createElement('span', { className: 'time-log__note' }, entry.note));
    }
    row.appendChild(main);

    if (running) {
        row.appendChild(liveClock('time-log__duration', entry.started_at));
        row.appendChild(createElement('span', {
            className: 'time-log__date time-log__date--running',
        }, 'идёт'));
    } else {
        row.appendChild(createElement('span', { className: 'time-log__duration' },
            formatDuration(entry.duration_seconds)));
        row.appendChild(createElement('span', {
            className: 'time-log__date',
            'data-tooltip': sessionHours(entry),
        }, sessionDay(entry)));
    }
    return row;
}

/** День сессии тем же словарём, что и срок карточки: «Сегодня», «Вчера», «10 сент.». */
function sessionDay(entry) {
    return formatDueDate(toDateKey(parseTimestamp(entry.started_at)));
}

/** «14:05–14:30» по местным часам — подсказка к дню. */
function sessionHours(entry) {
    const clock = (value) => parseTimestamp(value)
        .toLocaleTimeString('ru-RU', { hour: '2-digit', minute: '2-digit' });
    return `${clock(entry.started_at)}–${clock(entry.ended_at)}`;
}

/**
 * Форма «добавить вручную» — для работы, на которую забыли включить таймер.
 * Спрашивает день и длительность; куда в этом дне лечь сессии, решает
 * `manualSessionStart`.
 */
function openManualForm(container, cardId, onDone) {
    const form = createElement('div', { className: 'time-log__form' });

    const fields = createElement('div', { className: 'time-log__fields' });

    const dayField = createElement('label', { className: 'time-log__field' });
    dayField.appendChild(createElement('span', { className: 'time-log__field-label' }, 'День'));
    const today = todayKey();
    const dayInput = createElement('input', {
        className: 'form-input time-log__day',
        type: 'date',
        max: today,
        value: today,
    });
    dayField.appendChild(dayInput);
    fields.appendChild(dayField);

    const durationField = createElement('div', { className: 'time-log__field' });
    durationField.appendChild(createElement('span', { className: 'time-log__field-label' }, 'Сколько'));
    const duration = createDurationInput();
    durationField.appendChild(duration.el);
    fields.appendChild(durationField);

    form.appendChild(fields);

    const noteInput = createElement('input', {
        className: 'form-input',
        placeholder: 'Что делали — необязательно',
        maxlength: '500',
    });
    form.appendChild(noteInput);

    const actions = createElement('div', { className: 'time-log__form-actions' });
    const cancelBtn = createElement('button', { className: 'btn btn--secondary btn--sm', type: 'button' }, 'Отмена');
    const saveBtn = createElement('button', { className: 'btn btn--primary btn--sm', type: 'button' }, 'Добавить');
    actions.append(cancelBtn, saveBtn);
    form.appendChild(actions);

    const close = (added) => {
        form.remove();
        onDone(added);
    };
    cancelBtn.addEventListener('click', () => close(false));

    const submit = async () => {
        const seconds = duration.getSeconds();
        if (seconds <= 0) {
            showToast('Укажите, сколько длилась работа', 'error');
            duration.focus();
            return;
        }
        const day = dayInput.value;
        const start = manualSessionStart(day, seconds);
        if (!start) {
            showToast(placementError(day), 'error');
            return;
        }

        saveBtn.disabled = true;
        try {
            await api.addTimeEntry(cardId, await resolveSelf(), toTimestamp(start), seconds,
                noteInput.value.trim() || null);
            showToast(`Добавлено ${formatDuration(seconds)}`);
            close(true);
        } catch (e) {
            showToast(errorText(e, 'Не удалось добавить время'), 'error');
            saveBtn.disabled = false;
        }
    };
    saveBtn.addEventListener('click', submit);

    for (const input of [dayInput, noteInput, ...duration.inputs]) {
        input.addEventListener('keydown', (e) => {
            // Enter отправляет эту форму и дальше, до окна карточки, не идёт.
            if (e.key === 'Enter') {
                e.preventDefault();
                e.stopPropagation();
                submit();
            }
        });
    }

    container.appendChild(form);
    duration.focus();
}

/** Почему сессия не поместилась в выбранный день. */
function placementError(day) {
    const today = todayKey();
    if (!day) return 'Выберите день';
    if (day > today) return 'Нельзя записать время на день, который ещё не наступил';
    if (day === today) return 'Сегодня прошло меньше времени, чем указано';
    return 'В один день столько не поместится';
}

/**
 * Длительность двумя числами — часы и минуты. Одно текстовое поле («1:30»,
 * «1ч 30м», «90») пришлось бы разбирать, и опечатка в нём тихо превращалась бы
 * в другое число.
 */
function createDurationInput() {
    const el = createElement('div', { className: 'duration-input' });
    const number = (label, max) => createElement('input', {
        className: 'form-input duration-input__num',
        type: 'number',
        min: '0',
        ...(max ? { max } : {}),
        step: '1',
        inputmode: 'numeric',
        placeholder: '0',
        'aria-label': label,
    });
    const hours = number('Часы');
    const minutes = number('Минуты', '59');
    el.append(
        hours, createElement('span', { className: 'duration-input__unit' }, 'ч'),
        minutes, createElement('span', { className: 'duration-input__unit' }, 'мин'),
    );

    const read = (input) => Math.max(0, Math.floor(Number(input.value) || 0));
    return {
        el,
        inputs: [hours, minutes],
        getSeconds: () => read(hours) * 3600 + read(minutes) * 60,
        focus: () => hours.focus(),
    };
}

// ─── Забытый таймер ──────────────────────────────────────────────────────

/**
 * Спрашивает про каждый таймер, который идёт дольше двенадцати часов: значит,
 * приложение закрыли или оно упало, а таймер остался. Вызывается один раз при
 * запуске из `app.js`.
 *
 * Молча такой таймер не закрывается — на карточку легли бы десять часов,
 * которых не было. Окно можно закрыть, ничего не решив: таймер продолжит идти,
 * а вопрос повторится при следующем запуске.
 */
export async function askAboutOrphanedTimers() {
    let orphans;
    try {
        orphans = await api.listOrphanedTimers();
    } catch (e) {
        console.error('Не удалось проверить забытые таймеры:', e);
        return;
    }
    // По одному: второе окно поверх первого спрашивало бы о двух задачах сразу.
    for (const timer of orphans) await askAboutOrphan(timer);
}

function askAboutOrphan(timer) {
    return new Promise((resolve) => {
        const overlay = createElement('div', { className: 'modal-overlay modal-overlay--confirm' });
        const modal = createElement('div', { className: 'modal orphan-timer' });

        let settled = false;
        const close = () => {
            if (settled) return;
            settled = true;
            overlay.remove();
            resolve();
        };
        // Esc и щелчок мимо окна значат «решу потом»: таймер не трогаем.
        overlay.__onClose = close;
        overlay.addEventListener('click', (e) => {
            if (e.target === overlay) close();
        });

        const header = createElement('div', { className: 'modal__header' });
        const heading = createElement('div', { className: 'confirm__heading' });
        heading.appendChild(createElement('span', {
            className: 'confirm__icon orphan-timer__icon',
            innerHTML: Icons.clock,
        }));
        heading.appendChild(createElement('h2', { className: 'modal__title' }, 'Незавершённый таймер'));
        header.appendChild(heading);
        modal.appendChild(header);

        const body = createElement('div', { className: 'modal__body' });
        body.appendChild(createElement('p', { className: 'confirm__message' },
            `Обнаружен незавершённый таймер по задаче «${timer.card_title}», запущен `
            + `${timeAgo(timer.elapsed_seconds)} назад. Остановить сейчас с этой длительностью `
            + 'или указать вручную?'));

        const field = createElement('div', { className: 'form-group orphan-timer__field' });
        field.appendChild(createElement('label', { className: 'form-label' },
            'Сколько на самом деле длилась работа'));
        const duration = createDurationInput();
        field.appendChild(duration.el);
        const error = createElement('p', { className: 'orphan-timer__error' });
        error.hidden = true;
        field.appendChild(error);
        body.appendChild(field);

        body.appendChild(createElement('p', { className: 'form-hint' },
            'Если закрыть окно, таймер продолжит идти, а вопрос повторится при следующем запуске.'));
        modal.appendChild(body);

        const footer = createElement('div', { className: 'modal__footer' });
        const stopNowBtn = createElement('button', { className: 'btn btn--secondary', type: 'button' },
            'Остановить сейчас');
        const saveBtn = createElement('button', { className: 'btn btn--primary', type: 'button' },
            'Сохранить указанное');
        footer.append(stopNowBtn, saveBtn);
        modal.appendChild(footer);

        const lock = (locked) => {
            stopNowBtn.disabled = locked;
            saveBtn.disabled = locked;
        };
        const fail = (e, fallback) => {
            error.textContent = errorText(e, fallback);
            error.hidden = false;
            lock(false);
        };
        const finish = (entry) => {
            if (active && active.entry_id === timer.entry_id) setActive(null);
            showToast(`Записано ${formatDuration(entry.duration_seconds)} — «${timer.card_title}»`);
            close();
        };

        stopNowBtn.addEventListener('click', async () => {
            lock(true);
            try {
                finish(await api.stopTimer(timer.entry_id));
            } catch (e) {
                fail(e, 'Не удалось остановить таймер');
            }
        });

        const saveTyped = async () => {
            const seconds = duration.getSeconds();
            if (seconds <= 0) {
                fail(null, 'Укажите длительность больше нуля');
                duration.focus();
                return;
            }
            lock(true);
            try {
                // Длиннее, чем прошло с запуска, бэкенд не примет и объяснит
                // почему — его текст и показываем.
                finish(await api.stopTimerWithDuration(timer.entry_id, seconds));
            } catch (e) {
                fail(e, 'Не удалось сохранить длительность');
            }
        };
        saveBtn.addEventListener('click', saveTyped);
        for (const input of duration.inputs) {
            input.addEventListener('keydown', (e) => {
                if (e.key === 'Enter') {
                    e.preventDefault();
                    saveTyped();
                }
            });
        }

        overlay.appendChild(modal);
        document.body.appendChild(overlay);
        duration.focus();
    });
}

/** «14 часов», «3 минуты» — сколько назад запущен забытый таймер. */
function timeAgo(seconds) {
    const hours = Math.floor(seconds / 3600);
    if (hours >= 1) return `${hours} ${pluralize(hours, ['час', 'часа', 'часов'])}`;
    // Меньше часа бывает только при пониженном пороге в отладочной сборке.
    const minutes = Math.max(1, Math.floor(seconds / 60));
    return `${minutes} ${pluralize(minutes, ['минуту', 'минуты', 'минут'])}`;
}
