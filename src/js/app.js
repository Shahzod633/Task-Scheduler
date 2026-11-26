// ============================================
// TaskFlow — Main Application Entry Point
// ============================================

import * as api from './api.js';
import Icons from './icons.js';
import { renderBoard } from './board.js';
import { renderHub } from './hub.js';
import { renderWorkspaceSidebar, ensureDefaultWorkspace } from './workspace.js';
import { initSidebarRail, setSidebarContext } from './sidebar.js';
import { initHeaderInteractions } from './header.js';
import { initTooltips } from './popover.js';
import { initModalEscape } from './dialog.js';
import { renderTemplatesPage } from './templates.js';
import { renderHomePage } from './home.js';
import { renderSettingsPage } from './settings.js';
import { renderInboxPage } from './inbox.js';
import { renderPlannerPage } from './planner.js';
import { renderAssistantPage } from './assistant.js';
import { renderRecentPage } from './recent.js';
import { renderFavoritesPage } from './favorites.js';
import { renderMistakesPage } from './mistakes.js';
import { renderListPage } from './list.js';
import { renderMembersPage } from './members.js';
import { renderHabitsPage } from './habits.js';
import { applyWorkspaceBackground } from './background.js';
import { initTheme } from './theme.js';
import { openPalette, closePalette, isPaletteOpen } from './palette.js';
import { initTimer, askAboutOrphanedTimers } from './timer.js';
import { $, showToast } from './utils.js';

let currentView = { name: 'hub', params: {} };
let defaultWorkspaceId = null;
let lastBoardId = null;

// Экран → пункт сайдбара, который подсвечивается. С 12.11 сайдбар — единственная
// навигация: плавающего дока (Inbox, Планировщик, Ассистент, Доска, «Выбрать
// другую доску») больше нет, его пункты переехали сюда, и сайдбар виден на
// всех экранах, включая доску.
const SIDEBAR_ITEM_IDS = {
    hub: 'sidebar-boards',
    board: 'sidebar-current-board',
    list: 'sidebar-list',
    planner: 'sidebar-planner',
    habits: 'sidebar-habits',
    inbox: 'sidebar-inbox',
    mistakes: 'sidebar-mistakes',
    assistant: 'sidebar-assistant',
    templates: 'sidebar-templates',
    home: 'sidebar-home',
    settings: 'sidebar-settings',
};

const ROUTES = {
    hub: (params) => renderHub(params.workspaceId || defaultWorkspaceId),
    board: async (params) => {
        const boardId = params.boardId || lastBoardId;
        if (!boardId) {
            showToast('Сначала откройте доску', 'error');
            return;
        }
        lastBoardId = boardId;
        await api.recordBoardView(boardId);
        await renderBoard(boardId);
        showCurrentBoardItem(boardId);
    },
    templates: (params) => renderTemplatesPage(params.workspaceId || defaultWorkspaceId),
    home: (params) => renderHomePage(params.workspaceId || defaultWorkspaceId),
    settings: (params) => renderSettingsPage(params.workspaceId || defaultWorkspaceId),
    inbox: (params) => renderInboxPage(params.workspaceId || defaultWorkspaceId),
    planner: (params) => renderPlannerPage(params.workspaceId || defaultWorkspaceId),
    assistant: (params) => renderAssistantPage(params.workspaceId || defaultWorkspaceId),
    recent: (params) => renderRecentPage(params.workspaceId || defaultWorkspaceId),
    favorites: (params) => renderFavoritesPage(params.workspaceId || defaultWorkspaceId),
    mistakes: (params) => renderMistakesPage(params.workspaceId || defaultWorkspaceId),
    list: (params) => renderListPage(params.workspaceId || defaultWorkspaceId),
    members: (params) => renderMembersPage(params.workspaceId || defaultWorkspaceId),
    // Привычки общие для всех пространств — экрану id пространства не нужен.
    habits: () => renderHabitsPage(),
};

/**
 * Initialize the application
 */
async function init() {
    try {
        // Тема — до первой отрисовки и раньше всего остального: пока режим не
        // прочитан, действует тёмная палитра из `:root`, и человек, выбравший
        // светлую, увидел бы тёмную вспышку на весь экран.
        await initTheme();

        // Ensure we have at least one workspace
        defaultWorkspaceId = await ensureDefaultWorkspace();

        // Render the global layout
        renderLayout();
        initSidebarRail();
        initHeaderInteractions(() => defaultWorkspaceId);
        // Индикатор идущего таймера — сразу вместе с шапкой. Первый экран его
        // не ждёт: таймера чаще всего нет, а чтение его — лишний шаг до доски.
        const timerReady = initTimer();
        // Подсказки делегированы на document — достаточно включить один раз
        initTooltips();
        // Esc закрывает верхнюю открытую модалку. Справка обещала это с самого
        // начала, но обработчика не существовало — работали только формы.
        initModalEscape();

        // Render sidebar
        await renderWorkspaceSidebar(defaultWorkspaceId);

        // Start at hub view
        await navigateTo('hub', { workspaceId: defaultWorkspaceId });

        // Listen for navigation events
        window.addEventListener('navigate', (e) => {
            const { view, ...params } = e.detail;
            navigateTo(view, params);
        });

        initGlobalSearchShortcut();

        // Вопрос про забытый таймер — поверх уже открытого экрана и только
        // после того, как идущий таймер прочитан. Иначе запоздавшее чтение
        // вернуло бы в шапку таймер, который человек только что остановил в
        // этом самом окне.
        timerReady.then(askAboutOrphanedTimers);

    } catch (error) {
        console.error('Failed to initialize app:', error);
        showToast('Ошибка инициализации приложения', 'error');
    }
}

/**
 * Ctrl+K — поиск по всему пространству.
 *
 * Обработчик живёт здесь, а не в самой палитре, по одной причине: только
 * `app.js` знает, какое пространство открыто. Палитра получает id снаружи и не
 * лезет в состояние маршрутизатора.
 *
 * Слушатель повешен на `keydown` в фазе перехвата: иначе поле ввода, из
 * которого нажали, успело бы обработать событие первым.
 */
function initGlobalSearchShortcut() {
    document.addEventListener('keydown', (e) => {
        // `metaKey` — ради Cmd+K: приложение пока только для Windows, но
        // привычка нажимать Cmd приезжает вместе с человеком.
        if ((e.ctrlKey || e.metaKey) && (e.key === 'k' || e.key === 'K' || e.code === 'KeyK')) {
            e.preventDefault();
            if (isPaletteOpen()) closePalette();
            else openPalette(defaultWorkspaceId);
        }
    }, true);
}

/**
 * Render the main app layout (header + sidebar + content area)
 */
function renderLayout() {
    const app = document.getElementById('app');
    app.innerHTML = `
        <!-- Header -->
        <header class="header" id="app-header">
            <div class="header__logo" id="header-logo" data-tooltip="На главную">
                <span class="header__logo-icon">${Icons.logo}</span>
                <span class="header__logo-text">Task<span class="header__logo-accent">Flow</span></span>
            </div>

            <nav class="header__nav">
                <button class="header__nav-btn" id="nav-workspaces">Пространства</button>
                <button class="header__nav-btn" id="nav-recent">Недавние</button>
                <button class="header__nav-btn" id="nav-starred">Избранное</button>
            </nav>

            <button class="header__create-btn" id="header-create-btn">+ Создать</button>

            <!-- Поиск — кнопка, открывающая палитру (Ctrl+K). Раньше здесь было
                 поле, которое искало только по названиям досок и только на
                 экране «Доски», а на остальных двенадцати экранах молчало. -->
            <button class="header__search" id="header-search" type="button">
                <span class="header__search-icon">${Icons.search}</span>
                <span class="header__search-text">Поиск по доскам и задачам</span>
                <kbd class="header__search-kbd">Ctrl K</kbd>
            </button>

            <div class="header__right">
                <!-- Идущий таймер: виден, только пока время идёт (см. timer.js) -->
                <button class="header-timer" id="header-timer" type="button" hidden>
                    <span class="header-timer__dot"></span>
                    <span class="header-timer__title"></span>
                    <span class="header-timer__time"></span>
                    <span class="header-timer__stop">${Icons.stop}</span>
                </button>
                <button class="header__icon-btn" id="btn-notifications" data-tooltip="Уведомления">
                    ${Icons.bell}
                </button>
                <button class="header__icon-btn" id="btn-help" data-tooltip="Справка">
                    ${Icons.help}
                </button>
                <div class="header__avatar" id="user-avatar" data-tooltip="Профиль">TF</div>
            </div>
        </header>

        <!-- Main -->
        <div class="main">
            <!-- Sidebar -->
            <!-- Пункты — кнопки, а не div: до них доходит Tab (12.11).
                 data-rail-label — подсказка, когда сайдбар свёрнут (sidebar.js). -->
            <aside class="sidebar" id="app-sidebar">
                <nav class="sidebar__section" aria-label="Разделы">
                    <button type="button" class="sidebar__item sidebar__item--active" id="sidebar-boards" data-rail-label="Доски">
                        <span class="sidebar__item-icon">${Icons.boards}</span>
                        <span class="sidebar__item-text">Доски</span>
                    </button>
                    <!-- Последняя открытая доска — возврат к ней в один щелчок, как
                         было у кнопки «Доска» в доке. Появляется после первой
                         открытой доски (showCurrentBoardItem). -->
                    <button type="button" class="sidebar__item sidebar__item--sub" id="sidebar-current-board" hidden>
                        <span class="sidebar__item-icon">${Icons.grid}</span>
                        <span class="sidebar__item-text"></span>
                    </button>
                    <button type="button" class="sidebar__item" id="sidebar-list" data-rail-label="Список">
                        <span class="sidebar__item-icon">${Icons.list}</span>
                        <span class="sidebar__item-text">Список</span>
                    </button>
                    <button type="button" class="sidebar__item" id="sidebar-planner" data-rail-label="Планировщик">
                        <span class="sidebar__item-icon">${Icons.calendar}</span>
                        <span class="sidebar__item-text">Планировщик</span>
                    </button>
                    <button type="button" class="sidebar__item" id="sidebar-habits" data-rail-label="Привычки">
                        <span class="sidebar__item-icon">${Icons.habit}</span>
                        <span class="sidebar__item-text">Привычки</span>
                    </button>
                    <button type="button" class="sidebar__item" id="sidebar-inbox" data-rail-label="Inbox">
                        <span class="sidebar__item-icon">${Icons.inbox}</span>
                        <span class="sidebar__item-text">Inbox</span>
                    </button>
                    <button type="button" class="sidebar__item" id="sidebar-mistakes" data-rail-label="Требуют внимания">
                        <span class="sidebar__item-icon">${Icons.alertTriangle}</span>
                        <span class="sidebar__item-text">Требуют внимания</span>
                    </button>
                    <button type="button" class="sidebar__item" id="sidebar-assistant" data-rail-label="Ассистент">
                        <span class="sidebar__item-icon">${Icons.sparkles}</span>
                        <span class="sidebar__item-text">Ассистент</span>
                    </button>
                    <button type="button" class="sidebar__item" id="sidebar-templates" data-rail-label="Шаблоны">
                        <span class="sidebar__item-icon">${Icons.template}</span>
                        <span class="sidebar__item-text">Шаблоны</span>
                    </button>
                    <button type="button" class="sidebar__item" id="sidebar-home" data-rail-label="Главная страница">
                        <span class="sidebar__item-icon">${Icons.home}</span>
                        <span class="sidebar__item-text">Главная страница</span>
                    </button>
                    <button type="button" class="sidebar__item" id="sidebar-settings" data-rail-label="Настройки">
                        <span class="sidebar__item-icon">${Icons.settings}</span>
                        <span class="sidebar__item-text">Настройки</span>
                    </button>
                </nav>

                <div class="sidebar__section">
                    <div class="sidebar__section-title">Рабочие пространства</div>
                    <div id="sidebar-workspaces"></div>
                </div>

                <div class="sidebar__footer">
                    <button type="button" class="sidebar__collapse" id="sidebar-collapse" aria-expanded="true">
                        <span class="sidebar__item-icon">${Icons.panelLeft}</span>
                        <span class="sidebar__collapse-text">Свернуть</span>
                    </button>
                </div>
            </aside>

            <!-- Content -->
            <div class="content" id="content"></div>
        </div>
    `;

    // Event: Logo click → go to hub
    $('#header-logo').addEventListener('click', () => {
        navigateTo('hub', { workspaceId: defaultWorkspaceId });
    });

    // Event: Create button
    $('#header-create-btn').addEventListener('click', () => {
        if (currentView.name === 'hub') {
            // Trigger create board modal from hub
            const createBtn = $('#create-board-btn');
            if (createBtn) createBtn.click();
        } else {
            // Navigate to hub first
            navigateTo('hub', { workspaceId: defaultWorkspaceId });
        }
    });

    // Поиск в шапке — тот же, что по Ctrl+K: по доскам и карточкам всего
    // пространства, с любого экрана.
    $('#header-search').addEventListener('click', () => {
        if (!isPaletteOpen()) openPalette(defaultWorkspaceId);
    });

    // Event: Sidebar navigation
    $('#sidebar-boards').addEventListener('click', () => {
        navigateTo('hub', { workspaceId: defaultWorkspaceId });
    });

    $('#sidebar-current-board').addEventListener('click', () => {
        if (lastBoardId) navigateTo('board', { boardId: lastBoardId });
    });

    $('#sidebar-list').addEventListener('click', () => {
        navigateTo('list', { workspaceId: defaultWorkspaceId });
    });

    $('#sidebar-planner').addEventListener('click', () => {
        navigateTo('planner', { workspaceId: defaultWorkspaceId });
    });

    $('#sidebar-habits').addEventListener('click', () => {
        navigateTo('habits', { workspaceId: defaultWorkspaceId });
    });

    $('#sidebar-inbox').addEventListener('click', () => {
        navigateTo('inbox', { workspaceId: defaultWorkspaceId });
    });

    $('#sidebar-mistakes').addEventListener('click', () => {
        navigateTo('mistakes', { workspaceId: defaultWorkspaceId });
    });

    $('#sidebar-assistant').addEventListener('click', () => {
        navigateTo('assistant', { workspaceId: defaultWorkspaceId });
    });

    $('#sidebar-templates').addEventListener('click', () => {
        navigateTo('templates', { workspaceId: defaultWorkspaceId });
    });

    $('#sidebar-home').addEventListener('click', () => {
        navigateTo('home', { workspaceId: defaultWorkspaceId });
    });

    $('#sidebar-settings').addEventListener('click', () => {
        navigateTo('settings', { workspaceId: defaultWorkspaceId });
    });
}

/**
 * Пункт «последняя открытая доска» под «Досками»: название доски — подписью,
 * а в свёрнутом сайдбаре — подсказкой. Название читается отдельно: экран
 * доски его не отдаёт, а лишний запрос по id дешевле, чем тянуть его оттуда.
 */
async function showCurrentBoardItem(boardId) {
    const item = $('#sidebar-current-board');
    if (!item) return;
    try {
        const board = await api.getBoard(boardId);
        if (boardId !== lastBoardId) return; // пока читали, открыли другую
        item.querySelector('.sidebar__item-text').textContent = board.name;
        item.dataset.railLabel = `Доска «${board.name}»`;
        item.setAttribute('aria-label', `Доска «${board.name}»`);
        if (item.closest('.sidebar--rail')) item.dataset.tooltip = item.dataset.railLabel;
        item.hidden = false;
    } catch (e) {
        // Доску удалили или она недоступна — пункт просто не показывается.
        item.hidden = true;
    }
}

/**
 * Highlight the active top-level sidebar item for the given view
 */
function updateSidebarActiveState(view) {
    document.querySelectorAll('.sidebar__item').forEach(item => item.classList.remove('sidebar__item--active'));
    const id = SIDEBAR_ITEM_IDS[view];
    if (id) {
        const el = document.getElementById(id);
        if (el) el.classList.add('sidebar__item--active');
    }
}

/**
 * Navigate between views. Single choke point: fully re-renders only the
 * central content area (and the sidebar around it) — never the whole app.
 */
async function navigateTo(view, params = {}) {
    if (!ROUTES[view]) return;

    if (params.workspaceId) defaultWorkspaceId = params.workspaceId;
    currentView = { name: view, params };

    updateSidebarActiveState(view);

    // Оформление, зависящее от экрана, живёт в CSS, а не в JS: слой фона
    // (#app-bg) сам не знает, какая страница открыта, и знать не должен.
    // Сейчас от этого атрибута зависит только доска — она показывает фон
    // без размытия.
    $('#app').dataset.view = view;

    // Сайдбар виден на всех экранах; на доске он по умолчанию свёрнут до
    // иконок — колонкам нужна ширина (sidebar.js).
    setSidebarContext(view);
    await renderWorkspaceSidebar(defaultWorkspaceId);

    // Фон принадлежит пространству, а не экрану, поэтому пересчитывается на
    // каждом переходе: `defaultWorkspaceId` только что мог смениться. Не
    // дожидаемся — оформление не должно задерживать отрисовку страницы.
    applyWorkspaceBackground(defaultWorkspaceId);

    try {
        await ROUTES[view](params);
    } catch (error) {
        console.error(`Failed to render view "${view}":`, error);
        showToast('Ошибка загрузки страницы', 'error');
    }
}

// Start the app when DOM is ready
if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
} else {
    init();
}
