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
            Object.assign(el.style, value);
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
const gradients = [
    'linear-gradient(135deg, #667eea 0%, #764ba2 100%)',
    'linear-gradient(135deg, #f093fb 0%, #f5576c 100%)',
    'linear-gradient(135deg, #4facfe 0%, #00f2fe 100%)',
    'linear-gradient(135deg, #43e97b 0%, #38f9d7 100%)',
    'linear-gradient(135deg, #fa709a 0%, #fee140 100%)',
    'linear-gradient(135deg, #a18cd1 0%, #fbc2eb 100%)',
    'linear-gradient(135deg, #ffecd2 0%, #fcb69f 100%)',
    'linear-gradient(135deg, #89f7fe 0%, #66a6ff 100%)',
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
