// ============================================
// TaskFlow — Settings Page
// (workspace settings + full-page profile form)
// ============================================

import * as api from './api.js';
import { renderProfileFields } from './header.js';
import { createElement, $, showToast } from './utils.js';

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

    content.appendChild(page);
    setTimeout(() => content.classList.remove('view-enter'), 420);

    renderProfileFields(profileCard);
}
