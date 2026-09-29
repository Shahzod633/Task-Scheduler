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

import * as api from './api.js';
import Icons from './icons.js';
import { confirmDialog } from './dialog.js';
import { renderMarkdown } from './markdown.js';
import { createElement, $, showToast, parseTimestamp, autoResize } from './utils.js';

/**
 * Вопрос, на который модель ещё думает: `{ workspaceId, text, promise }`.
 *
 * Живёт на уровне модуля, а не страницы: ответ идёт до двух минут, и за это
 * время человек успевает уйти на доску и вернуться. Вернувшись, он должен
 * увидеть свой вопрос и «печатает…», а не пустое поле и соблазн спросить
 * второй раз.
 */
let pending = null;

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

    const busy = () => pending !== null && pending.workspaceId === workspaceId;

    function syncControls() {
        const blocked = !ready || busy();
        input.disabled = !ready;
        sendBtn.disabled = blocked;
        clearBtn.disabled = busy() || history.length === 0;
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
        if (history.length === 0 && !busy()) {
            list.appendChild(createElement('div', { className: 'assistant__empty' },
                createElement('span', { className: 'assistant__empty-icon', innerHTML: Icons.sparkles }),
                createElement('p', {}, 'Здесь пока пусто. Спросите о своих задачах — например, «Какие задачи просрочены?» ' +
                'или «Сколько у меня задач со сроком на этой неделе?»'),
            ));
        }
        for (const message of history) list.appendChild(messageBubble(message));
        if (busy()) {
            list.appendChild(messageBubble({ role: 'user', content: pending.text }));
            list.appendChild(typingBubble());
        }
        list.scrollTop = list.scrollHeight;
        syncControls();
    }

    // ─── Отправка ───
    async function send() {
        const text = input.value.trim();
        if (!text || !ready || busy()) return;

        showError('');
        input.value = '';
        autoResize(input);

        const request = { workspaceId, text, promise: api.ollamaChat(workspaceId, text) };
        pending = request;
        renderList();

        try {
            const saved = await request.promise;
            if (page.isConnected) history.push(...saved);
        } catch (e) {
            // В истории ничего не осталось (бэкенд пишет пару только после
            // ответа), поэтому текст возвращается в поле — переспросить.
            if (page.isConnected) {
                if (!input.value.trim()) input.value = text;
                autoResize(input);
                showError(String(e));
            } else {
                showToast(String(e), 'error');
            }
        } finally {
            if (pending === request) pending = null;
        }

        if (page.isConnected) {
            renderList();
            input.focus();
        }
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
                     'Отменить это нельзя.',
            confirmText: 'Очистить',
            danger: true,
        });
        if (!ok) return;
        try {
            await api.clearChatHistory(workspaceId);
            history = [];
            renderList();
            showToast('История очищена');
        } catch (e) {
            showToast(String(e), 'error');
        }
    });

    // ─── Загрузка ───
    try {
        history = await api.getChatHistory(workspaceId);
    } catch (e) {
        showToast('Не удалось загрузить историю чата', 'error');
    }
    renderList();

    // Вернулись, пока модель ещё думает над прошлым вопросом, — дождаться и
    // перерисовать. Сам `send()` того вызова уже не видит эту страницу.
    if (busy()) {
        pending.promise
            .then(() => api.getChatHistory(workspaceId))
            .then((fresh) => { if (page.isConnected) { history = fresh; renderList(); } })
            .catch(() => {})
            .finally(() => { if (page.isConnected) renderList(); });
    }

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
            return;
        }
        if (!page.isConnected) return;

        if (!models.some(m => m.name === settings.model)) {
            subtitle.textContent = `${settings.model} · не найдена`;
            showNotice(`Модели «${settings.model}» нет в Ollama — её могли удалить. ` +
                       'Выберите другую в Настройках → «ИИ-ассистент».', toSettings);
            return;
        }

        subtitle.textContent = `${settings.model} · локально, через Ollama`;
        notice.hidden = true;
        ready = true;
        syncControls();
        if (!busy()) input.focus();
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

    // Откуда в ответе числа: какие данные модель запрашивала. Без этой строки
    // не отличить ответ по базе от ответа «из головы».
    const tools = [...new Set(message.tools_used || [])];
    if (tools.length) {
        bubble.appendChild(createElement('div', {
            className: 'assistant__tools',
            title: tools.join(', '),
        }, `Смотрел: ${tools.map(t => TOOL_LABELS[t] || t).join(' · ')}`));
    }

    const time = formatTime(message.created_at);
    if (time) bubble.appendChild(createElement('span', { className: 'assistant__time' }, time));

    row.appendChild(bubble);
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
