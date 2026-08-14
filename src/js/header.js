// ============================================
// TaskFlow — Global Header Interactions
// (notifications, help, profile, workspaces switcher, recent/starred nav)
// ============================================

import * as api from './api.js';
import Icons from './icons.js';
import { openPopover, closePopovers } from './popover.js';
import { createElement, $, $$, showToast, formatDate } from './utils.js';

const APP_VERSION = '1.0.0';

let getWorkspaceId = () => null;

/**
 * Wire up all header interactions. Call once after the layout is rendered.
 * @param {() => number|null} workspaceIdGetter - returns the currently active workspace id
 */
export async function initHeaderInteractions(workspaceIdGetter) {
    getWorkspaceId = workspaceIdGetter;

    $('#btn-notifications').addEventListener('click', (e) => {
        e.stopPropagation();
        toggleNotificationsPanel();
    });

    $('#btn-help').addEventListener('click', (e) => {
        e.stopPropagation();
        showHelpModal();
    });

    $('#user-avatar').addEventListener('click', (e) => {
        e.stopPropagation();
        showProfileModal();
    });

    $('#nav-workspaces').addEventListener('click', (e) => {
        e.stopPropagation();
        toggleWorkspacesPanel(e.currentTarget);
    });

    $('#nav-recent').addEventListener('click', () => {
        window.dispatchEvent(new CustomEvent('navigate', { detail: { view: 'recent' } }));
    });

    $('#nav-starred').addEventListener('click', () => {
        window.dispatchEvent(new CustomEvent('navigate', { detail: { view: 'favorites' } }));
    });

    loadInitialAvatar();
    refreshNotificationBadge();
}

function closeAllPopovers() {
    closePopovers();
}

// ─── Notifications ───

async function toggleNotificationsPanel() {
    const existing = $('#notifications-panel');
    if (existing) { existing.remove(); return; }
    closeAllPopovers();

    const anchor = $('#btn-notifications');
    const panel = createElement('div', { className: 'header-popover', id: 'notifications-panel' });
    panel.appendChild(createElement('div', { className: 'header-popover__title' }, 'Уведомления'));

    openPopover(panel, anchor, { width: 320, align: 'end' });

    try {
        const notifications = await api.getNotifications();

        if (notifications.length === 0) {
            panel.appendChild(createElement('div', { className: 'header-popover__empty' }, 'Нет новых уведомлений'));
        } else {
            const list = createElement('div', { className: 'notification-list' });
            for (const n of notifications) {
                const item = createElement('div', {
                    className: `notification-item ${n.read ? '' : 'notification-item--unread'}`
                });
                item.appendChild(createElement('div', { className: 'notification-item__title' }, n.title));
                if (n.body) {
                    item.appendChild(createElement('div', { className: 'notification-item__body' }, n.body));
                }
                item.appendChild(createElement('div', { className: 'notification-item__time' }, formatDate(n.created_at)));
                list.appendChild(item);
            }
            panel.appendChild(list);
        }

        if (notifications.some(n => !n.read)) {
            await api.markAllNotificationsRead();
            refreshNotificationBadge();
        }
    } catch (e) {
        panel.appendChild(createElement('div', { className: 'header-popover__empty' }, 'Ошибка загрузки уведомлений'));
    }
}

export async function refreshNotificationBadge() {
    try {
        const notifications = await api.getNotifications();
        const unread = notifications.filter(n => !n.read).length;
        const btn = $('#btn-notifications');
        if (!btn) return;
        let badge = $('.notification-badge', btn);
        if (unread > 0 && !badge) {
            btn.appendChild(createElement('span', { className: 'notification-badge' }));
        } else if (unread === 0 && badge) {
            badge.remove();
        }
    } catch (e) {
        // Non-critical — badge just stays as-is
    }
}

// ─── Help modal ───

function showHelpModal() {
    closeAllPopovers();
    const existingOverlay = $('.modal-overlay');
    if (existingOverlay) existingOverlay.remove();

    const overlay = createElement('div', { className: 'modal-overlay' });
    const modal = createElement('div', { className: 'modal' });

    const header = createElement('div', { className: 'modal__header' });
    header.appendChild(createElement('h2', { className: 'modal__title' }, 'Справка'));
    const closeBtn = createElement('button', { className: 'modal__close', innerHTML: Icons.x });
    closeBtn.addEventListener('click', () => overlay.remove());
    header.appendChild(closeBtn);
    modal.appendChild(header);

    const body = createElement('div', { className: 'modal__body' });

    body.appendChild(createElement('h3', { className: 'help-section-title' }, 'Горячие клавиши'));
    const shortcuts = [
        ['Enter', 'Добавить карточку или колонку'],
        ['Shift + Enter', 'Перенос строки в описании карточки'],
        ['Esc', 'Закрыть форму или модальное окно'],
        ['Клик по названию', 'Переименовать доску или колонку'],
    ];
    const list = createElement('div', { className: 'shortcut-list' });
    for (const [key, desc] of shortcuts) {
        const row = createElement('div', { className: 'shortcut-row' });
        row.appendChild(createElement('kbd', { className: 'shortcut-key' }, key));
        row.appendChild(createElement('span', { className: 'shortcut-desc' }, desc));
        list.appendChild(row);
    }
    body.appendChild(list);

    body.appendChild(createElement('h3', { className: 'help-section-title' }, 'О программе'));
    const about = createElement('div', { className: 'about-block' });
    about.appendChild(createElement('div', {}, `TaskFlow v${APP_VERSION}`));
    const link = createElement('a', { href: '#', className: 'about-link' }, 'Репозиторий проекта');
    link.addEventListener('click', (e) => e.preventDefault());
    about.appendChild(link);
    body.appendChild(about);

    modal.appendChild(body);
    overlay.appendChild(modal);
    overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
    document.body.appendChild(overlay);
}

// ─── Profile modal + shared profile form (also used full-page by settings.js) ───

async function loadInitialAvatar() {
    try {
        const profile = await api.getUserProfile();
        updateHeaderAvatar(profile.avatar_initials);
    } catch (e) {
        // keep default avatar text
    }
}

function updateHeaderAvatar(initials) {
    const avatar = $('#user-avatar');
    if (avatar) avatar.textContent = initials;
}

function showProfileModal() {
    closeAllPopovers();
    const existingOverlay = $('.modal-overlay');
    if (existingOverlay) existingOverlay.remove();

    const overlay = createElement('div', { className: 'modal-overlay' });
    const modal = createElement('div', { className: 'modal' });

    const header = createElement('div', { className: 'modal__header' });
    header.appendChild(createElement('h2', { className: 'modal__title' }, 'Профиль'));
    const closeBtn = createElement('button', { className: 'modal__close', innerHTML: Icons.x });
    closeBtn.addEventListener('click', () => overlay.remove());
    header.appendChild(closeBtn);
    modal.appendChild(header);

    const body = createElement('div', { className: 'modal__body' });
    modal.appendChild(body);
    overlay.appendChild(modal);
    overlay.addEventListener('click', (e) => { if (e.target === overlay) overlay.remove(); });
    document.body.appendChild(overlay);

    renderProfileFields(body, { onSaved: () => overlay.remove() });
}

/**
 * Renders the profile edit form (avatar initials, display name, theme toggle,
 * save button) into `container`. Shared by the header modal and the
 * full-page Settings screen.
 */
export async function renderProfileFields(container, opts = {}) {
    let profile;
    try {
        profile = await api.getUserProfile();
    } catch (e) {
        profile = { avatar_initials: 'TF', display_name: 'Пользователь', theme: 'dark' };
    }

    const initialsGroup = createElement('div', { className: 'form-group' });
    initialsGroup.appendChild(createElement('label', { className: 'form-label' }, 'Инициалы / аватар'));
    const initialsInput = createElement('input', {
        className: 'form-input',
        maxlength: '2',
        style: { maxWidth: '100px' }
    });
    initialsInput.value = profile.avatar_initials;
    initialsGroup.appendChild(initialsInput);
    container.appendChild(initialsGroup);

    const nameGroup = createElement('div', { className: 'form-group' });
    nameGroup.appendChild(createElement('label', { className: 'form-label' }, 'Отображаемое имя'));
    const nameInput = createElement('input', { className: 'form-input' });
    nameInput.value = profile.display_name;
    nameGroup.appendChild(nameInput);
    container.appendChild(nameGroup);

    const themeGroup = createElement('div', { className: 'form-group' });
    themeGroup.appendChild(createElement('label', { className: 'form-label' }, 'Тема оформления'));
    const themeToggle = createElement('div', { className: 'theme-toggle' });
    themeToggle.appendChild(createElement('button', {
        className: 'theme-toggle__option theme-toggle__option--active',
        type: 'button',
        disabled: 'true'
    }, 'Тёмная'));
    themeGroup.appendChild(themeToggle);
    themeGroup.appendChild(createElement('div', { className: 'form-hint' }, 'Светлая тема появится в следующих версиях'));
    container.appendChild(themeGroup);

    const saveBtn = createElement('button', { className: 'btn btn--primary' }, 'Сохранить');
    saveBtn.addEventListener('click', async () => {
        const initials = (initialsInput.value.trim() || 'TF').slice(0, 2).toUpperCase();
        const displayName = nameInput.value.trim() || 'Пользователь';
        try {
            await api.updateUserProfile(displayName, initials, 'dark');
            updateHeaderAvatar(initials);
            showToast('Профиль сохранён');
            if (opts.onSaved) opts.onSaved();
        } catch (e) {
            showToast('Ошибка сохранения профиля', 'error');
        }
    });
    container.appendChild(saveBtn);
}

// ─── Workspaces switcher ───

async function toggleWorkspacesPanel(anchor) {
    const existing = $('#workspaces-panel');
    if (existing) { existing.remove(); return; }
    closeAllPopovers();

    const panel = createElement('div', { className: 'header-popover', id: 'workspaces-panel' });
    panel.appendChild(createElement('div', { className: 'header-popover__title' }, 'Пространства'));

    openPopover(panel, anchor, { width: 280, align: 'start' });

    try {
        const workspaces = await api.getWorkspaces();
        const currentId = getWorkspaceId();

        const list = createElement('div', { className: 'workspace-option-list' });
        for (const ws of workspaces) {
            const item = createElement('div', {
                className: `workspace-option ${ws.id === currentId ? 'workspace-option--active' : ''}`
            });
            item.appendChild(createElement('div', { className: 'workspace-option__icon' }, ws.name.charAt(0).toUpperCase()));
            item.appendChild(createElement('span', { className: 'workspace-option__name' }, ws.name));
            item.addEventListener('click', () => {
                panel.remove();
                window.dispatchEvent(new CustomEvent('navigate', { detail: { view: 'hub', workspaceId: ws.id } }));
            });
            list.appendChild(item);
        }
        panel.appendChild(list);

        const createItem = createElement('div', { className: 'workspace-option workspace-option--create' });
        createItem.appendChild(createElement('span', { className: 'workspace-option__icon', innerHTML: Icons.plus }));
        createItem.appendChild(createElement('span', {}, 'Создать пространство'));
        createItem.addEventListener('click', () => {
            createItem.style.display = 'none';

            const form = createElement('div', { className: 'workspace-create-form' });
            const input = createElement('input', { className: 'form-input', placeholder: 'Название пространства...' });
            form.appendChild(input);
            const confirmBtn = createElement('button', { className: 'btn btn--primary btn--sm' }, 'Создать');
            confirmBtn.addEventListener('click', async () => {
                const name = input.value.trim();
                if (!name) return;
                try {
                    const ws = await api.createWorkspace(name);
                    panel.remove();
                    showToast(`Пространство "${name}" создано`);
                    window.dispatchEvent(new CustomEvent('navigate', { detail: { view: 'hub', workspaceId: ws.id } }));
                } catch (e) {
                    showToast('Ошибка создания пространства', 'error');
                }
            });
            form.appendChild(confirmBtn);
            panel.appendChild(form);
            input.focus();
            input.addEventListener('keydown', (e) => { if (e.key === 'Enter') confirmBtn.click(); });
        });
        panel.appendChild(createItem);
    } catch (e) {
        panel.appendChild(createElement('div', { className: 'header-popover__empty' }, 'Ошибка загрузки пространств'));
    }
}
