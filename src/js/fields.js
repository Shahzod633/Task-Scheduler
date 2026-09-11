// ============================================
// TaskFlow — Пользовательские поля
// ============================================
// Доска объявляет свои поля — текст, число, дата или выбор из списка, — и в
// окне каждой её карточки под стандартными полями появляется ввод под каждое.
//
// Модуль рисует две вещи: раздел «Пользовательские поля» в окне настроек доски
// и сами вводы в окне карточки. Как и `checklist.js` с `comments.js`, он не
// знает, откуда открыто окно карточки, и не импортирует `board.js`.
//
// Значения полей, в отличие от чек-листа, сохраняются кнопкой «Сохранить»
// окна карточки: они стоят среди стандартных полей и ведут себя так же, как
// название или срок, — закрыл окно крестиком, значит передумал.

import * as api from './api.js';
import Icons from './icons.js';
import { confirmDialog } from './dialog.js';
import { createElement, showToast, pluralize, formatDueDate } from './utils.js';

/** Подписи типов. В базе типы хранятся по-английски, как приоритет. */
const FIELD_TYPE_LABELS = {
    text: 'Текст',
    number: 'Число',
    date: 'Дата',
    select: 'Выбор из списка',
};

const errorText = (e, fallback) => (typeof e === 'string' ? e : fallback);

/**
 * Значение поля для чтения, а не для ввода: число — по-русски («3,5»,
 * «1 200»), дата — словарём срока («Сегодня», «30 сент.»), остальное как есть.
 * Нужна «Списку»: там значения только показываются.
 */
export function formatFieldValue(fieldType, value) {
    if (fieldType === 'number') {
        const number = Number(value);
        return Number.isFinite(number) ? number.toLocaleString('ru-RU', { maximumFractionDigits: 10 }) : value;
    }
    if (fieldType === 'date') return formatDueDate(value) || value;
    return value;
}

// ─── Настройки доски: список полей ───────────────────────────────────────

/**
 * Раздел «Пользовательские поля» окна настроек доски: список полей, удаление и
 * форма «+ Добавить поле».
 *
 * @param {HTMLElement} container
 * @param {number} boardId
 */
export function renderFieldSettings(container, boardId) {
    const section = createElement('div', { className: 'field-settings' });
    section.appendChild(createElement('h3', { className: 'field-settings__title' }, 'Пользовательские поля'));
    section.appendChild(createElement('p', { className: 'field-settings__hint' },
        'Поля появляются в окне каждой карточки этой доски, под стандартными полями.'));

    const list = createElement('div', { className: 'field-settings__list' });
    section.appendChild(list);

    const addBtn = createElement('button', { className: 'field-settings__add', type: 'button' }, '+ Добавить поле');
    section.appendChild(addBtn);
    container.appendChild(section);

    // Порядок — перетаскиванием за ручку, как колонки и карточки на доске.
    // `forceFallback`: перетаскивание HTML5 окно Tauri перехватывает (см.
    // `initSortable` в board.js). Клон уходит в <body>, иначе его обрезала бы
    // прокрутка окна настроек.
    new Sortable(list, {
        handle: '.field-settings__handle',
        draggable: '.field-settings__row',
        animation: 150,
        forceFallback: true,
        fallbackOnBody: true,
        fallbackClass: 'field-settings__row--dragging',
        ghostClass: 'field-settings__row--ghost',
        onEnd: async (evt) => {
            if (evt.oldIndex === evt.newIndex) return;
            const ids = [...list.querySelectorAll('.field-settings__row')].map(row => Number(row.dataset.fieldId));
            try {
                await api.reorderCustomFields(boardId, ids);
            } catch (e) {
                showToast(errorText(e, 'Не удалось переставить поля'), 'error');
                // На экране порядок уже не тот, что в базе, — перечитываем.
                await reload();
            }
        },
    });

    const reload = async () => {
        let fields;
        try {
            fields = await api.getCustomFieldsWithValues(boardId);
        } catch (e) {
            list.innerHTML = '';
            list.appendChild(createElement('p', { className: 'field-settings__empty' },
                'Не удалось загрузить поля'));
            return;
        }
        list.innerHTML = '';
        if (!fields.length) {
            list.appendChild(createElement('p', { className: 'field-settings__empty' }, 'Полей пока нет'));
            return;
        }
        for (const field of fields) list.appendChild(createFieldRow(field, reload));
    };

    addBtn.addEventListener('click', () => {
        addBtn.hidden = true;
        openCreateForm(section, boardId, async (created) => {
            addBtn.hidden = false;
            if (created) await reload();
        });
    });

    reload();
}

function createFieldRow(field, reload) {
    const row = createElement('div', {
        className: 'field-settings__row',
        dataset: { fieldId: String(field.id) },
    });

    row.appendChild(createElement('span', {
        className: 'field-settings__handle',
        innerHTML: Icons.grip,
        'data-tooltip': 'Перетащите, чтобы поменять порядок',
    }));

    const main = createElement('div', { className: 'field-settings__main' });
    // Название — кнопка: щелчок переименовывает, как название колонки на доске.
    const nameBtn = createElement('button', {
        className: 'field-settings__name',
        type: 'button',
        'data-tooltip': 'Щёлкните, чтобы переименовать',
    }, field.name);
    nameBtn.addEventListener('click', () => startRename(nameBtn, field, reload));
    main.appendChild(nameBtn);
    const details = field.field_type === 'select'
        ? `${FIELD_TYPE_LABELS.select}: ${field.select_options.join(', ')}`
        : FIELD_TYPE_LABELS[field.field_type] || field.field_type;
    main.appendChild(createElement('span', { className: 'field-settings__type' }, details));
    row.appendChild(main);

    if (field.field_type === 'select') {
        const optionsBtn = createElement('button', {
            className: 'icon-btn field-settings__options-btn',
            type: 'button',
            innerHTML: Icons.edit,
            'data-tooltip': 'Изменить варианты',
        });
        optionsBtn.addEventListener('click', () => openOptionsEditor(main, field, reload));
        row.appendChild(optionsBtn);
    }

    const removeBtn = createElement('button', {
        className: 'icon-btn icon-btn--danger field-settings__remove',
        type: 'button',
        innerHTML: Icons.trash,
        'data-tooltip': 'Удалить поле',
    });
    removeBtn.addEventListener('click', async () => {
        // Удаление необратимо и задевает все карточки доски сразу — поэтому
        // окно называет, сколько значений пропадёт.
        const filled = field.values.length;
        const message = filled
            ? `Поле «${field.name}» пропадёт из окна всех карточек доски, а его значения на ${filled} `
                + `${pluralize(filled, ['карточке', 'карточках', 'карточках'])} удалятся. Вернуть их будет нельзя.`
            : `Поле «${field.name}» пропадёт из окна всех карточек доски. Оно ещё нигде не заполнено.`;
        const ok = await confirmDialog({
            title: 'Удалить поле?',
            message,
            confirmText: 'Удалить',
            danger: true,
        });
        if (!ok) return;
        try {
            await api.deleteCustomField(field.id);
            showToast(`Поле «${field.name}» удалено`);
            await reload();
        } catch (e) {
            showToast(errorText(e, 'Не удалось удалить поле'), 'error');
        }
    });
    row.appendChild(removeBtn);

    return row;
}

/**
 * Переименование прямо в строке: Enter или уход из поля сохраняет, Esc
 * отменяет — так же, как у названия колонки на доске.
 */
function startRename(nameBtn, field, reload) {
    const input = createElement('input', {
        className: 'form-input field-settings__rename',
        maxlength: '60',
    });
    input.value = field.name;
    nameBtn.replaceWith(input);
    input.focus();
    input.select();

    let finished = false;
    const finish = async (save) => {
        if (finished) return;
        finished = true;
        const name = input.value.trim();
        if (!save || !name || name === field.name) {
            input.replaceWith(nameBtn);
            return;
        }
        try {
            await api.renameCustomField(field.id, name);
            showToast(`Поле переименовано в «${name}»`);
            await reload();
        } catch (e) {
            showToast(errorText(e, 'Не удалось переименовать поле'), 'error');
            input.replaceWith(nameBtn);
        }
    };

    input.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') {
            e.preventDefault();
            finish(true);
        } else if (e.key === 'Escape') {
            // Esc отменяет переименование, а не закрывает окно настроек: общий
            // обработчик (`initModalEscape`) слушает document и сюда не дойдёт.
            e.preventDefault();
            e.stopPropagation();
            finish(false);
        }
    });
    input.addEventListener('blur', () => finish(true));
}

/**
 * Правка вариантов выбора — строкой через запятую, как при создании.
 *
 * Значения на карточках не проверяются и не чинятся (§31.4): убранный вариант
 * у карточек, где он выбран, так и останется, и окно карточки покажет его
 * отдельной строкой списка.
 */
function openOptionsEditor(container, field, reload) {
    if (container.querySelector('.field-settings__options-editor')) return;

    const editor = createElement('div', { className: 'field-settings__options-editor' });
    const input = createElement('input', { className: 'form-input' });
    input.value = field.select_options.join(', ');
    editor.appendChild(input);
    editor.appendChild(createElement('p', { className: 'field-settings__note' },
        'Через запятую. Уже выбранное на карточках останется как есть, даже если такого варианта больше не будет.'));

    const actions = createElement('div', { className: 'field-settings__actions' });
    const cancelBtn = createElement('button', { className: 'btn btn--secondary btn--sm', type: 'button' }, 'Отмена');
    const saveBtn = createElement('button', { className: 'btn btn--primary btn--sm', type: 'button' }, 'Сохранить');
    actions.append(cancelBtn, saveBtn);
    editor.appendChild(actions);

    cancelBtn.addEventListener('click', () => editor.remove());
    const submit = async () => {
        saveBtn.disabled = true;
        try {
            await api.updateCustomFieldOptions(field.id, input.value.split(','));
            showToast('Варианты сохранены');
            await reload();
        } catch (e) {
            showToast(errorText(e, 'Не удалось сохранить варианты'), 'error');
            saveBtn.disabled = false;
        }
    };
    saveBtn.addEventListener('click', submit);
    input.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') {
            e.preventDefault();
            submit();
        }
    });

    container.appendChild(editor);
    input.focus();
}

/**
 * Форма нового поля: название, тип и — только у выбора из списка — варианты
 * через запятую, как и просило задание.
 */
function openCreateForm(container, boardId, onDone) {
    const form = createElement('div', { className: 'field-settings__form' });

    const fields = createElement('div', { className: 'field-settings__fields' });

    const nameField = createElement('label', { className: 'field-settings__field field-settings__field--grow' });
    nameField.appendChild(createElement('span', { className: 'field-settings__label' }, 'Название'));
    const nameInput = createElement('input', {
        className: 'form-input',
        placeholder: 'Например, «Срочность»',
        maxlength: '60',
    });
    nameField.appendChild(nameInput);
    fields.appendChild(nameField);

    const typeField = createElement('label', { className: 'field-settings__field' });
    typeField.appendChild(createElement('span', { className: 'field-settings__label' }, 'Тип'));
    const typeSelect = createElement('select', { className: 'form-input field-settings__type-select' });
    for (const [value, label] of Object.entries(FIELD_TYPE_LABELS)) {
        typeSelect.appendChild(createElement('option', { value }, label));
    }
    typeField.appendChild(typeSelect);
    fields.appendChild(typeField);
    form.appendChild(fields);

    const optionsField = createElement('label', { className: 'field-settings__field' });
    optionsField.appendChild(createElement('span', { className: 'field-settings__label' }, 'Варианты через запятую'));
    const optionsInput = createElement('input', {
        className: 'form-input',
        placeholder: 'Низкая, Средняя, Высокая',
    });
    optionsField.appendChild(optionsInput);
    optionsField.hidden = true;
    form.appendChild(optionsField);

    typeSelect.addEventListener('change', () => {
        optionsField.hidden = typeSelect.value !== 'select';
        if (!optionsField.hidden) optionsInput.focus();
    });

    const actions = createElement('div', { className: 'field-settings__actions' });
    const cancelBtn = createElement('button', { className: 'btn btn--secondary btn--sm', type: 'button' }, 'Отмена');
    const saveBtn = createElement('button', { className: 'btn btn--primary btn--sm', type: 'button' }, 'Добавить');
    actions.append(cancelBtn, saveBtn);
    form.appendChild(actions);

    const close = (created) => {
        form.remove();
        onDone(created);
    };
    cancelBtn.addEventListener('click', () => close(false));

    const submit = async () => {
        const name = nameInput.value.trim();
        if (!name) {
            showToast('Укажите название поля', 'error');
            nameInput.focus();
            return;
        }
        const type = typeSelect.value;
        // Пустые куски и пробелы вокруг запятых бэкенд уберёт и сам; здесь
        // строка только режется на варианты.
        const options = type === 'select' ? optionsInput.value.split(',') : null;

        saveBtn.disabled = true;
        try {
            const field = await api.createCustomField(boardId, name, type, options);
            showToast(`Поле «${field.name}» добавлено`);
            close(true);
        } catch (e) {
            showToast(errorText(e, 'Не удалось добавить поле'), 'error');
            saveBtn.disabled = false;
        }
    };
    saveBtn.addEventListener('click', submit);
    for (const input of [nameInput, optionsInput]) {
        input.addEventListener('keydown', (e) => {
            if (e.key === 'Enter') {
                e.preventDefault();
                submit();
            }
        });
    }

    container.appendChild(form);
    nameInput.focus();
}

// ─── Окно карточки: вводы под поля доски ────────────────────────────────

/**
 * Рисует вводы под поля доски в окне карточки.
 *
 * Поля грузятся асинхронно, но место под ними занимается сразу — там, где
 * функцию позвали, то есть под стандартными полями. Иначе блок, приехавший
 * позже чек-листа, встал бы под ним.
 *
 * Возвращает объект с `changes()`: окно карточки зовёт его по «Сохранить» и
 * пишет только то, что действительно поменялось.
 *
 * @param {HTMLElement} container
 * @param {number} cardId
 * @param {number|null} boardId - доска карточки; без неё полей не показать
 */
export function renderCardFields(container, cardId, boardId) {
    const wrap = createElement('div', { className: 'card-fields' });
    container.appendChild(wrap);

    // Какие вводы нарисованы и что в них было при открытии окна.
    const inputs = [];

    if (boardId) {
        api.getCustomFieldsWithValues(boardId)
            .then((fields) => {
                for (const field of fields) {
                    const current = field.values.find(v => v.card_id === cardId);
                    const initial = current ? current.value : '';
                    const input = createFieldInput(field, initial);
                    inputs.push({ field, input, initial });

                    const group = createElement('label', { className: 'form-group card-fields__field' });
                    group.appendChild(createElement('span', { className: 'form-label' }, field.name));
                    group.appendChild(input);
                    wrap.appendChild(group);
                }
            })
            .catch(() => showToast('Не удалось загрузить поля доски', 'error'));
    }

    return {
        /**
         * Изменённые поля: `[{ field, value }]`, где `value === null` — очистить.
         * Бросает `Error` с понятным текстом, если в поле числа не число: браузер
         * в этом случае отдаёт пустую строку, и без проверки поле молча
         * очистилось бы.
         */
        changes() {
            const result = [];
            for (const { field, input, initial } of inputs) {
                if (field.field_type === 'number' && input.validity.badInput) {
                    throw new Error(`В поле «${field.name}» нужно число`);
                }
                const value = input.value.trim();
                if (value !== initial) result.push({ field, value: value || null });
            }
            return result;
        },
    };
}

function createFieldInput(field, initial) {
    if (field.field_type === 'select') {
        const select = createElement('select', { className: 'form-input' });
        // Пустой вариант — «не выбрано»: поле необязательное, и очистить его
        // должно быть так же просто, как заполнить.
        select.appendChild(createElement('option', { value: '' }, '—'));
        for (const option of field.select_options) {
            select.appendChild(createElement('option', { value: option }, option));
        }
        // Значение, которого среди вариантов нет, выбрать нельзя, — но и
        // потерять его молча тоже: оно показывается отдельной строкой.
        if (initial && !field.select_options.includes(initial)) {
            select.appendChild(createElement('option', { value: initial }, initial));
        }
        select.value = initial;
        return select;
    }

    const attrs = { className: 'form-input' };
    if (field.field_type === 'number') Object.assign(attrs, { type: 'number', step: 'any', inputmode: 'decimal' });
    else if (field.field_type === 'date') attrs.type = 'date';
    else Object.assign(attrs, { type: 'text', maxlength: '500' });

    const input = createElement('input', attrs);
    input.value = initial;
    return input;
}
