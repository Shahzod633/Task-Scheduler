// ============================================
// TaskFlow — Метки карточки
// ============================================
// Метки принадлежат доске: создаются один раз и навешиваются на любые её
// карточки. Как чек-лист, пишутся сразу, без «Сохранить» — повесить метку и
// есть действие, а не черновик. Бэкенд (`add_label_to_card`) не пустит метку
// чужой доски, так что окно может не проверять это само.

import * as api from './api.js';
import Icons from './icons.js';
import { openPopover, closePopovers } from './popover.js';
import { confirmDialog } from './dialog.js';
import { createElement, showToast } from './utils.js';

const PALETTE = ['#e5484d', '#f76b15', '#f5b800', '#30a46c', '#12a594', '#3e63dd', '#8e4ec6', '#6e7681'];

/**
 * Блок «Метки» в окне карточки.
 *
 * @param {HTMLElement} container
 * @param {number} cardId
 * @param {number|null} boardId   доска карточки; без неё блок не рисуется
 * @param {() => void} onChanged  вызывается после каждой записи — лицевая
 *        сторона карточки обновится при закрытии окна
 */
export async function renderCardLabels(container, cardId, boardId, onChanged = () => {}) {
    if (boardId == null) return;

    const group = createElement('div', { className: 'form-group card-labels' });
    group.appendChild(createElement('label', { className: 'form-label' }, 'Метки'));
    const row = createElement('div', { className: 'card-labels__row' });
    group.appendChild(row);
    container.appendChild(group);

    let boardLabels = [];
    let assigned = new Set();

    const paint = () => {
        row.innerHTML = '';
        for (const label of boardLabels.filter(l => assigned.has(l.id))) {
            const chip = createElement('span', { className: 'label-chip', style: { background: label.color } });
            chip.textContent = label.name || '  ';
            row.appendChild(chip);
        }
        const add = createElement('button', { className: 'label-chip label-chip--add', type: 'button' },
            assigned.size ? '＋' : '＋ Метка');
        add.addEventListener('click', () => openPicker(add));
        row.appendChild(add);
    };

    const toggle = async (label) => {
        try {
            if (assigned.has(label.id)) {
                await api.removeLabelFromCard(cardId, label.id);
                assigned.delete(label.id);
            } else {
                await api.addLabelToCard(cardId, label.id);
                assigned.add(label.id);
            }
            onChanged();
        } catch (e) {
            showToast(typeof e === 'string' ? e : 'Не удалось изменить метку', 'error');
        }
    };

    const openPicker = (anchor) => {
        const menu = createElement('div', { className: 'label-picker' });
        const list = createElement('div', { className: 'label-picker__list' });
        menu.appendChild(list);

        const paintList = () => {
            list.innerHTML = '';
            if (boardLabels.length === 0) {
                list.appendChild(createElement('div', { className: 'label-picker__empty' }, 'На доске пока нет меток'));
            }
            for (const label of boardLabels) {
                const item = createElement('div', { className: 'label-picker__item' });
                const swatch = createElement('button', {
                    className: 'label-picker__swatch', type: 'button', style: { background: label.color },
                });
                swatch.appendChild(createElement('span', {}, label.name || ''));
                if (assigned.has(label.id)) swatch.appendChild(createElement('span', {}, '✓'));
                swatch.addEventListener('click', async () => { await toggle(label); paintList(); paint(); });
                item.appendChild(swatch);

                const del = createElement('button', {
                    className: 'label-picker__del', type: 'button', title: 'Удалить метку с доски', innerHTML: Icons.x,
                });
                del.addEventListener('click', async () => {
                    closePopovers();
                    const ok = await confirmDialog({
                        title: 'Удалить метку?',
                        message: `Метка «${label.name || 'без названия'}» пропадёт со всех карточек доски.`,
                        confirmText: 'Удалить', danger: true,
                    });
                    if (!ok) return;
                    try {
                        await api.deleteLabel(label.id);
                        boardLabels = boardLabels.filter(l => l.id !== label.id);
                        assigned.delete(label.id);
                        paint();
                        onChanged();
                    } catch (e) {
                        showToast('Не удалось удалить метку', 'error');
                    }
                });
                item.appendChild(del);
                list.appendChild(item);
            }
        };
        paintList();

        // Новая метка: имя + цвет из палитры.
        const form = createElement('div', { className: 'label-picker__new' });
        const nameInput = createElement('input', {
            className: 'form-input', placeholder: 'Новая метка…', maxlength: '40',
        });
        form.appendChild(nameInput);
        let color = PALETTE[0];
        const colors = createElement('div', { className: 'label-picker__colors' });
        const dots = PALETTE.map((c) => {
            const d = createElement('button', { className: 'label-picker__color', type: 'button', style: { background: c } });
            d.addEventListener('click', () => { color = c; dots.forEach(x => x.classList.toggle('is-active', x === d)); });
            colors.appendChild(d);
            return d;
        });
        dots[0].classList.add('is-active');
        form.appendChild(colors);

        const create = createElement('button', { className: 'btn btn--primary', type: 'button' }, 'Создать');
        create.addEventListener('click', async () => {
            try {
                const label = await api.createLabel(boardId, nameInput.value, color);
                boardLabels.push(label);
                await toggle(label);
                nameInput.value = '';
                paintList();
                paint();
            } catch (e) {
                showToast(typeof e === 'string' ? e : 'Не удалось создать метку', 'error');
            }
        });
        nameInput.addEventListener('keydown', (e) => { if (e.key === 'Enter') create.click(); });
        form.appendChild(create);
        menu.appendChild(form);

        openPopover(menu, anchor, { placement: 'bottom', align: 'start', gap: 4 });
    };

    try {
        const [all, mine] = await Promise.all([api.getLabels(boardId), api.getCardLabels(cardId)]);
        boardLabels = all;
        assigned = new Set(mine.map(l => l.id));
    } catch (e) {
        console.error('Не удалось прочитать метки:', e);
    }
    paint();
}
