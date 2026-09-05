// ============================================
// TaskFlow — Chart helpers (thin wrappers around the vendored Chart.js)
// ============================================
// `Chart` is a global provided by the classic <script src="vendor/chart.js">
// tag in index.html (same pattern as `Sortable` used directly in board.js).

// Chart.js рисует на canvas и CSS-переменных не видит, поэтому цвета
// вычитываются из темы в момент отрисовки, а не хранятся копией рядом. Копия
// и была здесь раньше — восемь значений, «синхронизированных с variables.css»
// на честном слове; со сменой темы такая копия расходится молча.
function token(name, fallback) {
    const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
    return value || fallback;
}

/** Цвета рядов данных по имени, а не значением: значение зависит от темы. */
const SERIES_TOKENS = {
    accent: ['--accent-color', '#6c5cff'],
    danger: ['--color-danger', '#f4685f'],
    success: ['--color-success', '#3ecf8e'],
};

function seriesColor(name) {
    const pair = SERIES_TOKENS[name] || SERIES_TOKENS.accent;
    return token(pair[0], pair[1]);
}

// Общая «плавность» для всех графиков приложения
const baseAnimation = {
    duration: 620,
    easing: 'easeOutQuart',
};

function baseTooltip() {
    return {
        backgroundColor: token('--chart-tooltip-bg', 'rgba(23, 28, 38, 0.95)'),
        borderColor: token('--chart-tooltip-border', 'rgba(255, 255, 255, 0.08)'),
        borderWidth: 1,
        titleColor: token('--text-primary', '#eef1f7'),
        bodyColor: token('--text-secondary', '#9aa5b8'),
        padding: 10,
        cornerRadius: 8,
        displayColors: false,
    };
}

const chartInstances = new WeakMap();

/**
 * Живые графики и способ нарисовать каждый заново.
 *
 * Нужно ради смены темы: у построенного графика цвета уже разобраны Chart.js
 * во внутреннюю структуру, и «подкрасить» их снаружи нельзя — проще позвать ту
 * же функцию с теми же аргументами. Поэтому цвет ряда сюда приходит именем
 * ('danger'), а не готовой строкой: строка запомнила бы цвет прежней темы.
 */
const live = new Map();

function destroyExisting(canvas) {
    const existing = chartInstances.get(canvas);
    if (existing) existing.destroy();
}

window.addEventListener('themechange', () => {
    for (const [canvas, redraw] of live) {
        // Холст мог уехать вместе со страницей — перерисовывать нечего.
        if (!canvas.isConnected) {
            live.delete(canvas);
            continue;
        }
        redraw();
    }
});

export function renderBarChart(canvas, labels, data, opts = {}) {
    destroyExisting(canvas);
    live.set(canvas, () => renderBarChart(canvas, labels, data, opts));
    const textColor = token('--text-secondary', '#9aa5b8');
    const gridColor = token('--chart-grid', 'rgba(255, 255, 255, 0.06)');
    const barColor = seriesColor(opts.color);
    const chart = new Chart(canvas, {
        type: 'bar',
        data: {
            labels,
            datasets: [{
                label: opts.label || '',
                data,
                backgroundColor: barColor,
                hoverBackgroundColor: barColor,
                borderRadius: 6,
                borderSkipped: false,
                maxBarThickness: 30,
            }]
        },
        options: {
            responsive: true,
            maintainAspectRatio: false,
            animation: baseAnimation,
            plugins: {
                legend: { display: false },
                tooltip: baseTooltip(),
            },
            scales: {
                x: { grid: { display: false }, ticks: { color: textColor, font: { size: 11 } } },
                y: { beginAtZero: true, ticks: { color: textColor, precision: 0 }, grid: { color: gridColor } },
            }
        }
    });
    chartInstances.set(canvas, chart);
    return chart;
}

export function renderLineChart(canvas, labels, series, opts = {}) {
    destroyExisting(canvas);
    live.set(canvas, () => renderLineChart(canvas, labels, series, opts));
    const textColor = token('--text-secondary', '#9aa5b8');
    const gridColor = token('--chart-grid', 'rgba(255, 255, 255, 0.06)');
    const chart = new Chart(canvas, {
        type: 'line',
        data: {
            labels,
            datasets: series.map(s => ({
                label: s.label,
                data: s.data,
                borderColor: seriesColor(s.color),
                backgroundColor: seriesColor(s.color),
                borderWidth: 2,
                tension: 0.38,
                pointRadius: 0,
                pointHoverRadius: 5,
                pointHitRadius: 14,
                fill: false,
            }))
        },
        options: {
            responsive: true,
            maintainAspectRatio: false,
            animation: baseAnimation,
            interaction: { mode: 'index', intersect: false },
            plugins: {
                legend: {
                    display: series.length > 1,
                    labels: { color: textColor, boxWidth: 10, boxHeight: 10, usePointStyle: true, padding: 16 }
                },
                tooltip: { ...baseTooltip(), displayColors: series.length > 1 },
            },
            scales: {
                x: { grid: { display: false }, ticks: { color: textColor, font: { size: 11 } } },
                y: { beginAtZero: true, ticks: { color: textColor, precision: 0 }, grid: { color: gridColor } },
            }
        }
    });
    chartInstances.set(canvas, chart);
    return chart;
}
