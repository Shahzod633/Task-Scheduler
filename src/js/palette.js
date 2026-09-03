// ============================================
// TaskFlow — Ctrl+K: поиск по всему пространству
// ============================================
//
// Данные берутся одним вызовом `list_all_cards_in_workspace` — того самого,
// на котором живёт экран «Список», — и фильтруются здесь, в браузере. Своего
// запроса у поиска нет намеренно: пространство целиком помещается в память
// (это личный планировщик, а не база на миллион строк), а искать по уже
// загруженному списку быстрее, чем ходить в базу на каждую букву.
//
// Обратная сторона: список загружается заново на каждое открытие палитры.
// Так и задумано — карточка, созданная минуту назад, должна находиться.

import * as api from './api.js';
import Icons from './icons.js';
import { createElement, $, showToast, escapeHtml } from './utils.js';
import { showCardEditModal } from './board.js';

/// Сколько строк каждого вида показывать. Ограничение не косметическое:
/// палитра открывается по горячей клавише поверх работы, и список на две сотни
/// строк в ней бесполезен — нужное всё равно ищут, дописывая запрос.
const MAX_BOARDS = 6;
const MAX_CARDS = 20;

let overlay = null;

/** Открыта ли палитра сейчас — чтобы Ctrl+K работал и на закрытие. */
export function isPaletteOpen() {
    return overlay !== null;
}

export function closePalette() {
    if (!overlay) return;
    overlay.remove();
    overlay = null;
}

/**
 * Открывает палитру поиска для указанного пространства.
 *
 * `workspaceId` приходит снаружи: палитра не должна знать, какое пространство
 * сейчас открыто, — это состояние маршрутизатора.
 */
export async function openPalette(workspaceId) {
    if (overlay) {
        closePalette();
        return;
    }
    if (!workspaceId) {
        showToast('Сначала откройте пространство', 'error');
        return;
    }

    overlay = createElement('div', { className: 'palette-overlay' });
    const box = createElement('div', { className: 'palette' });

    const searchRow = createElement('div', { className: 'palette__search' });
    searchRow.appendChild(createElement('span', { className: 'palette__search-icon', innerHTML: Icons.search }));
    const input = createElement('input', {
        className: 'palette__input',
        type: 'text',
        placeholder: 'Поиск по доскам и карточкам',
        autocomplete: 'off',
    });
    searchRow.appendChild(input);
    searchRow.appendChild(createElement('kbd', { className: 'palette__hint' }, 'Esc'));
    box.appendChild(searchRow);

    const results = createElement('div', { className: 'palette__results' });
    box.appendChild(results);

    overlay.appendChild(box);
    document.body.appendChild(overlay);
    input.focus();

    // Пока данные едут, палитра уже открыта и в неё уже печатают. Показать
    // пустой список было бы враньём — «ничего не нашлось» и «ещё не искали»
    // выглядят одинаково, а означают разное.
    results.appendChild(createElement('div', { className: 'palette__empty' }, 'Загружаю…'));

    let cards = [];
    let boards = [];
    try {
        const data = await api.listAllCardsInWorkspace(workspaceId);
        cards = data.cards || [];
        // Служебная доска Inbox скрыта от человека везде — значит и здесь.
        // Её карточки при этом настоящие и в поиске остаются.
        boards = (data.boards || []).filter(b => !b.is_system);
    } catch (e) {
        results.innerHTML = '';
        results.appendChild(createElement('div', { className: 'palette__empty' }, 'Не удалось загрузить данные пространства'));
        return;
    }

    // Палитру могли закрыть, пока данные ехали.
    if (!overlay) return;

    let items = [];
    let active = 0;

    const render = () => {
        const query = input.value.trim();
        items = search(query, boards, cards);
        active = 0;
        results.innerHTML = '';

        if (!items.length) {
            results.appendChild(createElement('div', { className: 'palette__empty' },
                query ? `Ничего не найдено по запросу «${query}»` : 'В этом пространстве пока пусто'));
            return;
        }

        let lastKind = null;
        for (const [index, item] of items.entries()) {
            if (item.kind !== lastKind) {
                results.appendChild(createElement('div', { className: 'palette__group' },
                    item.kind === 'board' ? 'Доски' : 'Карточки'));
                lastKind = item.kind;
            }
            results.appendChild(renderRow(item, index, query, () => {
                active = index;
                highlight();
            }, () => choose(item)));
        }
        highlight();
    };

    const highlight = () => {
        const rows = results.querySelectorAll('.palette__row');
        rows.forEach((row, i) => row.classList.toggle('palette__row--active', i === active));
        rows[active]?.scrollIntoView({ block: 'nearest' });
    };

    const choose = (item) => {
        closePalette();
        if (item.kind === 'board') {
            window.dispatchEvent(new CustomEvent('navigate', { detail: { view: 'board', boardId: item.board.id } }));
            return;
        }
        // Карточка открывается окном правки прямо поверх текущего экрана, а не
        // через переход на её доску: человек искал карточку, а не доску, и
        // лишний переход отбросил бы его с того места, где он работал.
        showCardEditModal(item.card, { onChange: () => {} });
    };

    input.addEventListener('input', render);

    input.addEventListener('keydown', (e) => {
        if (e.key === 'Escape') {
            e.preventDefault();
            closePalette();
        } else if (e.key === 'ArrowDown') {
            e.preventDefault();
            if (items.length) { active = (active + 1) % items.length; highlight(); }
        } else if (e.key === 'ArrowUp') {
            e.preventDefault();
            if (items.length) { active = (active - 1 + items.length) % items.length; highlight(); }
        } else if (e.key === 'Enter') {
            e.preventDefault();
            if (items[active]) choose(items[active]);
        }
    });

    // Щелчок мимо палитры закрывает её; щелчок внутри — нет.
    overlay.addEventListener('mousedown', (e) => {
        if (e.target === overlay) closePalette();
    });

    render();
}

/**
 * Что показывать по запросу.
 *
 * Пустой запрос — не «ничего не нашлось», а «ещё не искали»: показываем доски,
 * чтобы палитра работала ещё и как быстрый переход между ними.
 */
function search(query, boards, cards) {
    const needle = query.toLowerCase();

    if (!needle) {
        return boards.slice(0, MAX_BOARDS).map(board => ({ kind: 'board', board, score: 0 }));
    }

    const boardHits = boards
        .map(board => ({ kind: 'board', board, score: score(board.name, needle) }))
        .filter(hit => hit.score > 0)
        .sort((a, b) => b.score - a.score)
        .slice(0, MAX_BOARDS);

    const cardHits = cards
        .map(card => {
            // Название весит больше описания: человек помнит, как назвал
            // задачу, а совпадение в длинном описании чаще случайное.
            const titleScore = score(card.title, needle);
            const descScore = score(card.description || '', needle) * 0.4;
            return { kind: 'card', card, score: Math.max(titleScore, descScore) };
        })
        .filter(hit => hit.score > 0)
        .sort((a, b) => b.score - a.score)
        .slice(0, MAX_CARDS);

    return [...boardHits, ...cardHits];
}

/**
 * Насколько хорошо строка отвечает запросу. 0 — не отвечает вовсе.
 *
 * Ранжирование грубое и намеренно понятное: совпадение с начала строки выше
 * совпадения с начала слова, а то — выше совпадения в середине. Нечёткий поиск
 * (когда «упрвлн» находит «Управление») здесь был бы лишним: в личном
 * пространстве десятки досок, а не тысячи, и точный ввод не мучителен.
 */
function score(haystack, needle) {
    const text = haystack.toLowerCase();
    const at = text.indexOf(needle);
    if (at < 0) return 0;
    if (at === 0) return 3;
    // Начало слова: пробел, дефис или скобка перед совпадением.
    if (/[\s\-—(«"]/.test(text[at - 1])) return 2;
    return 1;
}

function renderRow(item, index, query, onHover, onClick) {
    const row = createElement('div', { className: 'palette__row' });
    row.dataset.index = String(index);

    if (item.kind === 'board') {
        row.appendChild(createElement('span', { className: 'palette__row-icon', innerHTML: Icons.boards }));
        const text = createElement('div', { className: 'palette__row-text' });
        text.appendChild(createElement('div', { className: 'palette__row-title', innerHTML: mark(item.board.name, query) }));
        text.appendChild(createElement('div', { className: 'palette__row-meta' }, 'Доска'));
        row.appendChild(text);
    } else {
        const card = item.card;
        row.appendChild(createElement('span', { className: 'palette__row-icon', innerHTML: Icons.list }));
        const text = createElement('div', { className: 'palette__row-text' });
        text.appendChild(createElement('div', { className: 'palette__row-title', innerHTML: mark(card.title, query) }));
        // Где лежит карточка — половина ответа на вопрос «та ли это задача»:
        // одинаково названные карточки на разных досках иначе неразличимы.
        text.appendChild(createElement('div', { className: 'palette__row-meta' },
            `${card.board_name} · ${card.column_name}`));
        row.appendChild(text);
        if (card.is_mistake) {
            row.appendChild(createElement('span', { className: 'palette__row-badge' }, 'ошибка'));
        }
    }

    row.addEventListener('mouseenter', onHover);
    row.addEventListener('click', onClick);
    return row;
}

/**
 * Подсвечивает совпадение в тексте.
 *
 * Совпадение ищется в **исходном** тексте, а экранируются уже три готовых
 * куска. Наоборот — искать в экранированном — нельзя: у карточки с названием
 * `a < b` экранированный вид это `a &lt; b`, и запрос «l» попал бы внутрь
 * `&lt;`, разрезав сущность пополам и превратив строку в мусор.
 *
 * Экранирование при этом обязательно: название пишет человек, и
 * `<img onerror=…>` в нём — обычный текст, а не разметка.
 */
function mark(text, query) {
    const needle = query.trim();
    if (!needle) return escapeHtml(text);

    const at = text.toLowerCase().indexOf(needle.toLowerCase());
    if (at < 0) return escapeHtml(text);

    return escapeHtml(text.slice(0, at))
        + '<mark class="palette__mark">' + escapeHtml(text.slice(at, at + needle.length)) + '</mark>'
        + escapeHtml(text.slice(at + needle.length));
}
