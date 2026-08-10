// ============================================
// TaskFlow — Main Application Entry Point
// ============================================

import * as api from './api.js';
import Icons from './icons.js';
import { renderBoard, getCurrentBoardId } from './board.js';
import { renderHub, filterBoards, getCurrentWorkspaceId } from './hub.js';
import { renderDock, showDock, hideDock, removeDock } from './dock.js';
import { renderWorkspaceSidebar, ensureDefaultWorkspace } from './workspace.js';
import { $, debounce, showToast } from './utils.js';

let currentView = 'hub'; // 'hub' | 'board'
let defaultWorkspaceId = null;

/**
 * Initialize the application
 */
async function init() {
    try {
        // Ensure we have at least one workspace
        defaultWorkspaceId = await ensureDefaultWorkspace();
        
        // Render the global layout
        renderLayout();
        
        // Render sidebar
        await renderWorkspaceSidebar(defaultWorkspaceId);
        
        // Start at hub view
        await navigateTo('hub', { workspaceId: defaultWorkspaceId });
        
        // Listen for navigation events
        window.addEventListener('navigate', (e) => {
            const { view, boardId, workspaceId } = e.detail;
            navigateTo(view, { boardId, workspaceId });
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
            <div class="header__logo" id="header-logo">
                <span class="header__logo-icon">${Icons.logo}</span>
                <span class="header__logo-text">TaskFlow</span>
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
                    <div class="sidebar__item" id="sidebar-templates">
                        <span class="sidebar__item-icon">${Icons.template}</span>
                        <span class="sidebar__item-text">Шаблоны</span>
                    </div>
                    <div class="sidebar__item" id="sidebar-home">
                        <span class="sidebar__item-icon">${Icons.home}</span>
                        <span class="sidebar__item-text">Главная страница</span>
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
        if (currentView === 'hub') {
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
        if (currentView === 'hub') {
            filterBoards(e.target.value);
        }
    }, 200));
    
    // Event: Sidebar navigation
    $('#sidebar-boards').addEventListener('click', () => {
        navigateTo('hub', { workspaceId: defaultWorkspaceId });
    });
    
    $('#sidebar-home').addEventListener('click', () => {
        navigateTo('hub', { workspaceId: defaultWorkspaceId });
    });
}

/**
 * Navigate between views
 */
async function navigateTo(view, params = {}) {
    currentView = view;
    
    // Update sidebar active state
    const sidebarItems = document.querySelectorAll('.sidebar__item');
    sidebarItems.forEach(item => item.classList.remove('sidebar__item--active'));
    
    if (view === 'hub') {
        const sidebarBoards = $('#sidebar-boards');
        if (sidebarBoards) sidebarBoards.classList.add('sidebar__item--active');
        
        const workspaceId = params.workspaceId || defaultWorkspaceId;
        defaultWorkspaceId = workspaceId;
        
        // Show sidebar
        const sidebar = $('#app-sidebar');
        if (sidebar) sidebar.classList.remove('sidebar--hidden');
        
        removeDock();
        await renderHub(workspaceId);
        await renderWorkspaceSidebar(workspaceId);
        
        // Clear search
        const searchInput = $('#search-input');
        if (searchInput) searchInput.value = '';
        
    } else if (view === 'board') {
        const boardId = params.boardId;
        if (!boardId) return;
        
        // Hide sidebar for more board space
        const sidebar = $('#app-sidebar');
        if (sidebar) sidebar.classList.add('sidebar--hidden');
        
        await renderBoard(boardId);
        renderDock();
        showDock();
    }
}

// Start the app when DOM is ready
if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
} else {
    init();
}
