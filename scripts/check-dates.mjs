// Verifies the date helpers in src/js/utils.js under a fixed timezone.
// Run once per zone via the TZ environment variable — the bugs being fixed only
// appear at certain offsets, so one zone proves nothing.
import {
    parseTimestamp, parseDueDate, toDateKey, todayKey, toTimestamp, isOverdue, formatDueDate, lastNDays,
    formatDuration, formatClock, manualSessionStart,
} from '../src/js/utils.js';

const zone = process.env.TZ || '(системная)';
const offsetMin = -new Date().getTimezoneOffset();
let failures = 0;

function check(name, actual, expected) {
    const ok = String(actual) === String(expected);
    if (!ok) failures++;
    console.log(`${ok ? 'OK  ' : 'FAIL'} ${name}: ${actual}${ok ? '' : ` (ожидалось ${expected})`}`);
}

console.log(`\n=== TZ=${zone}, смещение ${offsetMin >= 0 ? '+' : ''}${offsetMin / 60} ч ===`);

// 1. Отметка времени из SQLite — это UTC.
const ts = parseTimestamp('2026-08-21 16:04:31');
check('отметка 16:04:31 UTC разобрана как момент', ts.toISOString(), '2026-08-21T16:04:31.000Z');

// Наивный new Date() на той же строке даёт другой момент везде, кроме UTC.
const naive = new Date('2026-08-21 16:04:31');
if (offsetMin !== 0) {
    check('старый разбор действительно ошибался',
        naive.getTime() !== ts.getTime(), 'true');
    check('  размер ошибки, минут', (naive - ts) / 60000, -offsetMin);
}

// 2. Срок — календарная дата, а не момент: день должен совпасть с написанным.
check('срок 2026-08-25 остаётся 25-м числом', toDateKey(parseDueDate('2026-08-25')), '2026-08-25');
check('  и через формат «Срок»', formatDueDate('2026-08-25').length > 0, 'true');

// Именно здесь ломался прежний код в зонах западнее Гринвича.
check('старый разбор срока (UTC-полночь) даёт тот же день?',
    toDateKey(new Date('2026-08-25')) === '2026-08-25',
    offsetMin >= 0 ? 'true' : 'false');

// 3. Ключ дня — местный, а не UTC.
const localKey = todayKey();
check('todayKey совпадает с местным календарём',
    localKey, toDateKey(new Date()));
check('lastNDays заканчивается сегодняшним днём', lastNDays(5).at(-1), localKey);
check('lastNDays возвращает нужное число дней', lastNDays(30).length, 30);
check('lastNDays идёт по возрастанию',
    lastNDays(5).every((d, i, a) => i === 0 || a[i - 1] < d), 'true');

// 4. Просрочка: задача на сегодня ещё не просрочена, вчерашняя — да.
const today = new Date();
const key = (shift) => toDateKey(new Date(today.getFullYear(), today.getMonth(), today.getDate() + shift));
check('срок «сегодня» не считается просроченным', isOverdue(key(0)), 'false');
check('срок «завтра» не считается просроченным', isOverdue(key(1)), 'false');
check('срок «вчера» считается просроченным', isOverdue(key(-1)), 'true');
check('пустой срок не просрочен', isOverdue(null), 'false');
check('мусор не ломает разбор', isOverdue('не дата'), 'false');

// 5. Формат срока рядом с сегодняшним днём.
check('formatDueDate(сегодня)', formatDueDate(key(0)), 'Сегодня');
check('formatDueDate(завтра)', formatDueDate(key(1)), 'Завтра');
check('formatDueDate(вчера)', formatDueDate(key(-1)), 'Вчера');

// 6. Момент для базы — UTC, и туда-обратно он не сдвигается ни на минуту.
check('toTimestamp пишет UTC', toTimestamp(new Date(Date.UTC(2026, 8, 10, 7, 30, 5))), '2026-09-10 07:30:05');
check('toTimestamp → parseTimestamp — тот же момент',
    parseTimestamp(toTimestamp(ts)).getTime(), ts.getTime());

// Сессия, вписанная руками: местный день и местное время суток → UTC → обратно.
// День обязан остаться тем, что человек выбрал в календаре, в любой зоне —
// в том числе там, где местная полночь по UTC приходится на вчера.
for (const [h, m] of [[0, 30], [12, 0], [23, 45]]) {
    const local = new Date(2026, 8, 10, h, m, 0);
    const back = parseTimestamp(toTimestamp(local));
    check(`местные 10.09 ${h}:${String(m).padStart(2, '0')} остаются 10-м числом`, toDateKey(back), '2026-09-10');
}

// 7. Куда ложится сессия, вписанная руками. «Сейчас» — местные 11.09 10:00.
const now = new Date(2026, 8, 11, 10, 0, 0);
const placed = (day, seconds) => manualSessionStart(day, seconds, now);
const dayOf = (date) => (date ? toDateKey(parseTimestamp(toTimestamp(date))) : 'null');

check('прошлый день: два часа кончаются в полночь после него',
    placed('2026-09-10', 7200)?.getTime(), new Date(2026, 8, 10, 22, 0, 0).getTime());
check('  и после записи в базу остаются 10-м числом', dayOf(placed('2026-09-10', 7200)), '2026-09-10');
check('прошлый день: целые сутки начинаются в его полночь', dayOf(placed('2026-09-10', 86400)), '2026-09-10');
check('сегодня: три часа кончаются сейчас',
    placed('2026-09-11', 3 * 3600)?.getTime(), new Date(2026, 8, 11, 7, 0, 0).getTime());
check('сегодня: одиннадцать часов к 10:00 не помещаются', placed('2026-09-11', 11 * 3600), 'null');
check('завтрашний день не принимается', placed('2026-09-12', 3600), 'null');
check('пустой день не принимается', placed('', 3600), 'null');

// Дни перевода часов: в сутках 23 или 25 часов, и «целые сутки» помещаются в
// такой день ровно тогда, когда в нём не меньше 24 часов. Какие из этих дат —
// дни перевода, зависит от зоны; проверка верна для любой.
for (const day of ['2026-03-08', '2026-03-29', '2026-10-25', '2026-11-01']) {
    const start = parseDueDate(day);
    const hours = (new Date(start.getFullYear(), start.getMonth(), start.getDate() + 1) - start) / 3600000;
    check(`${day}: в сутках ${hours} ч, сутки работы помещаются?`,
        manualSessionStart(day, 86400, new Date(2026, 11, 31)) !== null, hours >= 24 ? 'true' : 'false');
}

// 8. Длительность — словами и тикающим счётчиком.
check('formatDuration(0)', formatDuration(0), '0 мин');
check('formatDuration(30 с)', formatDuration(30), '< 1 мин');
check('formatDuration(25 мин)', formatDuration(1500), '25 мин');
check('formatDuration(1 ч ровно)', formatDuration(3600), '1 ч');
check('formatDuration(2 ч 15 мин 59 с)', formatDuration(8159), '2 ч 15 мин');
check('formatDuration(null)', formatDuration(null), '0 мин');
check('formatClock(0)', formatClock(0), '0:00:00');
check('formatClock(12 мин 34 с)', formatClock(754), '0:12:34');
check('formatClock больше суток', formatClock(94443), '26:14:03');

console.log(failures === 0 ? '\nВСЁ ПРОШЛО' : `\nПРОВАЛОВ: ${failures}`);
process.exit(failures === 0 ? 0 : 1);
