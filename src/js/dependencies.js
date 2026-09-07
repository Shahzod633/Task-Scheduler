// ============================================
// TaskFlow — Зависимости между задачами
// ============================================
//
// Ребро «A блокирует B» читается как «пока не сделана A, не сделать B».
//
// Заблокированность здесь — **предупреждение, а не запрет**. Это личный
// планировщик: человек сам решил, что одна задача ждёт другую, и он же вправе
// решить, что передумал. Запрет заставил бы его сначала снимать связь, а потом
// делать то, что он и так собирался, — лишний шаг вместо подсказки. Поэтому
// единственное, что делает зависимость при закрытии задачи, — добавляет строку
// в тот же вопрос, который и так задаётся про финальную колонку.
//
// Модуль сознательно не импортирует `board.js`: тот импортирует этот, а
// открытие связанной карточки приходит колбэком снаружи. Иначе модули
// замкнулись бы в кольцо.

import * as api from './api.js';
import Icons from './icons.js';
import { confirmDialog } from './dialog.js';
import { searchCards, markMatch } from './filters.js';
import { createElement, showToast, pluralize } from './utils.js';

// ─── Вопрос при переносе в финальную колонку ─────────────────────────────

/**
 * Единственная дверь к вопросу «перенести в финальную колонку?».
 *
 * До появления зависимостей этот вопрос был выписан трижды — на доске, в
 * «Списке» и в Inbox, — тремя почти одинаковыми кусками. Строку про блокировку
 * пришлось бы дописывать в каждый, и они разошлись бы при первой же правке.
 *
 * @param {object}  opts
 * @param {number}  opts.cardId
 * @param {string} [opts.columnName] название целевой колонки, если известно
 * @param {string} [opts.title]      заголовок окна
 * @returns {Promise<boolean>}
 */
export async function confirmFinalColumnMove({ cardId, columnName = '', title = 'Перенести в финальную колонку?' }) {
    const base = columnName
        ? `Перемещение в «${columnName}» необратимо — задачу нельзя будет вернуть обратно.`
        : 'Перемещение в финальную колонку необратимо — карточку нельзя будет вернуть обратно.';

    const warning = await blockerWarning(cardId);

    return confirmDialog({
        title,
        // Перенос строки виден благодаря `white-space: pre-line` у
        // `.confirm__message`: предупреждение — отдельная мысль, и в одном
        // абзаце с «перемещение необратимо» оно теряется.
        message: warning ? `${base}\n\n${warning}` : base,
        confirmText: 'Подтвердить',
        danger: true,
    });
}

/**
 * Строка про незавершённые блокирующие задачи — или пустая строка.
 *
 * Отказ бэкенда здесь намеренно проглатывается: не сумев прочитать
 * зависимости, окно должно спросить про финальную колонку как раньше, а не
 * упасть и не пропустить вопрос вовсе.
 */
async function blockerWarning(cardId) {
    if (!cardId) return '';

    let open = [];
    try {
        const deps = await api.listCardDependencies(cardId);
        open = (deps.blocked_by || []).filter(d => !d.is_done);
    } catch (e) {
        console.error('Не удалось прочитать зависимости карточки:', e);
        return '';
    }

    if (!open.length) return '';

    if (open.length === 1) {
        return `Эта задача заблокирована задачей «${open[0].title}», которая ещё не завершена. Всё равно закрыть?`;
    }

    const rest = open.length - 1;
    const word = pluralize(rest, ['незавершённой задачей', 'незавершёнными задачами', 'незавершёнными задачами']);
    return `Эта задача заблокирована задачей «${open[0].title}» и ещё ${rest} ${word}. Всё равно закрыть?`;
}

// ─── Блоки в окне карточки ───────────────────────────────────────────────

/**
 * Рисует «Блокирует» и «Заблокировано» в окне карточки.
 *
 * Устроено как `renderChecklist`/`renderComments`: контейнер и карточка
 * снаружи, загрузка асинхронная, окно к этому моменту уже на экране.
 *
 * @param {HTMLElement} container
 * @param {object} cardData            карточка, чьи зависимости показываем
 * @param {object} [opts]
 * @param {number} [opts.workspaceId]  где искать карточки для связи
 * @param {Function} [opts.onChange]   вызывается после любой правки связей
 * @param {Function} [opts.onOpenCard] открыть связанную карточку (из board.js)
 */
export function renderDependencies(container, cardData, opts = {}) {
    const { workspaceId, onChange = () => {}, onOpenCard } = opts;

    const wrap = createElement('div', { className: 'card-deps' });
    container.appendChild(wrap);

    const blocks = [
        {
            key: 'blocking',
            label: 'Блокирует',
            hint: 'Эти задачи ждут текущую',
            empty: 'Никого не блокирует',
            pickTitle: 'Какая задача ждёт эту?',
            // Выбранная карточка становится заблокированной, текущая — блокирующей.
            link: (pickedId) => ({ blocker: cardData.id, blocked: pickedId }),
        },
        {
            key: 'blocked_by',
            label: 'Заблокировано',
            hint: 'Текущая задача ждёт эти',
            empty: 'Ничем не заблокирована',
            pickTitle: 'Какая задача блокирует эту?',
            link: (pickedId) => ({ blocker: pickedId, blocked: cardData.id }),
        },
    ];

    const paint = (deps) => {
        wrap.innerHTML = '';
        for (const block of blocks) {
            wrap.appendChild(createBlock(block, deps[block.key] || []));
        }
    };

    const reload = async () => {
        try {
            paint(await api.listCardDependencies(cardData.id));
        } catch (e) {
            console.error('Не удалось загрузить зависимости:', e);
            wrap.innerHTML = '';
            wrap.appendChild(createElement('p', { className: 'card-deps__error' },
                'Не удалось загрузить зависимости'));
        }
    };

    function createBlock(block, items) {
        const el = createElement('div', { className: 'card-deps__block' });

        const head = createElement('div', { className: 'card-deps__head' });
        head.appendChild(createElement('span', { className: 'card-deps__label' }, block.label));
        head.appendChild(createElement('span', {
            className: 'card-deps__count',
            'data-tooltip': block.hint,
        }, String(items.length)));

        const addBtn = createElement('button', {
            className: 'card-deps__add',
            type: 'button',
        }, '+ Добавить');
        addBtn.addEventListener('click', () => {
            if (!workspaceId) {
                showToast('Не удалось определить пространство', 'error');
                return;
            }
            openCardPicker({
                workspaceId,
                excludeCardId: cardData.id,
                title: block.pickTitle,
                onPick: async (picked) => {
                    const { blocker, blocked } = block.link(picked.id);
                    try {
                        await api.addCardDependency(blocker, blocked);
                        await reload();
                        onChange();
                        showToast('Зависимость добавлена');
                    } catch (e) {
                        // Текст отказа приходит с бэкенда и объясняет причину:
                        // цикл, дубль и ссылка на себя выглядят одинаково
                        // «не получилось», а означают разное.
                        showToast(typeof e === 'string' ? e : 'Не удалось добавить зависимость', 'error');
                    }
                },
            });
        });
        head.appendChild(addBtn);
        el.appendChild(head);

        if (!items.length) {
            el.appendChild(createElement('p', { className: 'card-deps__empty' }, block.empty));
            return el;
        }

        const list = createElement('div', { className: 'card-deps__list' });
        for (const item of items) list.appendChild(createRow(item));
        el.appendChild(list);
        return el;
    }

    function createRow(item) {
        const row = createElement('div', {
            className: `card-deps__row ${item.is_done ? 'card-deps__row--done' : ''}`,
        });

        row.appendChild(createElement('span', {
            className: 'card-deps__row-icon',
            innerHTML: item.is_done ? Icons.checkCircle : Icons.link,
            'data-tooltip': item.is_done ? 'Задача завершена' : 'Задача ещё не завершена',
        }));

        const text = createElement('div', { className: 'card-deps__row-text' });
        const title = createElement('button', {
            className: 'card-deps__row-title',
            type: 'button',
        }, item.title);
        title.addEventListener('click', () => openLinked(item));
        text.appendChild(title);
        text.appendChild(createElement('div', { className: 'card-deps__row-where' },
            `${item.board_name} · ${item.column_name}`));
        row.appendChild(text);

        const removeBtn = createElement('button', {
            className: 'card-deps__row-remove',
            type: 'button',
            innerHTML: Icons.x,
            'data-tooltip': 'Убрать связь',
        });
        removeBtn.addEventListener('click', async () => {
            try {
                await api.removeCardDependency(item.link_id);
                await reload();
                onChange();
            } catch (e) {
                showToast('Не удалось убрать связь', 'error');
            }
        });
        row.appendChild(removeBtn);

        return row;
    }

    /**
     * Переход к связанной карточке: сначала на её доску, потом окно правки.
     *
     * Доска нужна потому, что связанная задача часто лежит на другой, и одно
     * окно поверх чужого канбана не отвечает на вопрос «а где она вообще».
     *
     * Карточка перечитывается целиком, а не собирается из строки списка:
     * в `DependencyCard` лежат только название и место, а окно правки
     * сохраняет **все** поля — открытое с пустым описанием, оно затёрло бы
     * настоящее по кнопке «Сохранить».
     */
    async function openLinked(item) {
        window.dispatchEvent(new CustomEvent('navigate', {
            detail: { view: 'board', boardId: item.board_id },
        }));

        if (!onOpenCard || !workspaceId) return;

        try {
            const data = await api.listAllCardsInWorkspace(workspaceId);
            const full = (data.cards || []).find(c => c.id === item.card_id);
            // Не нашлась — значит, переход на доску уже случился и человек
            // увидит её там сам; выдумывать карточку ради окна не станем.
            if (full) onOpenCard(full);
        } catch (e) {
            console.error('Не удалось открыть связанную карточку:', e);
        }
    }

    reload();
}

// ─── Выбор карточки ──────────────────────────────────────────────────────

/**
 * Поиск карточки по всему пространству.
 *
 * Разметка и стили — палитры Ctrl+K (`.palette-*`), ранжирование — общее из
 * `filters.js`. Своего поиска здесь нет: два по-разному ищущих окна в одном
 * приложении — это два разных ответа на один вопрос.
 */
function openCardPicker({ workspaceId, excludeCardId, title, onPick }) {
    const overlay = createElement('div', { className: 'palette-overlay' });
    const box = createElement('div', { className: 'palette' });

    const searchRow = createElement('div', { className: 'palette__search' });
    searchRow.appendChild(createElement('span', { className: 'palette__search-icon', innerHTML: Icons.search }));
    const input = createElement('input', {
        className: 'palette__input',
        type: 'text',
        placeholder: title,
        autocomplete: 'off',
    });
    searchRow.appendChild(input);
    searchRow.appendChild(createElement('kbd', { className: 'palette__hint' }, 'Esc'));
    box.appendChild(searchRow);

    const results = createElement('div', { className: 'palette__results' });
    box.appendChild(results);
    overlay.appendChild(box);

    let closed = false;
    const close = () => {
        if (closed) return;
        closed = true;
        document.removeEventListener('keydown', onKey, true);
        overlay.remove();
    };

    // Перехват в фазе погружения: окно выбора лежит поверх окна карточки, а
    // общий обработчик Escape (`initModalEscape`) закрывает верхнюю
    // `.modal-overlay` — то есть карточку под нами, оставив выбор висеть.
    function onKey(e) {
        if (e.key !== 'Escape') return;
        e.preventDefault();
        e.stopPropagation();
        close();
    }
    document.addEventListener('keydown', onKey, true);

    overlay.addEventListener('click', (e) => { if (e.target === overlay) close(); });

    document.body.appendChild(overlay);
    input.focus();

    results.appendChild(createElement('div', { className: 'palette__empty' }, 'Загружаю…'));

    let cards = [];
    let items = [];
    let active = 0;

    const highlight = () => {
        const rows = results.querySelectorAll('.palette__row');
        rows.forEach((row, i) => row.classList.toggle('palette__row--active', i === active));
        rows[active]?.scrollIntoView({ block: 'nearest' });
    };

    const render = () => {
        const query = input.value.trim();
        items = searchCards(cards, query);
        active = 0;
        results.innerHTML = '';

        if (!items.length) {
            results.appendChild(createElement('div', { className: 'palette__empty' },
                query ? `Ничего не найдено по запросу «${query}»` : 'Начните вводить название задачи'));
            return;
        }

        for (const [index, hit] of items.entries()) {
            const card = hit.card;
            const row = createElement('div', { className: 'palette__row' });
            row.appendChild(createElement('span', { className: 'palette__row-icon', innerHTML: Icons.list }));

            const text = createElement('div', { className: 'palette__row-text' });
            text.appendChild(createElement('div', {
                className: 'palette__row-title',
                innerHTML: markMatch(card.title, query),
            }));
            // Где лежит карточка — половина ответа на вопрос «та ли это задача»:
            // одинаково названные задачи на разных досках иначе неразличимы.
            text.appendChild(createElement('div', { className: 'palette__row-meta' },
                `${card.board_name} · ${card.column_name}`));
            row.appendChild(text);

            row.addEventListener('mouseenter', () => { active = index; highlight(); });
            row.addEventListener('click', () => { close(); onPick(card); });
            results.appendChild(row);
        }
        highlight();
    };

    input.addEventListener('input', render);
    input.addEventListener('keydown', (e) => {
        if (e.key === 'ArrowDown') {
            e.preventDefault();
            if (items.length) { active = (active + 1) % items.length; highlight(); }
        } else if (e.key === 'ArrowUp') {
            e.preventDefault();
            if (items.length) { active = (active - 1 + items.length) % items.length; highlight(); }
        } else if (e.key === 'Enter') {
            e.preventDefault();
            const hit = items[active];
            if (hit) { close(); onPick(hit.card); }
        }
    });

    api.listAllCardsInWorkspace(workspaceId).then((data) => {
        if (closed) return;
        // Сама карточка из списка убрана: связь с самой собой бэкенд всё равно
        // отклонит, и предлагать её значит подсовывать заведомо неверный выбор.
        cards = (data.cards || []).filter(c => c.id !== excludeCardId);
        render();
    }).catch((e) => {
        if (closed) return;
        console.error('Не удалось загрузить карточки пространства:', e);
        results.innerHTML = '';
        results.appendChild(createElement('div', { className: 'palette__empty' },
            'Не удалось загрузить карточки пространства'));
    });
}
