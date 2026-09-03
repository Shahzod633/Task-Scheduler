// ============================================
// TaskFlow — Комментарии к карточке
// ============================================
// Отдельный модуль по той же причине, что и чек-лист: окно карточки
// открывается с четырёх экранов (доска, «Список», «Ошибки», планировщик), и ни
// одному из них не нужно знать, как устроено обсуждение.
//
// Комментарий сохраняется сразу при отправке, а не по кнопке «Сохранить» окна.
// Написанное — это действие, а не черновик правки карточки: потерять его
// оттого, что окно закрыли крестиком, нельзя.

import * as api from './api.js';
import Icons from './icons.js';
import { createElement, showToast, formatDate } from './utils.js';
import { confirmDialog } from './dialog.js';

/**
 * Рисует блок «Комментарии» для одной карточки внутри `container`.
 *
 * @param {HTMLElement} container
 * @param {number} cardId
 */
export async function renderComments(container, cardId) {
    const group = createElement('div', { className: 'form-group comments' });

    const header = createElement('div', { className: 'comments__header' });
    header.appendChild(createElement('label', { className: 'form-label' }, 'Комментарии'));
    const counter = createElement('span', { className: 'comments__count' });
    header.appendChild(counter);
    group.appendChild(header);

    const list = createElement('div', { className: 'comments__list' });
    group.appendChild(list);

    // Поле ввода стоит под списком, а не над ним: читают сверху вниз, и писать
    // естественно там, где закончилась переписка.
    const form = createElement('div', { className: 'comments__form' });
    const input = createElement('textarea', {
        className: 'form-input comments__input',
        rows: '2',
        placeholder: 'Написать комментарий…',
    });
    const sendBtn = createElement('button', {
        className: 'btn btn--primary comments__send',
    }, 'Отправить');
    form.appendChild(input);
    form.appendChild(sendBtn);
    group.appendChild(form);

    container.appendChild(group);

    let comments = [];

    const paint = () => {
        list.innerHTML = '';
        counter.textContent = comments.length ? String(comments.length) : '';
        if (!comments.length) {
            list.appendChild(createElement('div', { className: 'comments__empty' },
                'Комментариев пока нет'));
            return;
        }
        for (const comment of comments) {
            list.appendChild(createRow(comment));
        }
    };

    function createRow(comment) {
        const row = createElement('div', { className: 'comment' });

        const avatar = createElement('div', { className: 'comment__avatar' },
            comment.author ? comment.author.initials : '?');
        if (comment.author) avatar.style.background = comment.author.color;
        row.appendChild(avatar);

        const main = createElement('div', { className: 'comment__main' });

        const meta = createElement('div', { className: 'comment__meta' });
        // Автора могли удалить из справочника — текст при этом остаётся.
        meta.appendChild(createElement('span', { className: 'comment__author' },
            comment.author ? comment.author.name : 'Участник удалён'));
        meta.appendChild(createElement('span', { className: 'comment__time' },
            formatDate(comment.created_at)));
        main.appendChild(meta);

        // textContent, а не innerHTML: текст пишет человек, и разметка в нём —
        // это текст, а не разметка. Переносы строк сохраняет CSS.
        main.appendChild(createElement('div', { className: 'comment__body' }, comment.body));
        row.appendChild(main);

        const del = createElement('button', {
            className: 'comment__delete',
            innerHTML: Icons.trash,
            title: 'Удалить комментарий',
        });
        del.addEventListener('click', async () => {
            const ok = await confirmDialog({
                title: 'Удалить комментарий?',
                message: 'Восстановить его будет нельзя.',
                confirmText: 'Удалить',
                danger: true,
            });
            if (!ok) return;
            try {
                await api.deleteCardComment(comment.id);
                comments = comments.filter(c => c.id !== comment.id);
                paint();
            } catch (e) {
                showToast(String(e), 'error');
            }
        });
        row.appendChild(del);

        return row;
    }

    const submit = async () => {
        const body = input.value.trim();
        if (!body) return;
        sendBtn.disabled = true;
        try {
            const created = await api.createCardComment(cardId, body);
            comments.push(created);
            input.value = '';
            paint();
            input.focus();
        } catch (e) {
            showToast(String(e), 'error');
        } finally {
            sendBtn.disabled = false;
        }
    };

    sendBtn.addEventListener('click', submit);
    input.addEventListener('keydown', (e) => {
        // Enter переносит строку — в комментарии это нужно чаще, чем отправка.
        // Отправляет Ctrl+Enter, привычный по любому мессенджеру. И то и другое
        // останавливается здесь, чтобы не дойти до окна и не сохранить карточку.
        if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
            e.preventDefault();
            e.stopPropagation();
            submit();
        } else if (e.key === 'Enter') {
            e.stopPropagation();
        }
    });

    try {
        comments = await api.listCardComments(cardId);
    } catch (e) {
        comments = [];
        showToast('Не удалось загрузить комментарии', 'error');
    }
    paint();
}
