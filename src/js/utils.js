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
export function formatDate(dateStr) {
    if (!dateStr) return '';
    const date = new Date(dateStr);
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
    if (!dateStr) return '';
    const date = new Date(dateStr);
    if (Number.isNaN(date.getTime())) return '';

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
 * Check if a date is overdue
 */
export function isOverdue(dateStr) {
    if (!dateStr) return false;
    return new Date(dateStr) < new Date();
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
        const d = new Date(now);
        d.setDate(d.getDate() - i);
        days.push(d.toISOString().slice(0, 10));
    }
    return days;
}
