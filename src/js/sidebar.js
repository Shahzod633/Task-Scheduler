// ============================================
// TaskFlow — Сворачиваемый сайдбар («рельс»)
// ============================================
// Сайдбар сворачивается до полосы иконок 56 px. С 12.11 он единственная
// навигация приложения (плавающего дока больше нет) и виден на всех экранах.
//
// Как решается, свёрнут ли он:
//   • узкое окно (уже 1100 px) — свёрнут; кнопка разворачивает его до смены
//     ширины или экрана, но не запоминается;
//   • доска — свёрнут по умолчанию: колонкам нужна ширина, а 264 px сайдбара
//     при окне 1280 px съедали почти целую колонку;
//   • остальные экраны — развёрнут по умолчанию.
// Для доски и для остальных экранов выбор кнопкой запоминается отдельно, в
// localStorage. Это удобство окна, а не настройка: потерять его вместе с
// профилем WebView2 не страшно, и заводить ради него поле в базе незачем
// (тема, в отличие от него, живёт в SQLite — см. theme.js).

import { $ } from './utils.js';

const STORAGE_KEYS = {
    default: 'taskflow.sidebar',
    board: 'taskflow.sidebar.board',
};
const DEFAULT_RAIL = { default: false, board: true };
const narrow = window.matchMedia('(max-width: 1099px)');

let context = 'default';   // 'default' | 'board'
let narrowOverride = null;  // в узком окне: true/false — нажимали кнопку, null — нет

function readChoice() {
    try {
        return localStorage.getItem(STORAGE_KEYS[context]);
    } catch {
        return null;
    }
}

function writeChoice(value) {
    try {
        localStorage.setItem(STORAGE_KEYS[context], value);
    } catch {
        // Хранилище недоступно — выбор просто не переживёт перезапуск.
    }
}

function isRail() {
    if (narrow.matches) return narrowOverride ?? true;
    const choice = readChoice();
    if (choice === 'rail') return true;
    if (choice === 'full') return false;
    return DEFAULT_RAIL[context];
}

/**
 * В рельсе подписей рядом с иконками не видно — они уходят в подсказки.
 * В развёрнутом виде подсказка повторяла бы видимый текст, поэтому её нет.
 */
function syncTooltips(sidebar, rail) {
    for (const el of sidebar.querySelectorAll('[data-rail-label]')) {
        if (rail) el.setAttribute('data-tooltip', el.dataset.railLabel);
        else el.removeAttribute('data-tooltip');
    }
}

function apply() {
    const sidebar = $('#app-sidebar');
    if (!sidebar) return;
    const rail = isRail();
    sidebar.classList.toggle('sidebar--rail', rail);

    const toggle = $('#sidebar-collapse');
    if (toggle) {
        const label = rail ? 'Развернуть панель' : 'Свернуть панель';
        toggle.setAttribute('aria-expanded', String(!rail));
        toggle.setAttribute('aria-label', label);
        toggle.dataset.railLabel = label;
    }
    syncTooltips(sidebar, rail);
}

/**
 * Какой экран открыт — от этого зависит, свёрнут ли сайдбар по умолчанию.
 * Зовётся из `navigateTo` на каждом переходе.
 */
export function setSidebarContext(view) {
    const next = view === 'board' ? 'board' : 'default';
    if (next !== context) {
        context = next;
        narrowOverride = null;
    }
    apply();
}

/** Вызывать один раз, после того как разметка сайдбара на месте. */
export function initSidebarRail() {
    const sidebar = $('#app-sidebar');
    const toggle = $('#sidebar-collapse');
    if (!sidebar || !toggle) return;

    toggle.addEventListener('click', () => {
        if (narrow.matches) narrowOverride = !isRail();
        else writeChoice(isRail() ? 'full' : 'rail');
        apply();
    });
    narrow.addEventListener('change', () => {
        narrowOverride = null;
        apply();
    });

    // Список пространств перерисовывается на каждом переходе — подсказки его
    // пунктам нужно повесить заново. Наблюдается только состав узлов, а
    // меняются атрибуты, так что сам себя наблюдатель не будит.
    const workspaces = $('#sidebar-workspaces');
    if (workspaces) {
        new MutationObserver(() => syncTooltips(sidebar, sidebar.classList.contains('sidebar--rail')))
            .observe(workspaces, { childList: true });
    }

    apply();
}
