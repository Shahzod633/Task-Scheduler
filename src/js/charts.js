// ============================================
// TaskFlow — Chart helpers (thin wrappers around the vendored Chart.js)
// ============================================
// `Chart` is a global provided by the classic <script src="vendor/chart.js">
// tag in index.html (same pattern as `Sortable` used directly in board.js).

const gridColor = 'rgba(255, 255, 255, 0.08)';
const textColor = '#9fadbc';

export const chartColors = {
    accent: '#0079bf',
    danger: '#eb5a46',
    success: '#61bd4f',
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
                borderRadius: 4,
                maxBarThickness: 28,
            }]
        },
        options: {
            responsive: true,
            maintainAspectRatio: false,
            plugins: { legend: { display: false } },
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
                tension: 0.3,
                pointRadius: 2,
                fill: false,
            }))
        },
        options: {
            responsive: true,
            maintainAspectRatio: false,
            plugins: {
                legend: { display: series.length > 1, labels: { color: textColor, boxWidth: 12 } }
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
