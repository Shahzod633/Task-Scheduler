// ============================================
// TaskFlow — Floating Dock (Screen Г)
// ============================================

import Icons from './icons.js';
import { createElement, $ } from './utils.js';

/**
 * Render the floating dock.
 * @param {string} activeView - which dock-driven view is currently active: 'board' | 'inbox' | 'planner'
 */
export function renderDock(activeView = 'board') {
    // Remove existing dock
    const existing = $('#floating-dock');
    if (existing) existing.remove();

    const dock = createElement('div', { className: 'dock', id: 'floating-dock' });

    // Inbox button
    dock.appendChild(createDockButton('inbox', Icons.inbox, 'Inbox', () => {
        window.dispatchEvent(new CustomEvent('navigate', { detail: { view: 'inbox' } }));
    }, activeView === 'inbox'));

    // Planner button
    dock.appendChild(createDockButton('planner', Icons.calendar, 'Планировщик', () => {
        window.dispatchEvent(new CustomEvent('navigate', { detail: { view: 'planner' } }));
    }, activeView === 'planner'));

    // Separator
    dock.appendChild(createElement('div', { className: 'dock__separator' }));

    // Board button
    dock.appendChild(createDockButton('board', Icons.boards, 'Доска', () => {
        window.dispatchEvent(new CustomEvent('navigate', { detail: { view: 'board' } }));
    }, activeView === 'board'));

    // Separator
    dock.appendChild(createElement('div', { className: 'dock__separator' }));

    // Switch board button
    dock.appendChild(createDockButton('switch', Icons.switchBoard, 'Выбрать другую доску', () => {
        window.dispatchEvent(new CustomEvent('navigate', { detail: { view: 'hub' } }));
    }));

    document.body.appendChild(dock);
}

/**
 * Create a dock button
 */
function createDockButton(id, icon, label, onClick, isActive = false) {
    const btn = createElement('button', {
        className: `dock__btn ${isActive ? 'dock__btn--active' : ''}`,
        id: `dock-btn-${id}`,
        innerHTML: `${icon} <span>${label}</span>`,
        'data-tooltip': label
    });
    btn.addEventListener('click', onClick);
    return btn;
}

/**
 * Show dock
 */
export function showDock() {
    const dock = $('#floating-dock');
    if (dock) dock.classList.remove('dock--hidden');
}

/**
 * Hide dock
 */
export function hideDock() {
    const dock = $('#floating-dock');
    if (dock) dock.classList.add('dock--hidden');
}

/**
 * Remove dock from DOM
 */
export function removeDock() {
    const dock = $('#floating-dock');
    if (dock) dock.remove();
}
