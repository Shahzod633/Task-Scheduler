// Verifies the date helpers in src/js/utils.js under a fixed timezone.
// Run once per zone via the TZ environment variable — the bugs being fixed only
// appear at certain offsets, so one zone proves nothing.
import { parseTimestamp, parseDueDate, toDateKey, todayKey, isOverdue, formatDueDate, lastNDays }
    from '../src/js/utils.js';

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

console.log(failures === 0 ? '\nВСЁ ПРОШЛО' : `\nПРОВАЛОВ: ${failures}`);
process.exit(failures === 0 ? 0 : 1);
