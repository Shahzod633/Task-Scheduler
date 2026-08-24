// ============================================
// TaskFlow — Workspace background, one per workspace
// ============================================
// Switching workspaces switches the picture behind the whole window. It is one
// layer at the root of the app rather than a background inside any component:
// the sidebar does not exist on the board screen, and a picture that only some
// screens can show is not a property of the workspace, it is decoration on one
// panel.
//
// The picture also gets its own element rather than being a background on `#app`
// itself, because the blur must apply to the picture only: `filter: blur()` on a
// container takes all of its text with it.

import * as api from './api.js';
import { $ } from './utils.js';

/**
 * workspace id → data URL, or null for "asked, has none".
 *
 * The picture travels as base64, so without this the same few hundred kilobytes
 * would cross the IPC boundary on every click in the sidebar. Entries are
 * dropped explicitly when the picture is changed or reset — nothing else can
 * invalidate them, since the file is only ever written by this app.
 */
const cache = new Map();

/**
 * Which call to `applyWorkspaceBackground` is the current one.
 *
 * Reading a picture takes long enough to lose a race: switch to a workspace
 * whose image is not cached yet, switch back to one that is, and the slower read
 * would land last and paint the workspace the user has already left.
 */
let latestRequest = 0;

export function invalidateBackground(workspaceId) {
    cache.delete(workspaceId);
}

/**
 * The workspace's background as a data URL, or null. Shared with the settings
 * screen so the preview there and the sidebar itself read the same cache.
 */
export async function getBackgroundUrl(workspaceId) {
    return loadBackground(workspaceId);
}

async function loadBackground(workspaceId) {
    if (cache.has(workspaceId)) return cache.get(workspaceId);
    try {
        const url = await api.getWorkspaceBackground(workspaceId);
        cache.set(workspaceId, url || null);
        return url || null;
    } catch (e) {
        // A failed read must never take the sidebar down with it: fall back to
        // the plain dark panel and remember nothing, so a later attempt can
        // still succeed.
        console.error('Не удалось загрузить фон сайдбара:', e);
        return null;
    }
}

/**
 * Applies the given workspace's background to the whole window, removing
 * whatever was there before. Safe to call on every navigation, and independent
 * of which screen is currently rendered.
 */
export async function applyWorkspaceBackground(workspaceId) {
    const request = ++latestRequest;

    const url = workspaceId ? await loadBackground(workspaceId) : null;
    if (request !== latestRequest) return;

    const app = $('#app');
    if (!app) return;

    let layer = document.getElementById('app-bg');

    if (!url) {
        // No picture means no layer at all — the ordinary dark window, exactly
        // as it was before this feature existed. Removing it rather than hiding
        // it is what keeps a screen from getting stuck on the previous
        // workspace's photo.
        if (layer) layer.remove();
        return;
    }

    if (!layer) {
        layer = document.createElement('div');
        layer.id = 'app-bg';
        // First child of #app, before the header: the layer is fixed to the
        // viewport, so its position in the tree only decides paint order.
        app.insertBefore(layer, app.firstChild);
    }
    layer.style.setProperty('--app-bg-image', `url("${url}")`);
}
