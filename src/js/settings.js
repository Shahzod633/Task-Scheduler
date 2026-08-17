// ============================================
// TaskFlow — Settings Page
// (workspace settings + full-page profile form)
// ============================================

import * as api from './api.js';
import Icons from './icons.js';
import { renderProfileFields } from './header.js';
import { createElement, $, showToast, pluralize } from './utils.js';

export async function renderSettingsPage(workspaceId) {
    const content = $('#content');
    content.innerHTML = '';
    content.classList.add('view-enter');

    const page = createElement('div', { className: 'page page--settings' });
    page.appendChild(createElement('h2', { className: 'page__title' }, 'Настройки'));

    try {
        const workspaces = await api.getWorkspaces();
        const workspace = workspaces.find(w => w.id === workspaceId);

        if (workspace) {
            page.appendChild(createElement('h3', { className: 'settings-section-title' }, 'Рабочее пространство'));
            const card = createElement('div', { className: 'settings-card' });

            const nameGroup = createElement('div', { className: 'form-group' });
            nameGroup.appendChild(createElement('label', { className: 'form-label' }, 'Название'));
            const nameInput = createElement('input', { className: 'form-input' });
            nameInput.value = workspace.name;
            nameGroup.appendChild(nameInput);
            card.appendChild(nameGroup);

            const visGroup = createElement('div', { className: 'form-group' });
            visGroup.appendChild(createElement('label', { className: 'form-label' }, 'Приватность'));
            const visSelect = createElement('select', { className: 'form-input' });
            const options = [['private', 'Приватное'], ['public', 'Публичное']];
            for (const [value, label] of options) {
                const opt = createElement('option', { value }, label);
                if (workspace.visibility === value) opt.setAttribute('selected', 'selected');
                visSelect.appendChild(opt);
            }
            visGroup.appendChild(visSelect);
            card.appendChild(visGroup);

            const saveBtn = createElement('button', { className: 'btn btn--primary' }, 'Сохранить');
            saveBtn.addEventListener('click', async () => {
                const name = nameInput.value.trim();
                if (!name) return;
                try {
                    await api.updateWorkspace(workspace.id, name, visSelect.value);
                    showToast('Пространство обновлено');
                    window.dispatchEvent(new CustomEvent('navigate', { detail: { view: 'settings', workspaceId } }));
                } catch (e) {
                    showToast('Ошибка сохранения', 'error');
                }
            });
            card.appendChild(saveBtn);

            page.appendChild(card);
        }
    } catch (e) {
        showToast('Ошибка загрузки пространства', 'error');
    }

    page.appendChild(createElement('h3', { className: 'settings-section-title' }, 'Профиль'));
    const profileCard = createElement('div', { className: 'settings-card' });
    page.appendChild(profileCard);

    page.appendChild(createElement('h3', { className: 'settings-section-title' }, 'Резервные копии'));
    const backupCard = createElement('div', { className: 'settings-card' });
    page.appendChild(backupCard);

    content.appendChild(page);
    setTimeout(() => content.classList.remove('view-enter'), 420);

    renderProfileFields(profileCard);
    renderBackupSection(backupCard);
}

/**
 * Backups panel.
 *
 * All of the user's data lives in a single SQLite file, so the app now snapshots
 * it on every start and keeps the last 10. This section exists so that safety
 * net is visible rather than a silent implementation detail — otherwise nobody
 * would know the copies are there when they need them.
 */
async function renderBackupSection(container) {
    container.appendChild(createElement('p', { className: 'form-hint settings-backup__intro' },
        'Копия базы создаётся автоматически при запуске приложения, но не чаще раза в час. Хранятся последние 10.'));

    let backups = [];
    let dir = '';
    try {
        [backups, dir] = await Promise.all([api.getBackups(), api.getBackupDir()]);
    } catch (e) {
        container.appendChild(createElement('p', { className: 'form-hint' }, 'Не удалось прочитать папку с копиями'));
        return;
    }

    const pathRow = createElement('div', { className: 'settings-backup__path' });
    pathRow.appendChild(createElement('code', { className: 'settings-backup__path-value' }, dir));
    container.appendChild(pathRow);

    const count = backups.length;
    container.appendChild(createElement('div', { className: 'settings-backup__count' },
        count === 0
            ? 'Копий пока нет — первая появится при следующем запуске'
            : `${count} ${pluralize(count, ['копия', 'копии', 'копий'])}`));

    if (count > 0) {
        const list = createElement('div', { className: 'settings-backup__list' });
        for (const backup of backups) {
            const row = createElement('div', { className: 'settings-backup__row' });
            row.appendChild(createElement('span', { className: 'settings-backup__date' }, backup.created_at));
            row.appendChild(createElement('span', { className: 'settings-backup__size' }, formatBytes(backup.size_bytes)));
            list.appendChild(row);
        }
        container.appendChild(list);
    }

    const openBtn = createElement('button', {
        className: 'btn btn--secondary',
        innerHTML: `${Icons.archive} <span>Открыть папку с копиями</span>`
    });
    openBtn.addEventListener('click', async () => {
        try {
            await api.openBackupDir();
        } catch (e) {
            showToast('Не удалось открыть папку', 'error');
        }
    });
    container.appendChild(openBtn);

    container.appendChild(createElement('p', { className: 'form-hint settings-backup__note' },
        'Чтобы восстановиться из копии, закройте приложение и замените ею файл trello_clone.db в папке выше. Восстановление прямо из интерфейса появится позже.'));
}

function formatBytes(bytes) {
    if (bytes < 1024) return `${bytes} Б`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} КБ`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} МБ`;
}
