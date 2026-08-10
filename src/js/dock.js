// ============================================
// TaskFlow — Floating Dock (Screen Г)
// ============================================

import Icons from './icons.js';
import { createElement, $ } from './utils.js';

let currentDockView = 'board';

/**
 * Render the floating dock
 */
export function renderDock() {
    // Remove existing dock
    const existing = $('#floating-dock');
    if (existing) existing.remove();
    
    const dock = createElement('div', { className: 'dock', id: 'floating-dock' });
    
    // Inbox button
    dock.appendChild(createDockButton('inbox', Icons.inbox, 'Inbox', () => {
        setActiveDockButton('inbox');
    }));
    
    // Planner button
    dock.appendChild(createDockButton('planner', Icons.calendar, 'Планировщик', () => {
        setActiveDockButton('planner');
    }));
    
    // Separator
    dock.appendChild(createElement('div', { className: 'dock__separator' }));
    
    // Board button (active by default)
    dock.appendChild(createDockButton('board', Icons.boards, 'Доска', () => {
        setActiveDockButton('board');
    }, true));
    
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
 * Set active dock button
 */
function setActiveDockButton(viewId) {
    const buttons = document.querySelectorAll('.dock__btn');
    buttons.forEach(btn => btn.classList.remove('dock__btn--active'));
    
    const activeBtn = $(`#dock-btn-${viewId}`);
    if (activeBtn) activeBtn.classList.add('dock__btn--active');
    
    currentDockView = viewId;
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
