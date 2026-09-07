// ============================================
// TaskFlow — Тема оформления
// ============================================
// Три режима, но палитры две: «Тёмная», «Светлая» и «Как в системе», где
// выбор за Windows. Отсюда два разных понятия, которые нельзя путать:
//
//   • режим (`mode`)     — что выбрал человек, это и лежит в базе;
//   • палитра (`resolved`) — что сейчас нарисовано, это и стоит в `data-theme`.
//
// Для 'dark'/'light' они совпадают, для 'system' — нет, и меняться палитра
// может без единого действия пользователя: он переключил тему в самой Windows,
// пока TaskFlow открыт.
//
// Хранится режим там же, где остальные настройки, — в SQLite
// (`user_profile.theme`), а не в localStorage: это Tauri-приложение, и
// настройка, живущая в хранилище браузера, потерялась бы вместе с профилем
// WebView2, не попала бы ни в резервную копию, ни в экспорт.

import * as api from './api.js';

export const THEME_MODES = ['dark', 'light', 'system'];

/** Что видит человек в переключателе. */
export const THEME_LABELS = {
    dark: 'Тёмная',
    light: 'Светлая',
    system: 'Как в системе',
};

const DEFAULT_MODE = 'dark';

let currentMode = DEFAULT_MODE;

/**
 * Медиазапрос создаётся один раз и живёт всё время работы приложения.
 *
 * Слушатель с него не снимается намеренно: подписка нужна ровно столько,
 * сколько открыто окно, а «отписаться при уходе с экрана настроек» означало бы
 * перестать реагировать на системную тему, как только человек закрыл настройки.
 */
const systemDark = window.matchMedia('(prefers-color-scheme: dark)');

/** Какая палитра соответствует режиму прямо сейчас. */
export function resolveMode(mode) {
    if (mode === 'system') return systemDark.matches ? 'dark' : 'light';
    return mode === 'light' ? 'light' : 'dark';
}

/** Выбранный режим (не палитра). */
export function getThemeMode() {
    return currentMode;
}

/**
 * Ставит палитру на `<html>`.
 *
 * Атрибут проставляется всегда, включая `dark`, хотя тёмная палитра и так
 * базовая: по нему видно, что тема уже применена, а не осталась значением по
 * умолчанию из CSS.
 */
function paint(mode) {
    const resolved = resolveMode(mode);
    document.documentElement.dataset.theme = resolved;
    // Графики рисуются на canvas и CSS-переменных не видят — им нужен сигнал.
    window.dispatchEvent(new CustomEvent('themechange', { detail: { mode, resolved } }));
}

/**
 * Читает сохранённый режим и применяет его.
 *
 * Вызывать до первой отрисовки: пока режим не прочитан, действует тёмная
 * палитра из `:root`, и человек со светлой темой увидит тёмную вспышку.
 */
export async function initTheme() {
    try {
        const profile = await api.getUserProfile();
        if (THEME_MODES.includes(profile.theme)) currentMode = profile.theme;
    } catch (e) {
        // Настройки не прочитались — это не повод не запускаться. Тёмная
        // палитра и так уже нарисована, менять нечего.
        console.error('Не удалось прочитать тему:', e);
    }

    paint(currentMode);

    // Windows умеет менять тему на ходу — по расписанию или руками. Пока
    // выбран режим «как в системе», приложение обязано ехать следом, не
    // дожидаясь перезапуска.
    systemDark.addEventListener('change', () => {
        if (currentMode === 'system') paint(currentMode);
    });
}

/**
 * Переключает режим: сначала рисует, потом сохраняет.
 *
 * Порядок именно такой — переключатель должен отзываться мгновенно, а поход в
 * базу занимает хоть и миллисекунды, но ждать его незачем. Если запись
 * сорвалась, откатываем нарисованное: показывать одну тему, сохранив другую,
 * хуже, чем не переключиться вовсе.
 */
export async function setThemeMode(mode) {
    if (!THEME_MODES.includes(mode)) return currentMode;

    const previous = currentMode;
    currentMode = mode;
    paint(currentMode);

    try {
        await api.updateTheme(mode);
    } catch (e) {
        currentMode = previous;
        paint(currentMode);
        throw e;
    }
    return currentMode;
}
