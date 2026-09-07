// ============================================
// TaskFlow — Markdown → HTML
// ============================================
// Свой парсер, а не библиотека с CDN: приложение офлайновое, единственное
// сетевое обращение во всём коде — исходящий SMTP (PROJECT_NOTES §5), и
// заводить второе ради разметки заметок не следует. Класть библиотеку в
// `lib/` тоже незачем — здесь нужно подмножество разметки на полторы сотни
// строк, а не полный CommonMark.
//
// Поддерживается: заголовки (#..######), списки маркированные и нумерованные,
// цитаты, блоки кода (``` и отступом), горизонтальная черта, таблицы не
// поддерживаются сознательно, из строчного — **жирный**, *курсив*,
// `код`, ~~зачёркнутый~~, [ссылка](url) и голые ссылки.
//
// ── Безопасность ──
// Текст экранируется ПЕРВЫМ делом, до любого разбора, и обратно в HTML
// превращается только то, что породил сам парсер. Заметку пишет владелец
// приложения и злоумышленника здесь нет, но заметка попадает в резервную
// копию и экспорт, а оттуда — в чужую установку.

/** `&`, `<`, `>`, кавычки → сущности. Ровно то же, что `escapeHtml` в utils. */
function esc(str) {
    return String(str)
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;')
        .replace(/"/g, '&quot;')
        .replace(/'/g, '&#39;');
}

/**
 * Ссылки: разрешены только http/https и mailto.
 *
 * Отдельная проверка, потому что `javascript:` в `[текст](...)` экранирование
 * скобок не ловит — там нет ни одного опасного символа.
 */
function safeHref(url) {
    const trimmed = String(url).trim();
    if (/^(https?:\/\/|mailto:)/i.test(trimmed)) return esc(trimmed);
    return null;
}

/** Строчная разметка внутри уже экранированного текста. */
function inline(text) {
    let out = text;

    // Код идёт первым и его содержимое дальше не разбирается: `**` внутри
    // обратных кавычек — это две звёздочки, а не жирный шрифт. Вырезаем
    // фрагменты в заглушки, а в конце возвращаем на место.
    const codes = [];
    out = out.replace(/`([^`]+)`/g, (_, code) => {
        codes.push(`<code>${code}</code>`);
        return `\u0000CODE${codes.length - 1}\u0000`;
    });

    // [текст](url) — до голых ссылок, иначе url внутри скобок съедается первым.
    out = out.replace(/\[([^\]]+)\]\(([^)\s]+)\)/g, (whole, label, url) => {
        const href = safeHref(url);
        // Ссылка с неразрешённой схемой остаётся видимым текстом, а не молча
        // исчезает: пропажа куска заметки хуже неработающей ссылки.
        return href ? `<a href="${href}" target="_blank" rel="noreferrer noopener">${label}</a>` : whole;
    });

    // Голые ссылки — только вне уже собранных <a ...>.
    out = out.replace(/(^|[\s(])(https?:\/\/[^\s<)]+)/g, (_, before, url) =>
        `${before}<a href="${esc(url)}" target="_blank" rel="noreferrer noopener">${url}</a>`);

    out = out
        .replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>')
        .replace(/(^|[^*])\*([^*\n]+)\*/g, '$1<em>$2</em>')
        .replace(/~~([^~]+)~~/g, '<del>$1</del>');

    return out.replace(/\u0000CODE(\d+)\u0000/g, (_, i) => codes[+i]);
}

/** Маркер списка в начале строки: `- `, `* `, `+ ` или `1. `. */
function listMarker(line) {
    const bullet = line.match(/^\s{0,3}[-*+]\s+(.*)$/);
    if (bullet) return { type: 'ul', text: bullet[1] };
    const ordered = line.match(/^\s{0,3}\d+[.)]\s+(.*)$/);
    if (ordered) return { type: 'ol', text: ordered[1] };
    return null;
}

/**
 * Markdown → HTML.
 *
 * Разбор построчный и однопроходный: заметка — это страница текста, а не
 * документ, на котором стоило бы экономить.
 */
export function renderMarkdown(source) {
    const lines = esc(String(source ?? '')).split(/\r?\n/);
    const html = [];

    let list = null;        // 'ul' | 'ol' — открытый список
    let paragraph = [];     // накопленные строки абзаца
    let quote = [];         // накопленные строки цитаты

    const closeParagraph = () => {
        if (!paragraph.length) return;
        // Перевод строки внутри абзаца сохраняется: в заметках его ставят
        // осознанно, а не по случайному переносу, как в вёрстке статьи.
        html.push(`<p>${inline(paragraph.join('<br>'))}</p>`);
        paragraph = [];
    };
    const closeList = () => {
        if (!list) return;
        html.push(`</${list}>`);
        list = null;
    };
    const closeQuote = () => {
        if (!quote.length) return;
        html.push(`<blockquote>${quote.map(q => inline(q)).join('<br>')}</blockquote>`);
        quote = [];
    };
    const closeAll = () => { closeParagraph(); closeList(); closeQuote(); };

    for (let i = 0; i < lines.length; i++) {
        const line = lines[i];

        // ─── Блок кода в тройных кавычках ───
        const fence = line.match(/^\s{0,3}```\s*([\w+-]*)\s*$/);
        if (fence) {
            closeAll();
            const body = [];
            i++;
            while (i < lines.length && !/^\s{0,3}```\s*$/.test(lines[i])) {
                body.push(lines[i]);
                i++;
            }
            // Незакрытый блок дочитывается до конца текста, а не роняет разбор:
            // человек ещё печатает, а предпросмотр обновляется на каждой букве.
            const lang = fence[1] ? ` class="language-${fence[1]}"` : '';
            html.push(`<pre><code${lang}>${body.join('\n')}</code></pre>`);
            continue;
        }

        // ─── Пустая строка — конец любого блока ───
        if (!line.trim()) { closeAll(); continue; }

        // ─── Горизонтальная черта ───
        if (/^\s{0,3}([-*_])\s*(\1\s*){2,}$/.test(line)) {
            closeAll();
            html.push('<hr>');
            continue;
        }

        // ─── Заголовок ───
        const heading = line.match(/^\s{0,3}(#{1,6})\s+(.*)$/);
        if (heading) {
            closeAll();
            const level = heading[1].length;
            html.push(`<h${level}>${inline(heading[2].trim())}</h${level}>`);
            continue;
        }

        // ─── Цитата ───
        // Ищем `&gt;`, а не `>`: экранирование идёт первым, до разбора, и к
        // этому месту угловая скобка уже стала сущностью.
        const quoted = line.match(/^\s{0,3}&gt;\s?(.*)$/);
        if (quoted) {
            closeParagraph();
            closeList();
            quote.push(quoted[1]);
            continue;
        }
        closeQuote();

        // ─── Список ───
        const item = listMarker(line);
        if (item) {
            closeParagraph();
            if (list !== item.type) {
                closeList();
                html.push(`<${item.type}>`);
                list = item.type;
            }
            html.push(`<li>${inline(item.text)}</li>`);
            continue;
        }
        closeList();

        paragraph.push(line);
    }

    closeAll();
    return html.join('\n');
}
