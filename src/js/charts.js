// ============================================
// TaskFlow — Chart helpers (thin wrappers around the vendored Chart.js)
// ============================================
// `Chart` is a global provided by the classic <script src="vendor/chart.js">
// tag in index.html (same pattern as `Sortable` used directly in board.js).

// Значения синхронизированы с css/variables.css — Chart.js рисует на canvas
// и не видит CSS-переменные.
const gridColor = 'rgba(255, 255, 255, 0.06)';
const textColor = '#9aa5b8';

export const chartColors = {
    accent: '#6c5cff',
    danger: '#f4685f',
    success: '#3ecf8e',
};

// Общая «плавность» для всех графиков приложения
const baseAnimation = {
    duration: 620,
    easing: 'easeOutQuart',
};

const baseTooltip = {
    backgroundColor: 'rgba(23, 28, 38, 0.95)',
    borderColor: 'rgba(255, 255, 255, 0.08)',
    borderWidth: 1,
    titleColor: '#eef1f7',
    bodyColor: '#9aa5b8',
    padding: 10,
    cornerRadius: 8,
    displayColors: false,
};

const chartInstances = new WeakMap();

function destroyExisting(canvas) {
    const existing = chartInstances.get(canvas);
    if (existing) existing.destroy();
}

export function renderBarChart(canvas, labels, data, opts = {}) {
    destroyExisting(canvas);
    const chart = new Chart(canvas, {
        type: 'bar',
        data: {
            labels,
            datasets: [{
                label: opts.label || '',
                data,
                backgroundColor: opts.color || chartColors.accent,
                hoverBackgroundColor: opts.color || chartColors.accent,
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
                tooltip: baseTooltip,
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
    const chart = new Chart(canvas, {
        type: 'line',
        data: {
            labels,
            datasets: series.map(s => ({
                label: s.label,
                data: s.data,
                borderColor: s.color,
                backgroundColor: s.color,
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
                tooltip: { ...baseTooltip, displayColors: series.length > 1 },
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
