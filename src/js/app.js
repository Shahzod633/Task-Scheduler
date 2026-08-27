// ============================================
// TaskFlow — Main Application Entry Point
// ============================================

import * as api from './api.js';
import Icons from './icons.js';
import { renderBoard } from './board.js';
import { renderHub, filterBoards } from './hub.js';
import { renderDock, showDock, removeDock } from './dock.js';
import { renderWorkspaceSidebar, ensureDefaultWorkspace } from './workspace.js';
import { initHeaderInteractions } from './header.js';
import { initTooltips } from './popover.js';
import { initModalEscape } from './dialog.js';
import { renderTemplatesPage } from './templates.js';
import { renderHomePage } from './home.js';
import { renderSettingsPage } from './settings.js';
import { renderInboxPage } from './inbox.js';
import { renderPlannerPage } from './planner.js';
import { renderRecentPage } from './recent.js';
import { renderFavoritesPage } from './favorites.js';
import { renderMistakesPage } from './mistakes.js';
import { renderListPage } from './list.js';
import { renderMembersPage } from './members.js';
import { applyWorkspaceBackground } from './background.js';
import { $, debounce, showToast } from './utils.js';

let currentView = { name: 'hub', params: {} };
let defaultWorkspaceId = null;
let lastBoardId = null;

// Views that show the left sidebar (workspace/hub-style navigation)
const SIDEBAR_VIEWS = new Set(['hub', 'templates', 'home', 'settings', 'recent', 'favorites', 'mistakes', 'list', 'members']);
// Views that show the floating dock (board working context)
const DOCK_VIEWS = new Set(['board', 'inbox', 'planner']);
// view name -> sidebar top-level item id
const SIDEBAR_ITEM_IDS = { hub: 'sidebar-boards', list: 'sidebar-list', templates: 'sidebar-templates', home: 'sidebar-home', settings: 'sidebar-settings' };

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
    },
    templates: (params) => renderTemplatesPage(params.workspaceId || defaultWorkspaceId),
    home: (params) => renderHomePage(params.workspaceId || defaultWorkspaceId),
    settings: (params) => renderSettingsPage(params.workspaceId || defaultWorkspaceId),
    inbox: (params) => renderInboxPage(params.workspaceId || defaultWorkspaceId),
    planner: (params) => renderPlannerPage(params.workspaceId || defaultWorkspaceId),
    recent: (params) => renderRecentPage(params.workspaceId || defaultWorkspaceId),
    favorites: (params) => renderFavoritesPage(params.workspaceId || defaultWorkspaceId),
    mistakes: (params) => renderMistakesPage(params.workspaceId || defaultWorkspaceId),
    list: (params) => renderListPage(params.workspaceId || defaultWorkspaceId),
    members: (params) => renderMembersPage(params.workspaceId || defaultWorkspaceId),
};

/**
 * Initialize the application
 */
async function init() {
    try {
        // Ensure we have at least one workspace
        defaultWorkspaceId = await ensureDefaultWorkspace();

        // Render the global layout
        renderLayout();
        initHeaderInteractions(() => defaultWorkspaceId);
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

    } catch (error) {
        console.error('Failed to initialize app:', error);
        showToast('Ошибка инициализации приложения', 'error');
    }
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

            <div class="header__search">
                <span class="header__search-icon">${Icons.search}</span>
                <input type="text" class="header__search-input" id="search-input" placeholder="Поиск">
            </div>

            <div class="header__right">
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
            <aside class="sidebar" id="app-sidebar">
                <div class="sidebar__section">
                    <div class="sidebar__item sidebar__item--active" id="sidebar-boards">
                        <span class="sidebar__item-icon">${Icons.boards}</span>
                        <span class="sidebar__item-text">Доски</span>
                    </div>
                    <div class="sidebar__item" id="sidebar-list">
                        <span class="sidebar__item-icon">${Icons.list}</span>
                        <span class="sidebar__item-text">Список</span>
                    </div>
                    <div class="sidebar__item" id="sidebar-templates">
                        <span class="sidebar__item-icon">${Icons.template}</span>
                        <span class="sidebar__item-text">Шаблоны</span>
                    </div>
                    <div class="sidebar__item" id="sidebar-home">
                        <span class="sidebar__item-icon">${Icons.home}</span>
                        <span class="sidebar__item-text">Главная страница</span>
                    </div>
                    <div class="sidebar__item" id="sidebar-settings">
                        <span class="sidebar__item-icon">${Icons.settings}</span>
                        <span class="sidebar__item-text">Настройки</span>
                    </div>
                </div>

                <div class="sidebar__section">
                    <div class="sidebar__section-title">Рабочие пространства</div>
                    <div id="sidebar-workspaces"></div>
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

    // Event: Search
    const searchInput = $('#search-input');
    searchInput.addEventListener('input', debounce((e) => {
        if (currentView.name === 'hub') {
            filterBoards(e.target.value);
        }
    }, 200));

    // Event: Sidebar navigation
    $('#sidebar-boards').addEventListener('click', () => {
        navigateTo('hub', { workspaceId: defaultWorkspaceId });
    });

    $('#sidebar-list').addEventListener('click', () => {
        navigateTo('list', { workspaceId: defaultWorkspaceId });
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
 * central content area (and the dock/sidebar chrome around it) — never the
 * whole app.
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

    const sidebar = $('#app-sidebar');
    if (sidebar) sidebar.classList.toggle('sidebar--hidden', !SIDEBAR_VIEWS.has(view));

    if (SIDEBAR_VIEWS.has(view)) {
        await renderWorkspaceSidebar(defaultWorkspaceId);
    }

    // Фон принадлежит пространству, а не экрану, поэтому пересчитывается на
    // каждом переходе: `defaultWorkspaceId` только что мог смениться. Не
    // дожидаемся — оформление не должно задерживать отрисовку страницы.
    applyWorkspaceBackground(defaultWorkspaceId);

    if (DOCK_VIEWS.has(view)) {
        renderDock(view);
        showDock();
    } else {
        removeDock();
    }

    const searchInput = $('#search-input');
    if (searchInput) searchInput.value = '';

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
