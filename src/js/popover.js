// ============================================
// TaskFlow — Popover Layer
// ============================================
// Единый слой для всего, что всплывает поверх интерфейса: подсказки
// (data-tooltip), выпадающие панели шапки, контекстные меню.
//
// ЗАЧЕМ СЛОЙ, А НЕ ПРОСТО z-index
// Контентная область (.content) и .main объявлены с overflow: hidden ради
// собственного скролла. Любой попап, который физически лежит внутри них,
// обрезается их границами — z-index на клиппинг не влияет ВООБЩЕ. Именно
// на это напоролась подсказка кнопок хедера доски: она рисуется над
// кнопкой, то есть выше верхней кромки .content, и срезалась целиком.
//
// Слой прикреплён напрямую к <body> и позиционируется fixed, поэтому его
// содержимое не обрезается ничем и не зависит от стековых контекстов
// родителей (transform / filter / opacity / backdrop-filter в разметке).
// Координаты считаются от триггера через getBoundingClientRect().

const VIEWPORT_MARGIN = 8;   // минимальный отступ от кромки окна
const ANCHOR_GAP = 8;        // зазор между триггером и попапом
const TOOLTIP_DELAY = 260;   // задержка показа подсказки, мс

let layerEl = null;

// Открытые сейчас попапы: node -> { anchor, opts, close }.
// Нужен, чтобы пересчитать позицию при ресайзе/скролле и не потерять
// слушатели закрытия.
const openPopovers = new Map();

/**
 * Возвращает (создавая при первом обращении) слой попапов в конце <body>.
 */
function getLayer() {
    if (layerEl && layerEl.isConnected) return layerEl;
    layerEl = document.createElement('div');
    layerEl.className = 'app-popover-layer';
    document.body.appendChild(layerEl);
    return layerEl;
}

function clamp(value, min, max) {
    // Попап выше/шире окна — прижимаем к верхнему/левому краю, иначе
    // Math.min(max) увёл бы его за противоположную кромку.
    if (max < min) return min;
    return Math.min(Math.max(value, min), max);
}

/**
 * Ставит элемент рядом с триггером в координатах окна.
 * Переворачивает попап на другую сторону, если с предпочтительной не
 * хватает места, и прижимает к видимой области.
 *
 * @param {HTMLElement} node - позиционируемый элемент (уже в слое)
 * @param {HTMLElement} anchor - элемент-триггер
 * @param {{placement?: 'bottom'|'top', align?: 'center'|'start'|'end', gap?: number}} opts
 * @returns {'bottom'|'top'} фактическая сторона
 */
export function placeAnchored(node, anchor, opts = {}) {
    const { placement = 'bottom', align = 'center', gap = ANCHOR_GAP } = opts;

    const rect = anchor.getBoundingClientRect();

    // Абсолютное позиционирование выставляем ДО измерения. Пока элемент
    // статический, блочный div растягивается на всю ширину слоя (то есть
    // окна), и измеренная ширина оказалась бы равна ширине экрана — попап
    // «не влезал» бы никуда и прижимался к левому краю вместо триггера.
    node.style.position = 'absolute';

    // offsetWidth/offsetHeight — layout-метрики: в отличие от
    // getBoundingClientRect они не учитывают transform, поэтому измерение
    // корректно даже во время анимации появления попапа.
    const width = node.offsetWidth;
    const height = node.offsetHeight;
    const viewportWidth = document.documentElement.clientWidth;
    const viewportHeight = document.documentElement.clientHeight;

    const spaceBelow = viewportHeight - rect.bottom - gap - VIEWPORT_MARGIN;
    const spaceAbove = rect.top - gap - VIEWPORT_MARGIN;

    let side = placement;
    if (side === 'bottom' && height > spaceBelow && spaceAbove > spaceBelow) side = 'top';
    else if (side === 'top' && height > spaceAbove && spaceBelow > spaceAbove) side = 'bottom';

    const rawTop = side === 'top' ? rect.top - gap - height : rect.bottom + gap;
    const top = clamp(rawTop, VIEWPORT_MARGIN, viewportHeight - height - VIEWPORT_MARGIN);

    let rawLeft;
    if (align === 'start') rawLeft = rect.left;
    else if (align === 'end') rawLeft = rect.right - width;
    else rawLeft = rect.left + rect.width / 2 - width / 2;
    const left = clamp(rawLeft, VIEWPORT_MARGIN, viewportWidth - width - VIEWPORT_MARGIN);

    node.style.top = `${Math.round(top)}px`;
    node.style.left = `${Math.round(left)}px`;
    node.dataset.placement = side;

    return side;
}

/**
 * Открывает попап в общем слое: переносит узел в слой, позиционирует его
 * по триггеру и вешает закрытие по клику снаружи и по Escape.
 *
 * @param {HTMLElement} node - содержимое попапа (уже собранный DOM)
 * @param {HTMLElement} anchor - элемент-триггер
 * @param {object} opts - placement / align / gap / width / exclusive / onClose
 * @returns {() => void} функция закрытия
 */
export function openPopover(node, anchor, opts = {}) {
    const { width, exclusive = true, onClose } = opts;

    if (exclusive) closePopovers();
    if (width) node.style.width = `${width}px`;

    getLayer().appendChild(node);
    placeAnchored(node, anchor, opts);

    const close = () => {
        if (!openPopovers.has(node)) return;
        openPopovers.delete(node);
        document.removeEventListener('click', onDocumentClick);
        document.removeEventListener('keydown', onKeyDown);
        resizeObserver.disconnect();
        node.remove();
        if (onClose) onClose();
    };

    const onDocumentClick = (e) => {
        // Узел могли убрать напрямую через .remove() из обработчика пункта
        // меню — тогда просто снимаем слушатели.
        if (!node.isConnected) { close(); return; }
        if (node.contains(e.target)) return;
        close();
    };

    const onKeyDown = (e) => {
        if (e.key === 'Escape') close();
    };

    // Панель шапки дозагружает содержимое асинхронно и меняет высоту уже
    // после открытия — пересчитываем позицию, чтобы она не вылезла за
    // нижнюю кромку окна.
    const resizeObserver = new ResizeObserver(() => {
        if (node.isConnected) placeAnchored(node, anchor, opts);
    });
    resizeObserver.observe(node);

    // Клик, которым попап открыли, ещё всплывает — слушатель вешаем со
    // следующего тика, иначе попап закроется сразу же.
    setTimeout(() => {
        if (!openPopovers.has(node)) return;
        document.addEventListener('click', onDocumentClick);
        document.addEventListener('keydown', onKeyDown);
    }, 0);

    openPopovers.set(node, { anchor, opts, close });
    return close;
}

/**
 * Закрывает все открытые попапы (подсказки не трогает).
 */
export function closePopovers() {
    for (const entry of [...openPopovers.values()]) entry.close();
}

/**
 * Пересчитывает позиции открытых попапов. Попап, чей триггер исчез из
 * DOM (например, доска перерисовалась), закрывается.
 */
function repositionPopovers() {
    for (const [node, entry] of [...openPopovers]) {
        if (!node.isConnected || !entry.anchor.isConnected) {
            entry.close();
            continue;
        }
        placeAnchored(node, entry.anchor, entry.opts);
    }
}

// ─── Подсказки (data-tooltip) ───────────────────────────────────────────
// Раньше подсказка была псевдоэлементом ::after внутри самой кнопки —
// поэтому и обрезалась overflow-контейнерами. Теперь это один
// переиспользуемый узел в слое, позиционируемый по триггеру.

let tooltipEl = null;
let tooltipTrigger = null;
let tooltipTimer = 0;

function getTooltipEl() {
    if (tooltipEl && tooltipEl.isConnected) return tooltipEl;
    tooltipEl = document.createElement('div');
    tooltipEl.className = 'tooltip';
    tooltipEl.setAttribute('role', 'tooltip');
    getLayer().appendChild(tooltipEl);
    return tooltipEl;
}

function showTooltip(trigger) {
    const text = trigger.dataset.tooltip;
    if (!text || !trigger.isConnected) return;

    const el = getTooltipEl();
    el.textContent = text;
    el.classList.remove('tooltip--visible');
    // Позиционируем до показа: элемент всегда в DOM (скрыт прозрачностью),
    // поэтому его размеры уже измеримы.
    placeAnchored(el, trigger, { placement: 'bottom', align: 'center' });
    // Принудительный reflow фиксирует стартовое состояние перехода до того,
    // как навесим класс, иначе браузер схлопнет смену координат и появление
    // в одну перерисовку и анимация не проиграется. rAF здесь не годится:
    // в фоновой вкладке кадры не выдаются и подсказка так и осталась бы
    // прозрачной.
    void el.offsetWidth;
    el.classList.add('tooltip--visible');
    tooltipTrigger = trigger;
}

function hideTooltip() {
    clearTimeout(tooltipTimer);
    tooltipTrigger = null;
    if (tooltipEl) tooltipEl.classList.remove('tooltip--visible');
}

/**
 * Включает подсказки для всех элементов с data-tooltip.
 * Слушатели делегированы на document, поэтому работают и для элементов,
 * созданных позже (доска и страницы перерисовываются целиком).
 */
export function initTooltips() {
    document.addEventListener('pointerover', (e) => {
        // Триггер мог исчезнуть вместе с перерисовкой экрана, пока
        // подсказка висела — снимаем её при первом же движении курсора.
        if (tooltipTrigger && !tooltipTrigger.isConnected) hideTooltip();

        const trigger = e.target.closest?.('[data-tooltip]');
        if (!trigger || trigger === tooltipTrigger) return;

        hideTooltip();
        tooltipTimer = setTimeout(() => showTooltip(trigger), TOOLTIP_DELAY);
    });

    document.addEventListener('pointerout', (e) => {
        const trigger = e.target.closest?.('[data-tooltip]');
        if (!trigger) return;
        // Переход на вложенный svg/span внутри того же триггера уходом не
        // считается.
        if (e.relatedTarget && trigger.contains(e.relatedTarget)) return;
        hideTooltip();
    });

    // Клавиатурная навигация: по фокусу показываем сразу, без задержки.
    document.addEventListener('focusin', (e) => {
        const trigger = e.target.closest?.('[data-tooltip]');
        if (!trigger) return;
        clearTimeout(tooltipTimer);
        showTooltip(trigger);
    });
    document.addEventListener('focusout', hideTooltip);

    // Подсказка не должна пережить нажатие, скролл или уход из окна.
    document.addEventListener('pointerdown', hideTooltip, true);
    document.addEventListener('keydown', (e) => { if (e.key === 'Escape') hideTooltip(); });
    window.addEventListener('blur', hideTooltip);
}

// ─── Реакция на изменение геометрии окна ───
// Ресайз окна и скролл контейнеров (доска ездит по горизонтали) сдвигают
// триггер — попапы переставляем, подсказку просто убираем.
window.addEventListener('resize', () => {
    hideTooltip();
    repositionPopovers();
});

// scroll не всплывает, поэтому слушаем в фазе перехвата.
document.addEventListener('scroll', () => {
    hideTooltip();
    repositionPopovers();
}, true);
