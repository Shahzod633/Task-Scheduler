// ============================================
// TaskFlow — ИИ-ассистент (чат с локальной Ollama)
// ============================================
// Ollama — внешний процесс: TaskFlow её не запускает. Поэтому экран первым
// делом выясняет, можно ли вообще разговаривать, и если нельзя — говорит
// почему и что сделать, а историю всё равно показывает: она лежит в базе и от
// Ollama не зависит.
//
// История своя у каждого пространства (`chat_history.workspace_id`).
// Системный промпт и окно контекста живут на бэкенде; отсюда уходит только
// текст нового сообщения.
//
// Изменения ассистент молча не делает: предложенное изменение приходит
// превью с кнопками «Да» / «Отмена», и ответ продолжается только после
// выбора. Само ожидание хранит бэкенд — ушли на доску и вернулись, превью на
// месте.

import * as api from './api.js';
import Icons from './icons.js';
import { confirmDialog } from './dialog.js';
import { renderMarkdown } from './markdown.js';
import { refreshTimer } from './timer.js';
import { createElement, $, showToast, parseTimestamp, autoResize } from './utils.js';

/**
 * Запрос, на который модель ещё думает: `{ workspaceId, text, promise }`.
 *
 * Живёт на уровне модуля, а не страницы: ответ идёт до нескольких минут, и
 * за это время человек успевает уйти на доску и вернуться. Вернувшись, он
 * должен увидеть свой вопрос и «печатает…», а не пустое поле и соблазн
 * спросить второй раз.
 */
let inFlight = null;

/** Инструменты бэкенда (`ai_tools.rs`) — человеческими словами. */
const TOOL_LABELS = {
    get_boards: 'доски',
    get_board_columns: 'колонки доски',
    get_cards_in_column: 'карточки колонки',
    search_cards: 'поиск задач',
    get_card_details: 'задача целиком',
    get_workload: 'нагрузка',
    get_overdue_cards: 'просроченные',
    get_cards_by_deadline_range: 'сроки',
    get_time_summary: 'учёт времени',
};

/** Изменения, после которых стоит перечитать таймер в шапке. */
const TIMER_TOOLS = new Set(['start_timer', 'stop_timer']);

export async function renderAssistantPage(workspaceId) {
    const content = $('#content');
    content.innerHTML = '';
    content.classList.add('view-enter');

    const page = createElement('div', { className: 'page page--assistant' });

    // ─── Шапка ───
    const header = createElement('div', { className: 'assistant__header' });
    const titleBox = createElement('div', { className: 'assistant__title-box' });
    titleBox.appendChild(createElement('h2', { className: 'page__title assistant__title' }, 'ИИ-ассистент'));
    const subtitle = createElement('p', { className: 'assistant__subtitle' }, 'Проверяем подключение к Ollama…');
    titleBox.appendChild(subtitle);
    header.appendChild(titleBox);

    const clearBtn = createElement('button', {
        className: 'btn btn--ghost assistant__clear',
        innerHTML: `${Icons.trash} <span>Очистить историю</span>`,
    });
    header.appendChild(clearBtn);
    page.appendChild(header);

    // Плашка о том, почему поговорить нельзя. Пустая — скрыта.
    const notice = createElement('div', { className: 'assistant__notice', hidden: 'hidden' });
    page.appendChild(notice);

    const list = createElement('div', { className: 'assistant__messages' });
    page.appendChild(list);

    // ─── Поле ввода ───
    const composer = createElement('div', { className: 'assistant__composer' });
    const error = createElement('p', { className: 'assistant__error', hidden: 'hidden' });
    const inputRow = createElement('div', { className: 'assistant__input-row' });
    const input = createElement('textarea', {
        className: 'form-input assistant__input',
        rows: '1',
        placeholder: 'Напишите сообщение…',
        title: 'Enter — отправить, Shift+Enter — новая строка',
    });
    const sendBtn = createElement('button', {
        className: 'btn btn--primary assistant__send',
        innerHTML: Icons.send,
        'data-tooltip': 'Отправить',
    });
    inputRow.appendChild(input);
    inputRow.appendChild(sendBtn);
    composer.appendChild(error);
    composer.appendChild(inputRow);
    page.appendChild(composer);

    content.appendChild(page);
    setTimeout(() => content.classList.remove('view-enter'), 420);

    // ─── Состояние ───
    let ready = false;        // Ollama отвечает и модель на месте
    let history = [];
    /** Изменение, ждущее «Да» / «Отмена» (`AiPendingAction`), или null. */
    let awaiting = null;

    const busy = () => inFlight !== null && inFlight.workspaceId === workspaceId;

    function syncControls() {
        input.disabled = !ready;
        // Пока ждём ответа на превью, новый вопрос не принимается: он
        // пришёлся бы посреди незаконченного ответа.
        sendBtn.disabled = !ready || busy() || awaiting !== null;
        clearBtn.disabled = busy() || (history.length === 0 && awaiting === null);
    }

    function showNotice(text, action) {
        notice.innerHTML = '';
        notice.appendChild(createElement('span', { className: 'assistant__notice-icon', innerHTML: Icons.alertTriangle }));
        notice.appendChild(createElement('p', { className: 'assistant__notice-text' }, text));
        if (action) notice.appendChild(action);
        notice.hidden = false;
    }

    function showError(text) {
        error.textContent = text;
        error.hidden = !text;
    }

    function renderList() {
        list.innerHTML = '';
        if (history.length === 0 && !busy() && !awaiting) {
            list.appendChild(createElement('div', { className: 'assistant__empty' },
                createElement('span', { className: 'assistant__empty-icon', innerHTML: Icons.sparkles }),
                createElement('p', {}, 'Здесь пока пусто. Спросите о своих задачах — например, «Какие задачи просрочены?» ' +
                'или попросите: «Создай задачу „Подготовить отчёт“ на доске …»'),
            ));
        }
        for (const message of history) list.appendChild(messageBubble(message));
        if (busy()) {
            list.appendChild(messageBubble({ role: 'user', content: inFlight.text }));
            list.appendChild(typingBubble());
        } else if (awaiting) {
            list.appendChild(messageBubble({ role: 'user', content: awaiting.user_text }));
            list.appendChild(actionCard(awaiting, ready, confirm));
        }
        list.scrollTop = list.scrollHeight;
        syncControls();
    }

    /** Перечитать историю и ожидание — после ответа, пришедшего без нас. */
    async function reload() {
        try {
            const [fresh, action] = await Promise.all([
                api.getChatHistory(workspaceId),
                api.getPendingAction(workspaceId),
            ]);
            history = fresh;
            awaiting = action;
        } catch (e) {
            showToast('Не удалось загрузить историю чата', 'error');
        }
        if (page.isConnected) renderList();
    }

    /**
     * Ведёт один запрос к ассистенту — вопрос или «Да»/«Отмена» — до итога:
     * ответ ложится в историю, либо приходит следующее превью.
     *
     * При отказе в истории ничего не остаётся (бэкенд пишет пару только
     * после ответа), поэтому вопрос возвращается в поле — переспросить.
     */
    async function track(text, promise) {
        const request = { workspaceId, text, promise };
        inFlight = request;
        awaiting = null;
        showError('');
        renderList();

        try {
            const turn = await promise;
            // Таймер в шапке живёт своей жизнью — после изменения из чата его
            // надо перечитать, даже если сам чат уже закрыли.
            if (turn.messages.some(m => (m.tools_used || []).some(t => TIMER_TOOLS.has(t)))) {
                refreshTimer();
            }
            if (page.isConnected) {
                history.push(...turn.messages);
                awaiting = turn.pending;
            }
        } catch (e) {
            if (page.isConnected) {
                if (!input.value.trim()) input.value = text;
                autoResize(input);
                showError(String(e));
            } else {
                showToast(String(e), 'error');
            }
        } finally {
            if (inFlight === request) inFlight = null;
        }

        if (page.isConnected) {
            renderList();
            if (!awaiting) input.focus();
        }
    }

    function send() {
        const text = input.value.trim();
        if (!text || !ready || busy() || awaiting) return;
        input.value = '';
        autoResize(input);
        track(text, api.ollamaChat(workspaceId, text));
    }

    /** «Да» или «Отмена» на превью. */
    function confirm(approved) {
        const action = awaiting;
        if (!action || busy()) return;
        track(action.user_text, api.ollamaConfirmAction(workspaceId, action.id, approved));
    }

    sendBtn.addEventListener('click', send);
    input.addEventListener('keydown', (e) => {
        // `isComposing` — чтобы Enter, подтверждающий ввод из IME, не
        // отправлял недописанное.
        if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) {
            e.preventDefault();
            send();
        }
    });
    input.addEventListener('input', () => autoResize(input));

    clearBtn.addEventListener('click', async () => {
        const ok = await confirmDialog({
            title: 'Очистить историю?',
            message: 'Вся переписка с ассистентом в этом пространстве будет удалена. ' +
                     'Отменить это нельзя.' +
                     (awaiting ? ' Предложенное изменение тоже отменится.' : ''),
            confirmText: 'Очистить',
            danger: true,
        });
        if (!ok) return;
        try {
            await api.clearChatHistory(workspaceId);
            history = [];
            awaiting = null;
            renderList();
            showToast('История очищена');
        } catch (e) {
            showToast(String(e), 'error');
        }
    });

    // ─── Загрузка ───
    await reload();

    // Вернулись, пока модель ещё думает над прошлым запросом, — дождаться и
    // перечитать. Сам `track()` того вызова уже не видит эту страницу.
    if (busy()) inFlight.promise.catch(() => {}).finally(reload);

    await checkReady();

    async function checkReady() {
        ready = false;
        syncControls();

        let settings;
        try {
            settings = await api.getAiSettings();
        } catch (e) {
            subtitle.textContent = 'Не удалось прочитать настройки';
            showNotice(String(e));
            return;
        }

        const toSettings = createElement('button', { className: 'btn btn--secondary' }, 'Открыть Настройки');
        toSettings.addEventListener('click', () => {
            window.dispatchEvent(new CustomEvent('navigate', { detail: { view: 'settings' } }));
        });

        if (!settings.model) {
            subtitle.textContent = 'Модель не выбрана';
            showNotice('Запустите Ollama и выберите модель в Настройках → «ИИ-ассистент».', toSettings);
            return;
        }

        let models;
        try {
            models = await api.ollamaCheckStatus();
        } catch (e) {
            if (!page.isConnected) return;
            subtitle.textContent = `${settings.model} · нет связи`;
            const retry = createElement('button', { className: 'btn btn--secondary' }, 'Проверить снова');
            retry.addEventListener('click', checkReady);
            showNotice(String(e), retry);
            renderList();
            return;
        }
        if (!page.isConnected) return;

        if (!models.some(m => m.name === settings.model)) {
            subtitle.textContent = `${settings.model} · не найдена`;
            showNotice(`Модели «${settings.model}» нет в Ollama — её могли удалить. ` +
                       'Выберите другую в Настройках → «ИИ-ассистент».', toSettings);
            renderList();
            return;
        }

        subtitle.textContent = `${settings.model} · локально, через Ollama`;
        notice.hidden = true;
        ready = true;
        // Перерисовка, а не только кнопки: превью, нарисованное до проверки
        // связи, держало «Да» выключенной.
        renderList();
        if (!busy() && !awaiting) input.focus();
    }
}

function messageBubble(message) {
    const mine = message.role === 'user';
    const row = createElement('div', { className: `assistant__row assistant__row--${mine ? 'user' : 'ai'}` });
    const bubble = createElement('div', { className: `assistant__bubble assistant__bubble--${mine ? 'user' : 'ai'}` });

    if (mine) {
        // Текст человека показывается как написан — без разметки: звёздочка
        // в вопросе не должна превращаться в курсив.
        bubble.appendChild(createElement('div', { className: 'assistant__text' }, message.content));
    } else {
        // Модели отвечают в markdown. Парсер сперва экранирует весь текст,
        // так что HTML из ответа модели в страницу не попадает.
        bubble.appendChild(createElement('div', {
            className: 'assistant__text markdown-body',
            innerHTML: renderMarkdown(message.content),
        }));
    }

    // Что ассистент менял по ходу ответа и что с этим стало — остаётся в
    // истории навсегда, в отличие от превью.
    const actions = message.actions || [];
    if (actions.length) {
        const box = createElement('ul', { className: 'assistant__actions' });
        for (const a of actions) box.appendChild(actionLine(a));
        bubble.appendChild(box);
    }

    // Откуда в ответе числа: какие данные модель запрашивала. Изменения здесь
    // не повторяются — они уже перечислены выше.
    const tools = [...new Set(message.tools_used || [])].filter(t => t in TOOL_LABELS);
    if (tools.length) {
        bubble.appendChild(createElement('div', {
            className: 'assistant__tools',
            title: tools.join(', '),
        }, `Смотрел: ${tools.map(t => TOOL_LABELS[t]).join(' · ')}`));
    }

    const time = formatTime(message.created_at);
    if (time) bubble.appendChild(createElement('span', { className: 'assistant__time' }, time));

    row.appendChild(bubble);
    return row;
}

/** Строчка журнала действий: что предлагалось и чем кончилось. */
function actionLine(action) {
    const states = {
        done: { icon: Icons.checkCircle, prefix: '' },
        cancelled: { icon: Icons.x, prefix: 'Отменено: ' },
        failed: { icon: Icons.alertTriangle, prefix: 'Не выполнено: ' },
    };
    const state = states[action.status] || states.failed;
    const line = createElement('li', { className: `assistant__action-log assistant__action-log--${action.status}` });
    line.appendChild(createElement('span', { className: 'assistant__action-log-icon', innerHTML: state.icon }));
    const text = state.prefix + action.description + (action.error ? ` — ${action.error}` : '');
    line.appendChild(createElement('span', {}, text));
    return line;
}

/**
 * Превью изменения с кнопками. «Да» стоит первой, но фокус не получает:
 * случайный Enter, оставшийся от отправки вопроса, не должен ничего менять в
 * базе.
 */
function actionCard(action, enabled, onChoice) {
    const row = createElement('div', { className: 'assistant__row assistant__row--ai' });
    const card = createElement('div', { className: 'assistant__bubble assistant__bubble--ai assistant__action' });

    card.appendChild(createElement('p', { className: 'assistant__action-title' }, 'Ассистент хочет выполнить:'));
    card.appendChild(createElement('p', { className: 'assistant__action-text' }, action.description));

    if (action.warnings.length) {
        const warn = createElement('ul', { className: 'assistant__action-warnings' });
        for (const w of action.warnings) {
            const li = createElement('li', {});
            li.appendChild(createElement('span', { className: 'assistant__action-warning-icon', innerHTML: Icons.alertTriangle }));
            li.appendChild(createElement('span', {}, w));
            warn.appendChild(li);
        }
        card.appendChild(warn);
    }

    card.appendChild(createElement('p', { className: 'assistant__action-question' }, 'Подтвердить?'));
    const buttons = createElement('div', { className: 'assistant__action-buttons' });
    const yes = createElement('button', { className: 'btn btn--primary btn--sm' }, 'Да');
    const no = createElement('button', { className: 'btn btn--secondary btn--sm' }, 'Отмена');
    // Без связи с Ollama «Да» выполнило бы изменение, а ответить модель уже
    // не смогла бы — пусть сначала вернётся связь.
    yes.disabled = !enabled;
    no.disabled = !enabled;
    yes.addEventListener('click', () => onChoice(true));
    no.addEventListener('click', () => onChoice(false));
    buttons.appendChild(yes);
    buttons.appendChild(no);
    card.appendChild(buttons);

    row.appendChild(card);
    return row;
}

function typingBubble() {
    const row = createElement('div', { className: 'assistant__row assistant__row--ai' });
    const bubble = createElement('div', {
        className: 'assistant__bubble assistant__bubble--ai assistant__typing',
        'aria-label': 'Модель думает',
    });
    for (let i = 0; i < 3; i++) bubble.appendChild(createElement('span', { className: 'assistant__dot' }));
    row.appendChild(bubble);
    return row;
}

/** «14:05» для сегодняшних, «25.09, 14:05» — для остальных. */
function formatTime(value) {
    const date = parseTimestamp(value);
    if (!date) return '';
    const hm = date.toLocaleTimeString('ru-RU', { hour: '2-digit', minute: '2-digit' });
    const today = new Date();
    if (date.toDateString() === today.toDateString()) return hm;
    const dm = date.toLocaleDateString('ru-RU', { day: '2-digit', month: '2-digit' });
    return `${dm}, ${hm}`;
}
