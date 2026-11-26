// ============================================
// TaskFlow — Метки карточки
// ============================================
// Метки принадлежат доске: создаются один раз и навешиваются на любые её
// карточки. Как чек-лист, пишутся сразу, без «Сохранить» — повесить метку и
// есть действие, а не черновик. Бэкенд (`add_label_to_card`) не пустит метку
// чужой доски, так что окно может не проверять это само.
//
// Цвета новых меток — токены `--label-*` из variables.css, а не свой список:
// палитра у приложения одна. В базу уходит значение токена (#rrggbb) — цвет
// метки это данные, и при смене темы он меняться не должен.

import * as api from './api.js';
import Icons from './icons.js';
import { openPopover, closePopovers } from './popover.js';
import { confirmDialog } from './dialog.js';
import { createElement, showToast, textColorOn } from './utils.js';

/** Токены палитры в порядке показа и их имена — для подсказки и диктора. */
const PALETTE = [
    ['--label-red', 'Красный'],
    ['--label-orange', 'Оранжевый'],
    ['--label-yellow', 'Жёлтый'],
    ['--label-lime', 'Салатовый'],
    ['--label-green', 'Зелёный'],
    ['--label-sky', 'Голубой'],
    ['--label-blue', 'Синий'],
    ['--label-purple', 'Фиолетовый'],
    ['--label-pink', 'Розовый'],
    ['--label-black', 'Графитовый'],
];

/** Значения токенов палитры: читаются при открытии окна, а не при загрузке модуля. */
function paletteColors() {
    const css = getComputedStyle(document.documentElement);
    return PALETTE.map(([token, name]) => ({ value: css.getPropertyValue(token).trim(), name }))
        .filter((c) => /^#[0-9a-f]{6}$/i.test(c.value));
}

/** Чип метки: цвет — её собственный, текст — тот из двух, что на нём читается. */
function paintChip(el, label) {
    el.style.background = label.color;
    el.style.color = textColorOn(label.color);
}

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
        for (const label of boardLabels.filter((l) => assigned.has(l.id))) {
            const chip = createElement('span', { className: 'label-chip' }, label.name || ' ');
            paintChip(chip, label);
            if (!label.name) chip.setAttribute('aria-label', 'Метка без названия');
            row.appendChild(chip);
        }
        const add = createElement('button', {
            className: 'label-chip label-chip--add',
            type: 'button',
            innerHTML: Icons.plus,
            'aria-label': assigned.size ? 'Изменить метки' : 'Добавить метку',
        });
        if (!assigned.size) add.appendChild(createElement('span', {}, 'Метка'));
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
        // `context-menu` — общая подложка всплывающих меню (стекло, рамка,
        // тень). Без неё окно было прозрачным и просвечивало чек-листом.
        const menu = createElement('div', { className: 'context-menu label-picker' });
        const list = createElement('div', { className: 'label-picker__list' });
        menu.appendChild(list);

        const paintList = () => {
            list.innerHTML = '';
            if (boardLabels.length === 0) {
                list.appendChild(createElement('div', { className: 'label-picker__empty' }, 'На доске пока нет меток'));
            }
            for (const label of boardLabels) {
                const on = assigned.has(label.id);
                const item = createElement('div', { className: 'label-picker__item' });
                const swatch = createElement('button', {
                    className: 'label-picker__swatch',
                    type: 'button',
                    'aria-pressed': on ? 'true' : 'false',
                });
                paintChip(swatch, label);
                swatch.appendChild(createElement('span', { className: 'label-picker__name' }, label.name || 'Без названия'));
                if (on) swatch.appendChild(createElement('span', { className: 'label-picker__check', innerHTML: Icons.check }));
                swatch.addEventListener('click', async () => { await toggle(label); paintList(); paint(); });
                item.appendChild(swatch);

                const del = createElement('button', {
                    className: 'label-picker__del',
                    type: 'button',
                    innerHTML: Icons.x,
                    'aria-label': `Удалить метку «${label.name || 'без названия'}» с доски`,
                    'data-tooltip': 'Удалить с доски',
                });
                del.addEventListener('click', async () => {
                    closePopovers();
                    const ok = await confirmDialog({
                        title: 'Удалить метку?',
                        message: `Метка «${label.name || 'без названия'}» пропадёт со всех карточек доски.`,
                        confirmText: 'Удалить',
                        danger: true,
                    });
                    if (!ok) return;
                    try {
                        await api.deleteLabel(label.id);
                        boardLabels = boardLabels.filter((l) => l.id !== label.id);
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

        // Новая метка: имя и цвет из палитры приложения.
        const form = createElement('div', { className: 'label-picker__new' });
        const nameInput = createElement('input', {
            className: 'form-input',
            placeholder: 'Название новой метки',
            maxlength: '40',
            'aria-label': 'Название новой метки',
        });
        form.appendChild(nameInput);

        const colors = paletteColors();
        let color = colors[0] ? colors[0].value : '';
        const dots = createElement('div', { className: 'label-picker__colors', role: 'group', 'aria-label': 'Цвет метки' });
        const dotEls = colors.map((c, i) => {
            const d = createElement('button', {
                className: 'label-picker__color',
                type: 'button',
                style: { background: c.value },
                'aria-label': c.name,
                'aria-pressed': i === 0 ? 'true' : 'false',
                'data-tooltip': c.name,
            });
            d.addEventListener('click', () => {
                color = c.value;
                dotEls.forEach((x) => x.setAttribute('aria-pressed', x === d ? 'true' : 'false'));
            });
            dots.appendChild(d);
            return d;
        });
        form.appendChild(dots);

        const create = createElement('button', { className: 'btn btn--primary btn--sm', type: 'button' }, 'Создать метку');
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
        assigned = new Set(mine.map((l) => l.id));
    } catch (e) {
        console.error('Не удалось прочитать метки:', e);
    }
    paint();
}
