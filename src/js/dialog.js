// ============================================
// TaskFlow — Confirmations & modal keyboard handling
// ============================================
// Destructive actions (archiving, deleting) must pass through confirmDialog().
// Before this module they fired on a single click with no way back — the
// "Архивировать" button sat right next to "Сохранить" in the card editor.

import Icons from './icons.js';
import { createElement, $$ } from './utils.js';

/**
 * Asks the user to confirm an action. Resolves `true` only when they actively
 * confirm; cancelling, pressing Escape or clicking the backdrop resolves
 * `false`.
 *
 * Unlike the other modals in this codebase, this one deliberately does NOT
 * remove existing `.modal-overlay` nodes: a confirmation is usually raised from
 * inside another modal (archiving from the card editor), and tearing down its
 * parent would drop the user back on the board mid-decision. It stacks on top
 * instead, via `.modal-overlay--confirm`.
 *
 * @param {object}  opts
 * @param {string}  opts.title
 * @param {string}  opts.message      - plain text; inserted as a text node
 * @param {string} [opts.confirmText]
 * @param {string} [opts.cancelText]
 * @param {boolean}[opts.danger]      - red confirm button + warning icon
 * @returns {Promise<boolean>}
 */
export function confirmDialog({
    title,
    message,
    confirmText = 'Подтвердить',
    cancelText = 'Отмена',
    danger = false,
} = {}) {
    return new Promise((resolve) => {
        const overlay = createElement('div', { className: 'modal-overlay modal-overlay--confirm' });
        const modal = createElement('div', { className: 'modal modal--confirm' });

        let settled = false;
        const close = (result) => {
            if (settled) return;
            settled = true;
            overlay.remove();
            resolve(result);
        };

        // Picked up by the global Escape handler in initModalEscape().
        overlay.__onClose = () => close(false);

        const header = createElement('div', { className: 'modal__header' });
        const heading = createElement('div', { className: 'confirm__heading' });
        if (danger) {
            heading.appendChild(createElement('span', {
                className: 'confirm__icon',
                innerHTML: Icons.alertTriangle,
            }));
        }
        heading.appendChild(createElement('h2', { className: 'modal__title' }, title));
        header.appendChild(heading);
        modal.appendChild(header);

        const body = createElement('div', { className: 'modal__body' });
        body.appendChild(createElement('p', { className: 'confirm__message' }, message));
        modal.appendChild(body);

        const footer = createElement('div', { className: 'modal__footer' });

        const cancelBtn = createElement('button', { className: 'btn btn--secondary' }, cancelText);
        cancelBtn.addEventListener('click', () => close(false));
        footer.appendChild(cancelBtn);

        const confirmBtn = createElement('button', {
            className: `btn ${danger ? 'btn--danger' : 'btn--primary'}`,
        }, confirmText);
        confirmBtn.addEventListener('click', () => close(true));
        footer.appendChild(confirmBtn);

        modal.appendChild(footer);
        overlay.appendChild(modal);

        overlay.addEventListener('click', (e) => {
            if (e.target === overlay) close(false);
        });

        document.body.appendChild(overlay);

        // For destructive actions the safe option takes focus, so a stray Enter
        // cancels instead of deleting something.
        (danger ? cancelBtn : confirmBtn).focus();
    });
}

/**
 * Makes Escape close the topmost open modal.
 *
 * The help screen has always advertised "Esc — закрыть форму или модальное
 * окно", but only the inline forms ever honoured it: modals had no key handling
 * at all. Called once from app.js.
 *
 * Every modal in the app lives directly on <body>, so the last `.modal-overlay`
 * in DOM order is the one on top. A modal can opt into custom teardown by
 * setting `overlay.__onClose`; otherwise it is simply removed, which is what
 * each of their close buttons does anyway.
 */
export function initModalEscape() {
    document.addEventListener('keydown', (e) => {
        if (e.key !== 'Escape') return;

        const overlays = $$('.modal-overlay');
        if (overlays.length === 0) return;

        const top = overlays[overlays.length - 1];
        if (typeof top.__onClose === 'function') {
            top.__onClose();
        } else {
            top.remove();
        }
    });
}
