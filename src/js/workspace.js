// ============================================
// TaskFlow — Workspace Manager (Screen Б)
// ============================================

import * as api from './api.js';
import Icons from './icons.js';
import { createElement, $, $$ } from './utils.js';

/**
 * Render sidebar workspace section
 */
export async function renderWorkspaceSidebar(workspaceId, isExpanded = true) {
    const workspaces = await api.getWorkspaces();
    const sidebarWorkspace = $('#sidebar-workspaces');
    if (!sidebarWorkspace) return;
    
    sidebarWorkspace.innerHTML = '';
    
    for (const ws of workspaces) {
        const wsBlock = createElement('div', { className: 'sidebar__workspace' });
        
        // Workspace header
        const header = createElement('div', {
            className: 'sidebar__workspace-header',
            dataset: { workspaceId: ws.id }
        });
        
        const icon = createElement('div', {
            className: 'sidebar__workspace-icon',
            style: { background: 'var(--gradient-1)' }
        }, ws.name.charAt(0).toUpperCase());
        header.appendChild(icon);
        
        const name = createElement('span', { className: 'sidebar__workspace-name' }, ws.name);
        header.appendChild(name);
        
        const toggle = createElement('span', {
            className: `sidebar__workspace-toggle ${ws.id === workspaceId && isExpanded ? 'sidebar__workspace-toggle--open' : ''}`,
            innerHTML: Icons.chevronRight
        });
        header.appendChild(toggle);
        
        wsBlock.appendChild(header);
        
        // Workspace submenu
        const menu = createElement('div', {
            className: `sidebar__workspace-menu ${ws.id === workspaceId && isExpanded ? 'sidebar__workspace-menu--open' : ''}`
        });
        
        // Boards
        const boardsItem = createElement('div', {
            className: `sidebar__item ${ws.id === workspaceId ? 'sidebar__item--active' : ''}`
        });
        boardsItem.appendChild(createElement('span', { className: 'sidebar__item-icon', innerHTML: Icons.boards }));
        boardsItem.appendChild(createElement('span', { className: 'sidebar__item-text' }, 'Доски'));
        boardsItem.addEventListener('click', () => {
            window.dispatchEvent(new CustomEvent('navigate', { detail: { view: 'hub', workspaceId: ws.id } }));
        });
        menu.appendChild(boardsItem);
        
        // Members
        const membersItem = createElement('div', { className: 'sidebar__item' });
        membersItem.appendChild(createElement('span', { className: 'sidebar__item-icon', innerHTML: Icons.users }));
        membersItem.appendChild(createElement('span', { className: 'sidebar__item-text' }, 'Участники'));
        const addMemberBtn = createElement('span', {
            className: 'sidebar__item-action',
            innerHTML: Icons.plus
        });
        membersItem.appendChild(addMemberBtn);
        menu.appendChild(membersItem);
        
        // Settings
        const settingsItem = createElement('div', { className: 'sidebar__item' });
        settingsItem.appendChild(createElement('span', { className: 'sidebar__item-icon', innerHTML: Icons.settings }));
        settingsItem.appendChild(createElement('span', { className: 'sidebar__item-text' }, 'Настройки'));
        menu.appendChild(settingsItem);
        
        wsBlock.appendChild(menu);
        
        // Toggle click handler
        header.addEventListener('click', () => {
            const isOpen = menu.classList.contains('sidebar__workspace-menu--open');
            menu.classList.toggle('sidebar__workspace-menu--open');
            toggle.classList.toggle('sidebar__workspace-toggle--open');
            
            if (!isOpen) {
                window.dispatchEvent(new CustomEvent('navigate', { detail: { view: 'hub', workspaceId: ws.id } }));
            }
        });
        
        sidebarWorkspace.appendChild(wsBlock);
    }
}

/**
 * Create initial workspace if none exists
 */
export async function ensureDefaultWorkspace() {
    const workspaces = await api.getWorkspaces();
    if (workspaces.length === 0) {
        const ws = await api.createWorkspace('Моё рабочее пространство');
        return ws.id;
    }
    return workspaces[0].id;
}
