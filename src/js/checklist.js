// ============================================
// TaskFlow — Checklists (sub-tasks inside a card)
// ============================================
// Lives in its own module because the card modal is opened from three places
// (board, "Список", "Требуют внимания") and none of them should have to know
// how a checklist works.
//
// Unlike the rest of the card modal, edits here are saved immediately rather
// than on "Сохранить". A checkbox that silently forgets what you ticked because
// you closed the window is worse than the small inconsistency: ticking something
// off *is* the action, not a draft of one.

import * as api from './api.js';
import Icons from './icons.js';
import { createElement, showToast } from './utils.js';

/**
 * Renders the "Чек-лист" block for one card into `container`.
 *
 * @param {HTMLElement} container
 * @param {number} cardId
 * @param {() => void} onChanged - called after every saved edit, so the caller
 *        can refresh the card face counter once the modal closes
 */
export async function renderChecklist(container, cardId, onChanged = () => {}) {
    const group = createElement('div', { className: 'form-group checklist' });

    const header = createElement('div', { className: 'checklist__header' });
    header.appendChild(createElement('label', { className: 'form-label' }, 'Чек-лист'));
    const progress = createElement('span', { className: 'checklist__progress' });
    header.appendChild(progress);
    group.appendChild(header);

    const bar = createElement('div', { className: 'checklist__bar' });
    const barFill = createElement('div', { className: 'checklist__bar-fill' });
    bar.appendChild(barFill);
    group.appendChild(bar);

    const list = createElement('div', { className: 'checklist__items' });
    group.appendChild(list);

    container.appendChild(group);

    let items = [];

    const paintProgress = () => {
        const total = items.length;
        const done = items.filter(i => i.is_done).length;
        progress.textContent = total ? `${done} из ${total}` : '';
        bar.style.display = total ? '' : 'none';
        barFill.style.width = total ? `${Math.round((done / total) * 100)}%` : '0%';
    };

    const paint = () => {
        list.innerHTML = '';
        if (items.length === 0) {
            list.appendChild(createElement('div', { className: 'checklist__empty' },
                'Пунктов пока нет'));
        }
        for (const item of items) {
            list.appendChild(createItemRow(item));
        }
        paintProgress();
    };

    function createItemRow(item) {
        const row = createElement('div', {
            className: `checklist__item ${item.is_done ? 'checklist__item--done' : ''}`
        });

        const box = createElement('input', { type: 'checkbox', className: 'checklist__check' });
        box.checked = item.is_done;
        box.addEventListener('change', async () => {
            // Locked while the write is in flight so a double-click cannot send
            // two toggles and land on the state the user did not pick.
            box.disabled = true;
            try {
                item.is_done = await api.toggleChecklistItem(item.id);
                row.classList.toggle('checklist__item--done', item.is_done);
                paintProgress();
                onChanged();
            } catch (e) {
                box.checked = item.is_done;
                showToast('Не удалось отметить пункт', 'error');
            } finally {
                box.disabled = false;
            }
        });
        row.appendChild(box);

        row.appendChild(createElement('span', { className: 'checklist__text' }, item.text));

        const removeBtn = createElement('button', {
            className: 'icon-btn icon-btn--danger checklist__remove',
            type: 'button',
            innerHTML: Icons.trash,
            'data-tooltip': 'Удалить пункт'
        });
        removeBtn.addEventListener('click', async () => {
            try {
                await api.deleteChecklistItem(item.id);
                items = items.filter(i => i.id !== item.id);
                paint();
                onChanged();
            } catch (e) {
                showToast('Не удалось удалить пункт', 'error');
            }
        });
        row.appendChild(removeBtn);

        return row;
    }

    // ─── Add form ───
    const form = createElement('div', { className: 'checklist__add' });
    const input = createElement('input', {
        className: 'form-input checklist__input',
        placeholder: 'Добавить пункт...'
    });
    form.appendChild(input);

    const addBtn = createElement('button', {
        className: 'btn btn--secondary btn--sm',
        type: 'button',
        innerHTML: Icons.plus
    });
    form.appendChild(addBtn);
    group.appendChild(form);

    const submit = async () => {
        const text = input.value.trim();
        if (!text) return;
        addBtn.disabled = true;
        try {
            const created = await api.createChecklistItem(cardId, text);
            items.push(created);
            input.value = '';
            paint();
            onChanged();
            // Focus stays put so a list can be typed in one go.
            input.focus();
        } catch (e) {
            showToast(String(e), 'error');
        } finally {
            addBtn.disabled = false;
        }
    };

    addBtn.addEventListener('click', submit);
    input.addEventListener('keydown', (e) => {
        // Enter must not reach the modal and submit the whole card.
        if (e.key === 'Enter') { e.preventDefault(); e.stopPropagation(); submit(); }
    });

    try {
        items = await api.listChecklistItems(cardId);
    } catch (e) {
        items = [];
        showToast('Не удалось загрузить чек-лист', 'error');
    }
    paint();
}
