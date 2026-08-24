// ============================================
// TaskFlow — Utility Functions
// ============================================

/**
 * Create a DOM element with optional attributes and children
 */
export function createElement(tag, attrs = {}, ...children) {
    const el = document.createElement(tag);
    
    for (const [key, value] of Object.entries(attrs)) {
        if (key === 'className') {
            el.className = value;
        } else if (key === 'innerHTML') {
            el.innerHTML = value;
        } else if (key === 'style' && typeof value === 'object') {
            for (const [prop, val] of Object.entries(value)) {
                // Кастомные свойства (--foo) невидимы для присваивания через
                // style.foo — их принимает только setProperty.
                if (prop.startsWith('--')) {
                    el.style.setProperty(prop, val);
                } else {
                    el.style[prop] = val;
                }
            }
        } else if (key.startsWith('on') && typeof value === 'function') {
            el.addEventListener(key.slice(2).toLowerCase(), value);
        } else if (key === 'dataset') {
            for (const [dk, dv] of Object.entries(value)) {
                el.dataset[dk] = dv;
            }
        } else {
            el.setAttribute(key, value);
        }
    }
    
    for (const child of children) {
        if (typeof child === 'string') {
            el.appendChild(document.createTextNode(child));
        } else if (child instanceof Node) {
            el.appendChild(child);
        }
    }
    
    return el;
}

/**
 * Shorthand query selectors
 */
export function $(selector, parent = document) {
    return parent.querySelector(selector);
}

export function $$(selector, parent = document) {
    return [...parent.querySelectorAll(selector)];
}

/**
 * Format date string
 */
// ─── Даты и время ───
//
// В базе живут два РАЗНЫХ вида дат, и путать их нельзя:
//
//   1. Отметки времени (`created_at`, `mistake_marked_at`, `opened_at`) пишутся
//      SQLite'ом через `datetime('now')` — это **UTC** в формате
//      "YYYY-MM-DD HH:MM:SS", без какого-либо указания зоны.
//   2. Срок (`due_date`) приходит из `<input type="date">` — это **календарная
//      дата** "YYYY-MM-DD" без времени и без зоны. «25 августа» означает
//      25 августа там, где сидит пользователь.
//
// `new Date(строка)` разбирает и то и другое неправильно: строку с временем и
// без зоны он считает МЕСТНОЙ (отметка уезжает на величину часового пояса —
// у нас на 5 часов), а строку из одной даты, наоборот, считает полуночью UTC
// (в зонах западнее Гринвича это даёт сдвиг на сутки назад).
//
// Поэтому разбор идёт только через функции ниже. Прямой `new Date(строка_из_БД)`
// в коде приложения быть не должен.

/** True для строки вида "YYYY-MM-DD" без части времени. */
function isDateOnly(value) {
    return /^\d{4}-\d{2}-\d{2}$/.test(String(value).trim());
}

/**
 * Отметка времени из базы (UTC) → `Date`.
 * Строку с явной зоной (`...Z`, `...+05:00`) принимает как есть.
 */
export function parseTimestamp(value) {
    if (!value) return null;
    const str = String(value).trim();

    if (/([zZ]|[+-]\d{2}:?\d{2})$/.test(str)) {
        const explicit = new Date(str);
        return Number.isNaN(explicit.getTime()) ? null : explicit;
    }

    const m = str.match(/^(\d{4})-(\d{2})-(\d{2})[ T](\d{2}):(\d{2})(?::(\d{2}))?/);
    if (m) {
        return new Date(Date.UTC(+m[1], +m[2] - 1, +m[3], +m[4], +m[5], +(m[6] || 0)));
    }

    // Одна дата без времени — это не момент, а день; берём местную полночь.
    if (isDateOnly(str)) {
        const [y, mo, d] = str.split('-').map(Number);
        return new Date(y, mo - 1, d);
    }

    const fallback = new Date(str);
    return Number.isNaN(fallback.getTime()) ? null : fallback;
}

/**
 * Срок карточки → `Date` в местной полуночи этого дня.
 * Если в значении есть время, оно разбирается как отметка времени.
 */
export function parseDueDate(value) {
    if (!value) return null;
    const str = String(value).trim();
    if (isDateOnly(str)) {
        const [y, mo, d] = str.split('-').map(Number);
        return new Date(y, mo - 1, d);
    }
    return parseTimestamp(str);
}

/**
 * `Date` → "YYYY-MM-DD" по **местному** календарю.
 *
 * Именно этим следует получать ключ дня, а не `toISOString().slice(0, 10)`:
 * тот переводит время в UTC, и с полуночи до конца смещения зоны выдаёт
 * вчерашнюю дату.
 */
export function toDateKey(date) {
    if (!date) return '';
    const pad = (n) => String(n).padStart(2, '0');
    return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

/** Сегодняшняя дата в местном календаре, "YYYY-MM-DD". */
export function todayKey() {
    return toDateKey(new Date());
}

export function formatDate(dateStr) {
    if (!dateStr) return '';
    const date = parseTimestamp(dateStr);
    if (!date) return '';
    const now = new Date();
    const diff = now - date;

    if (diff < 60000) return 'только что';
    if (diff < 3600000) return `${Math.floor(diff / 60000)} мин. назад`;
    if (diff < 86400000) return `${Math.floor(diff / 3600000)} ч. назад`;
    
    const options = { day: 'numeric', month: 'short' };
    if (date.getFullYear() !== now.getFullYear()) {
        options.year = 'numeric';
    }
    return date.toLocaleDateString('ru-RU', options);
}

/**
 * Format a deadline.
 *
 * `formatDate` above measures how long ago something happened, which is right
 * for a notification and wrong for a due date: a deadline is in the future, so
 * its difference is negative, falls into the first branch and prints
 * "только что". Deadlines get an absolute date instead, with today and tomorrow
 * spelled out because those are the two that matter at a glance.
 */
export function formatDueDate(dateStr) {
    const date = parseDueDate(dateStr);
    if (!date) return '';

    const atMidnight = (d) => new Date(d.getFullYear(), d.getMonth(), d.getDate());
    const days = Math.round((atMidnight(date) - atMidnight(new Date())) / 86400000);

    if (days === 0) return 'Сегодня';
    if (days === 1) return 'Завтра';
    if (days === -1) return 'Вчера';

    const options = { day: 'numeric', month: 'short' };
    if (date.getFullYear() !== new Date().getFullYear()) options.year = 'numeric';
    return date.toLocaleDateString('ru-RU', options);
}

/**
 * Просрочен ли срок.
 *
 * Срок «25 августа» истекает в **конце** 25 августа, а не в его начале:
 * задача, которую нужно сделать сегодня, ещё не просрочена. Раньше сравнение
 * шло с полуночью UTC, из-за чего карточка со сроком «сегодня» краснела
 * с раннего утра.
 */
export function isOverdue(dateStr) {
    const date = parseDueDate(dateStr);
    if (!date) return false;

    if (isDateOnly(dateStr)) {
        const endOfDay = new Date(date.getFullYear(), date.getMonth(), date.getDate(), 23, 59, 59, 999);
        return endOfDay < new Date();
    }
    return date < new Date();
}

/**
 * Debounce function
 */
export function debounce(fn, delay = 300) {
    let timer;
    return function(...args) {
        clearTimeout(timer);
        timer = setTimeout(() => fn.apply(this, args), delay);
    };
}

/**
 * Generate a random gradient from the predefined set
 */
// Обложки досок. Значения совпадают с --gradient-1…8 в css/variables.css;
// хранятся строкой в БД, поэтому у уже созданных досок фон не меняется.
const gradients = [
    'linear-gradient(135deg, #6c5cff 0%, #b45cff 100%)',
    'linear-gradient(135deg, #f0567a 0%, #ff9f45 100%)',
    'linear-gradient(135deg, #2f8bff 0%, #45e0e0 100%)',
    'linear-gradient(135deg, #12b981 0%, #7ee7a3 100%)',
    'linear-gradient(135deg, #ff5f8f 0%, #ffc94d 100%)',
    'linear-gradient(135deg, #8b5cf6 0%, #ec7fd0 100%)',
    'linear-gradient(135deg, #f97362 0%, #ffb37a 100%)',
    'linear-gradient(135deg, #3b6dff 0%, #7bb8ff 100%)',
];

export function getRandomGradient() {
    return gradients[Math.floor(Math.random() * gradients.length)];
}

export function getGradients() {
    return gradients;
}

/**
 * Show a toast notification
 */
export function showToast(message, type = 'success') {
    let container = document.getElementById('toast-container');
    if (!container) {
        container = createElement('div', { id: 'toast-container', className: 'toast-container' });
        document.body.appendChild(container);
    }
    
    const toast = createElement('div', { className: `toast toast--${type}` }, message);
    container.appendChild(toast);
    
    setTimeout(() => {
        toast.classList.add('toast--exit');
        setTimeout(() => toast.remove(), 300);
    }, 3000);
}

/**
 * Каскадное появление элемента.
 *
 * Классы и CSS-переменная снимаются сразу после проигрывания: пока висит
 * `.stagger-item`, у элемента остаётся анимационный `transform`, а он
 * перебивает инлайновые стили, которыми Sortable.js двигает карточки и
 * колонки при перетаскивании. Очистка гарантирует, что drag-and-drop не
 * столкнётся с анимацией входа.
 *
 * @param {HTMLElement} el
 * @param {number} index - позиция элемента; задаёт задержку
 * @param {string} [variant] - доп. класс варианта, например 'stagger-item--pop'
 */
export function staggerIn(el, index = 0, variant = '') {
    el.classList.add('stagger-item');
    if (variant) el.classList.add(variant);
    el.style.setProperty('--stagger', index);

    el.addEventListener('animationend', function onEnd(e) {
        // Событие всплывает от потомков — реагируем только на свою анимацию
        if (e.target !== el) return;
        el.removeEventListener('animationend', onEnd);
        el.classList.remove('stagger-item');
        if (variant) el.classList.remove(variant);
        el.style.removeProperty('--stagger');
    });

    return el;
}

/**
 * Picks the right Russian plural form for `n`.
 *
 * @param {number} n
 * @param {[string, string, string]} forms - [1 карточка, 2 карточки, 5 карточек]
 * @example pluralize(3, ['карточка', 'карточки', 'карточек']) // 'карточки'
 */
export function pluralize(n, [one, few, many]) {
    const mod100 = Math.abs(n) % 100;
    // 11–14 all take the "many" form, regardless of their last digit.
    if (mod100 >= 11 && mod100 <= 14) return many;

    const mod10 = mod100 % 10;
    if (mod10 === 1) return one;
    if (mod10 >= 2 && mod10 <= 4) return few;
    return many;
}

/**
 * Auto-resize textarea
 */
export function autoResize(textarea) {
    textarea.style.height = 'auto';
    textarea.style.height = textarea.scrollHeight + 'px';
}

/**
 * Escape HTML to prevent XSS
 */
export function escapeHtml(str) {
    const div = document.createElement('div');
    div.textContent = str;
    return div.innerHTML;
}

/**
 * Returns an array of 'YYYY-MM-DD' date strings for the last `n` days,
 * oldest first, ending today.
 */
export function lastNDays(n) {
    const days = [];
    const now = new Date();
    for (let i = n - 1; i >= 0; i--) {
        // Дни строятся в местном календаре: `toISOString()` здесь давал бы
        // UTC-дату, и последним столбцом графика с полуночи до конца смещения
        // зоны оказывался бы вчерашний день.
        days.push(toDateKey(new Date(now.getFullYear(), now.getMonth(), now.getDate() - i)));
    }
    return days;
}
