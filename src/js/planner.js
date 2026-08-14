// ============================================
// TaskFlow — Planner Page (calendar of cards with a due date)
// ============================================

import * as api from './api.js';
import { showCardEditModal } from './board.js';
import { createElement, $, showToast } from './utils.js';

const MONTH_NAMES = ['Январь', 'Февраль', 'Март', 'Апрель', 'Май', 'Июнь', 'Июль', 'Август', 'Сентябрь', 'Октябрь', 'Ноябрь', 'Декабрь'];
const WEEKDAY_NAMES = ['Пн', 'Вт', 'Ср', 'Чт', 'Пт', 'Сб', 'Вс'];

let viewYear;
let viewMonth; // 0-indexed

export async function renderPlannerPage(workspaceId) {
    const content = $('#content');
    content.innerHTML = '';
    content.classList.add('view-enter');

    const now = new Date();
    if (viewYear === undefined) {
        viewYear = now.getFullYear();
        viewMonth = now.getMonth();
    }

    const page = createElement('div', { className: 'page page--planner' });

    const header = createElement('div', { className: 'planner-header' });
    const prevBtn = createElement('button', { className: 'btn btn--ghost' }, '←');
    const title = createElement('h2', { className: 'page__title planner-header__title' }, `${MONTH_NAMES[viewMonth]} ${viewYear}`);
    const nextBtn = createElement('button', { className: 'btn btn--ghost' }, '→');
    const todayBtn = createElement('button', { className: 'btn btn--secondary' }, 'Сегодня');
    header.appendChild(prevBtn);
    header.appendChild(title);
    header.appendChild(nextBtn);
    header.appendChild(todayBtn);
    page.appendChild(header);

    const grid = createElement('div', { className: 'planner-grid' });
    page.appendChild(grid);

    content.appendChild(page);
    setTimeout(() => content.classList.remove('view-enter'), 420);

    prevBtn.addEventListener('click', () => { shiftMonth(-1); renderPlannerPage(workspaceId); });
    nextBtn.addEventListener('click', () => { shiftMonth(1); renderPlannerPage(workspaceId); });
    todayBtn.addEventListener('click', () => {
        viewYear = now.getFullYear();
        viewMonth = now.getMonth();
        renderPlannerPage(workspaceId);
    });

    try {
        const cards = await api.getCardsWithDueDates(workspaceId);
        buildGrid(grid, cards, workspaceId);
    } catch (e) {
        showToast('Ошибка загрузки планировщика', 'error');
    }
}

function shiftMonth(delta) {
    viewMonth += delta;
    if (viewMonth < 0) { viewMonth = 11; viewYear--; }
    if (viewMonth > 11) { viewMonth = 0; viewYear++; }
}

function buildGrid(grid, cards, workspaceId) {
    grid.innerHTML = '';

    for (const wd of WEEKDAY_NAMES) {
        grid.appendChild(createElement('div', { className: 'planner-grid__weekday' }, wd));
    }

    const firstOfMonth = new Date(viewYear, viewMonth, 1);
    let startOffset = firstOfMonth.getDay() - 1;
    if (startOffset < 0) startOffset = 6;

    const daysInMonth = new Date(viewYear, viewMonth + 1, 0).getDate();
    const daysInPrevMonth = new Date(viewYear, viewMonth, 0).getDate();

    const cardsByDay = new Map();
    for (const card of cards) {
        const day = (card.due_date || '').slice(0, 10);
        if (!day) continue;
        if (!cardsByDay.has(day)) cardsByDay.set(day, []);
        cardsByDay.get(day).push(card);
    }

    const todayStr = new Date().toISOString().slice(0, 10);

    for (let i = 0; i < startOffset; i++) {
        const dayNum = daysInPrevMonth - startOffset + i + 1;
        grid.appendChild(createElement('div', { className: 'planner-cell planner-cell--muted' },
            createElement('span', { className: 'planner-cell__day' }, String(dayNum))));
    }

    for (let day = 1; day <= daysInMonth; day++) {
        const dateStr = `${viewYear}-${String(viewMonth + 1).padStart(2, '0')}-${String(day).padStart(2, '0')}`;
        const cell = createElement('div', {
            className: `planner-cell ${dateStr === todayStr ? 'planner-cell--today' : ''}`
        });
        cell.appendChild(createElement('span', { className: 'planner-cell__day' }, String(day)));

        const dayCards = cardsByDay.get(dateStr) || [];
        for (const card of dayCards) {
            const chip = createElement('div', {
                className: `planner-card-chip ${dateStr < todayStr ? 'planner-card-chip--overdue' : ''}`
            });
            chip.appendChild(createElement('span', { className: 'planner-card-chip__title' }, card.title));
            if (card.board_name) {
                chip.appendChild(createElement('span', { className: 'planner-card-chip__board' }, card.board_name));
            }
            chip.addEventListener('click', () => {
                showCardEditModal(card, { onChange: () => renderPlannerPage(workspaceId) });
            });
            cell.appendChild(chip);
        }

        grid.appendChild(cell);
    }

    const totalCells = startOffset + daysInMonth;
    const trailing = (7 - (totalCells % 7)) % 7;
    for (let i = 1; i <= trailing; i++) {
        grid.appendChild(createElement('div', { className: 'planner-cell planner-cell--muted' },
            createElement('span', { className: 'planner-cell__day' }, String(i))));
    }
}
