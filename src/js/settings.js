// ============================================
// TaskFlow — Settings Page
// (workspace settings + full-page profile form)
// ============================================

import * as api from './api.js';
import Icons from './icons.js';
import { renderProfileFields } from './header.js';
import { applyWorkspaceBackground, invalidateBackground, getBackgroundUrl } from './background.js';
import { confirmDialog } from './dialog.js';
import { createElement, $, showToast, pluralize } from './utils.js';

export async function renderSettingsPage(workspaceId) {
    const content = $('#content');
    content.innerHTML = '';
    content.classList.add('view-enter');

    const page = createElement('div', { className: 'page page--settings' });
    page.appendChild(createElement('h2', { className: 'page__title' }, 'Настройки'));

    try {
        const workspaces = await api.getWorkspaces();
        const workspace = workspaces.find(w => w.id === workspaceId);

        if (workspace) {
            page.appendChild(createElement('h3', { className: 'settings-section-title' }, 'Рабочее пространство'));
            const card = createElement('div', { className: 'settings-card' });

            const nameGroup = createElement('div', { className: 'form-group' });
            nameGroup.appendChild(createElement('label', { className: 'form-label' }, 'Название'));
            const nameInput = createElement('input', { className: 'form-input' });
            nameInput.value = workspace.name;
            nameGroup.appendChild(nameInput);
            card.appendChild(nameGroup);

            const visGroup = createElement('div', { className: 'form-group' });
            visGroup.appendChild(createElement('label', { className: 'form-label' }, 'Приватность'));
            const visSelect = createElement('select', { className: 'form-input' });
            const options = [['private', 'Приватное'], ['public', 'Публичное']];
            for (const [value, label] of options) {
                const opt = createElement('option', { value }, label);
                if (workspace.visibility === value) opt.setAttribute('selected', 'selected');
                visSelect.appendChild(opt);
            }
            visGroup.appendChild(visSelect);
            card.appendChild(visGroup);

            const saveBtn = createElement('button', { className: 'btn btn--primary' }, 'Сохранить');
            saveBtn.addEventListener('click', async () => {
                const name = nameInput.value.trim();
                if (!name) return;
                try {
                    await api.updateWorkspace(workspace.id, name, visSelect.value);
                    showToast('Пространство обновлено');
                    window.dispatchEvent(new CustomEvent('navigate', { detail: { view: 'settings', workspaceId } }));
                } catch (e) {
                    showToast('Ошибка сохранения', 'error');
                }
            });
            card.appendChild(saveBtn);

            page.appendChild(card);
        }
    } catch (e) {
        showToast('Ошибка загрузки пространства', 'error');
    }

    // Фон принадлежит пространству, поэтому стоит сразу за его настройками и
    // выше «Профиля» — тот один на всё приложение.
    page.appendChild(createElement('h3', { className: 'settings-section-title' }, 'Фон пространства'));
    const backgroundCard = createElement('div', { className: 'settings-card' });
    page.appendChild(backgroundCard);

    page.appendChild(createElement('h3', { className: 'settings-section-title' }, 'Профиль'));
    const profileCard = createElement('div', { className: 'settings-card' });
    page.appendChild(profileCard);

    page.appendChild(createElement('h3', { className: 'settings-section-title' }, 'Напоминания о дедлайнах'));
    const remindersCard = createElement('div', { className: 'settings-card' });
    page.appendChild(remindersCard);

    page.appendChild(createElement('h3', { className: 'settings-section-title' }, 'Email-напоминания'));
    const emailCard = createElement('div', { className: 'settings-card' });
    page.appendChild(emailCard);

    page.appendChild(createElement('h3', { className: 'settings-section-title' }, 'Резервные копии'));
    const backupCard = createElement('div', { className: 'settings-card' });
    page.appendChild(backupCard);

    content.appendChild(page);
    setTimeout(() => content.classList.remove('view-enter'), 420);

    renderBackgroundSection(backgroundCard, workspaceId);
    renderProfileFields(profileCard);
    renderRemindersSection(remindersCard);
    renderEmailSection(emailCard);
    renderBackupSection(backupCard);
}

/**
 * «Напоминания о дедлайнах» — уведомление Windows, когда до срока карточки
 * остаётся меньше выбранного времени.
 *
 * Настройка одна на всё приложение, а не на пространство: уведомление приходит
 * когда окно свёрнуто, и «какое пространство сейчас открыто» в этот момент
 * ничего не значит.
 */
async function renderRemindersSection(container) {
    container.appendChild(createElement('p', { className: 'form-hint' },
        'Срок карточки истекает в конце указанного дня. Напоминание приходит один раз ' +
        'и только пока приложение запущено — даже если окно свёрнуто.'));

    let settings;
    try {
        settings = await api.getReminderSettings();
    } catch (e) {
        container.appendChild(createElement('p', { className: 'form-hint' }, 'Не удалось прочитать настройки напоминаний'));
        return;
    }

    const toggleRow = createElement('label', { className: 'settings-reminders__toggle' });
    const toggle = createElement('input', { type: 'checkbox', className: 'settings-reminders__checkbox' });
    toggle.checked = settings.enabled;
    toggleRow.appendChild(toggle);
    toggleRow.appendChild(createElement('span', {}, 'Напоминать о приближающихся дедлайнах'));
    container.appendChild(toggleRow);

    const hoursGroup = createElement('div', { className: 'form-group' });
    hoursGroup.appendChild(createElement('label', { className: 'form-label' }, 'За сколько предупреждать'));
    const hoursSelect = createElement('select', { className: 'form-input' });
    const choices = [
        [2, 'За 2 часа'],
        [6, 'За 6 часов'],
        [24, 'За сутки'],
        [48, 'За двое суток'],
        [72, 'За трое суток'],
        [168, 'За неделю'],
    ];
    for (const [value, label] of choices) {
        const opt = createElement('option', { value: String(value) }, label);
        if (settings.hours === value) opt.setAttribute('selected', 'selected');
        hoursSelect.appendChild(opt);
    }
    // Значение из базы может не совпасть ни с одним пунктом — например, после
    // правки файла руками. Тогда добавляем его отдельным пунктом, чтобы список
    // не показывал одно, а база хранила другое.
    if (!choices.some(([value]) => value === settings.hours)) {
        const opt = createElement('option', { value: String(settings.hours) }, `За ${settings.hours} ч`);
        opt.setAttribute('selected', 'selected');
        hoursSelect.appendChild(opt);
    }
    hoursGroup.appendChild(hoursSelect);
    container.appendChild(hoursGroup);

    const syncEnabled = () => { hoursSelect.disabled = !toggle.checked; };
    syncEnabled();

    const save = async () => {
        try {
            const saved = await api.updateReminderSettings(toggle.checked, Number(hoursSelect.value));
            // Бэкенд зажимает часы в допустимый диапазон, поэтому список
            // приводится к тому, что действительно сохранилось.
            hoursSelect.value = String(saved.hours);
            showToast(saved.enabled ? 'Напоминания включены' : 'Напоминания выключены');
        } catch (e) {
            showToast('Не удалось сохранить настройку', 'error');
        }
    };

    toggle.addEventListener('change', () => { syncEnabled(); save(); });
    hoursSelect.addEventListener('change', save);
}

/**
 * «Email-напоминания» — письмо за неделю до срока, второй канал того же
 * напоминания. Всплывающее окно Windows видно, только пока человек за этим
 * компьютером; письмо догонит его где угодно, и приходит оно сильно раньше —
 * когда ещё можно что-то успеть.
 *
 * Пароль приложения живёт в Диспетчере учётных данных Windows, а не в базе, и
 * обратно не читается никогда: в поле ввода он не подставляется, наружу
 * приходит только признак «сохранён».
 */
async function renderEmailSection(container) {
    container.appendChild(createElement('p', { className: 'form-hint' },
        'Письмо уходит за 7 дней до срока — по одному на задачу, один раз. ' +
        'Проверка идёт при каждом запуске приложения, поэтому пропущенное за ' +
        'время простоя письмо всё равно придёт.'));

    let settings;
    try {
        settings = await api.getEmailSettings();
    } catch (e) {
        container.appendChild(createElement('p', { className: 'form-hint' },
            'Не удалось прочитать настройки почты'));
        return;
    }

    const toggleRow = createElement('label', { className: 'settings-reminders__toggle' });
    const toggle = createElement('input', { type: 'checkbox', className: 'settings-reminders__checkbox' });
    toggle.checked = settings.enabled;
    toggleRow.appendChild(toggle);
    toggleRow.appendChild(createElement('span', {}, 'Присылать письма о приближающихся сроках'));
    container.appendChild(toggleRow);

    // Поля остаются доступными и при выключенном переключателе: заполняют их
    // как раз до того, как включить, а тестовое письмо надо уметь отправить
    // до того, как полагаться на рассылку.
    const grid = createElement('div', { className: 'settings-email__grid' });

    const hostInput = createElement('input', { className: 'form-input', placeholder: 'smtp.gmail.com' });
    hostInput.value = settings.smtp_host;
    grid.appendChild(field('SMTP-сервер', hostInput));

    const portInput = createElement('input', {
        className: 'form-input', type: 'number', min: '1', max: '65535',
    });
    portInput.value = String(settings.smtp_port);
    grid.appendChild(field('Порт', portInput,
        '465 — TLS сразу, 587 — STARTTLS. Без шифрования письма не уходят ни на каком порту.'));

    container.appendChild(grid);

    const userInput = createElement('input', { className: 'form-input', placeholder: 'ваш.адрес@gmail.com' });
    userInput.value = settings.username;
    container.appendChild(field('Логин отправителя', userInput,
        'Он же адрес в поле «От кого»: отправлять от чужого имени почтовые серверы всё равно не дают.'));

    const toInput = createElement('input', { className: 'form-input' });
    toInput.value = settings.recipient;
    container.appendChild(field('Кому присылать', toInput));

    // ─── Пароль приложения ───

    const passGroup = createElement('div', { className: 'form-group' });
    passGroup.appendChild(createElement('label', { className: 'form-label' }, 'Пароль приложения'));

    const passRow = createElement('div', { className: 'settings-email__password' });
    const passInput = createElement('input', {
        className: 'form-input', type: 'password', autocomplete: 'off',
        placeholder: 'Вставьте пароль приложения',
    });
    const passSaveBtn = createElement('button', { className: 'btn btn--secondary' }, 'Сохранить пароль');
    const passClearBtn = createElement('button', {
        className: 'btn btn--ghost settings-email__password-clear',
        title: 'Удалить сохранённый пароль',
        innerHTML: Icons.trash,
    });
    passRow.appendChild(passInput);
    passRow.appendChild(passSaveBtn);
    passRow.appendChild(passClearBtn);
    passGroup.appendChild(passRow);

    const passState = createElement('p', { className: 'form-hint settings-email__password-state' });
    passGroup.appendChild(passState);
    passGroup.appendChild(createElement('p', { className: 'form-hint' },
        'Пароль хранится в Диспетчере учётных данных Windows, а не в базе — ' +
        'поэтому его нет ни в резервных копиях, ни в файле экспорта. ' +
        'Для Gmail нужен именно пароль приложения из настроек аккаунта, а не пароль от почты.'));
    container.appendChild(passGroup);

    // ─── Действия ───

    const actions = createElement('div', { className: 'settings-email__actions' });
    const saveBtn = createElement('button', { className: 'btn btn--primary' }, 'Сохранить');
    const testBtn = createElement('button', {
        className: 'btn btn--secondary',
        innerHTML: `${Icons.mail} <span>Отправить тестовое письмо</span>`,
    });
    actions.appendChild(saveBtn);
    actions.appendChild(testBtn);
    container.appendChild(actions);

    // Результат проверки остаётся на экране, в отличие от всплывающей
    // подсказки: причину отказа сервера читают и перечитывают, а иногда и
    // переписывают в поиск.
    const result = createElement('p', { className: 'settings-email__result', hidden: 'hidden' });
    container.appendChild(result);

    function showResult(text, kind) {
        result.textContent = text;
        result.className = `settings-email__result settings-email__result--${kind}`;
        result.hidden = false;
    }

    function syncPasswordState() {
        passClearBtn.hidden = !settings.has_password;
        if (settings.has_password) {
            passState.textContent = 'Пароль сохранён';
            passState.classList.add('settings-email__password-state--set');
        } else {
            passState.textContent = toggle.checked
                ? 'Пароль не задан — письма не пойдут'
                : 'Пароль не задан';
            passState.classList.remove('settings-email__password-state--set');
        }
    }
    syncPasswordState();
    toggle.addEventListener('change', syncPasswordState);

    /** Сохраняет то, что сейчас в полях, и возвращает `true` при успехе. */
    async function saveSettings(quiet) {
        try {
            const saved = await api.updateEmailSettings(
                toggle.checked,
                hostInput.value,
                Number(portInput.value) || 0,
                userInput.value,
                toInput.value,
            );
            // Бэкенд срезает пробелы и зажимает порт — показываем то, что
            // действительно сохранилось, а не то, что было набрано.
            settings = saved;
            hostInput.value = saved.smtp_host;
            portInput.value = String(saved.smtp_port);
            userInput.value = saved.username;
            toInput.value = saved.recipient;
            toggle.checked = saved.enabled;
            syncPasswordState();
            if (!quiet) showToast('Настройки почты сохранены');
            return true;
        } catch (e) {
            showToast(String(e), 'error');
            return false;
        }
    }

    /** Кладёт набранный пароль в Диспетчер; пустое поле не трогает ничего. */
    async function savePasswordIfTyped() {
        const typed = passInput.value.trim();
        if (!typed) return true;
        try {
            settings.has_password = await api.setEmailPassword(typed);
            // Поле очищается сразу: пароль уже в Диспетчере, а вторая его
            // копия в памяти страницы никому не нужна.
            passInput.value = '';
            syncPasswordState();
            return true;
        } catch (e) {
            showToast(String(e), 'error');
            return false;
        }
    }

    passSaveBtn.addEventListener('click', async () => {
        if (!passInput.value.trim()) {
            showToast('Введите пароль приложения', 'error');
            return;
        }
        passSaveBtn.disabled = true;
        try {
            if (await savePasswordIfTyped()) showToast('Пароль сохранён');
        } finally {
            passSaveBtn.disabled = false;
        }
    });

    passClearBtn.addEventListener('click', async () => {
        const ok = await confirmDialog({
            title: 'Удалить пароль?',
            message: 'Пароль будет стёрт из Диспетчера учётных данных Windows. ' +
                     'Письма перестанут отправляться, пока вы не введёте его снова.',
            confirmText: 'Удалить',
            danger: true,
        });
        if (!ok) return;
        try {
            settings.has_password = await api.clearEmailPassword();
            syncPasswordState();
            showToast('Пароль удалён');
        } catch (e) {
            showToast(String(e), 'error');
        }
    });

    saveBtn.addEventListener('click', async () => {
        saveBtn.disabled = true;
        try {
            await savePasswordIfTyped();
            await saveSettings(false);
        } finally {
            saveBtn.disabled = false;
        }
    });

    testBtn.addEventListener('click', async () => {
        testBtn.disabled = true;
        showResult('Отправляем…', 'pending');
        try {
            // Сначала сохраняем — иначе проверялось бы не то, что человек
            // видит на экране, а то, что лежало в базе до правок.
            if (!await savePasswordIfTyped()) return;
            if (!await saveSettings(true)) return;

            const message = await api.sendTestEmail();
            showResult(message + '. Проверьте ящик — письмо могло попасть в «Спам».', 'ok');
        } catch (e) {
            showResult(String(e), 'error');
        } finally {
            testBtn.disabled = false;
        }
    });
}

/** Подпись, поле и необязательное пояснение под ним. */
function field(label, input, hint) {
    const group = createElement('div', { className: 'form-group' });
    group.appendChild(createElement('label', { className: 'form-label' }, label));
    group.appendChild(input);
    if (hint) group.appendChild(createElement('p', { className: 'form-hint' }, hint));
    return group;
}

/**
 * «Фон пространства» — картинка позади всего окна, своя у каждого пространства.
 *
 * Живёт в настройках пространства, а не в общих: все действия здесь идут с тем
 * `workspaceId`, который открыт, — «текущее пространство» на бэкенде не
 * угадывается.
 */
async function renderBackgroundSection(container, workspaceId) {
    container.appendChild(createElement('p', { className: 'form-hint settings-background__intro' },
        'Картинка ложится под всё окно и сильно размывается — это фон, а не иллюстрация. ' +
        'Она принадлежит этому пространству: у остальных останется свой.'));

    const preview = createElement('div', { className: 'settings-background__preview' });
    container.appendChild(preview);

    const actions = createElement('div', { className: 'settings-background__actions' });
    const uploadBtn = createElement('button', {
        className: 'btn btn--primary',
        innerHTML: `${Icons.image} <span>Загрузить фото</span>`
    });
    const clearBtn = createElement('button', {
        className: 'btn btn--secondary',
        innerHTML: `${Icons.trash} <span>Сбросить фон</span>`
    });
    actions.appendChild(uploadBtn);
    actions.appendChild(clearBtn);
    container.appendChild(actions);

    // Кнопка «Сбросить фон» показывается, только когда есть что сбрасывать:
    // иначе это ровно тот случай из раздела 5 заметок — элемент, обещающий
    // действие, которого не будет.
    async function refresh() {
        const url = await getBackgroundUrl(workspaceId);
        preview.innerHTML = '';
        if (url) {
            preview.appendChild(createElement('img', {
                className: 'settings-background__image',
                src: url,
                alt: 'Текущий фон сайдбара'
            }));
        } else {
            preview.appendChild(createElement('span', { className: 'settings-background__empty' },
                'Фон не задан'));
        }
        clearBtn.hidden = !url;
    }

    uploadBtn.addEventListener('click', async () => {
        uploadBtn.disabled = true;
        try {
            const source = await api.showOpenDialog('Изображение', ['png', 'jpg', 'jpeg', 'webp']);
            // Закрыть диалог, ничего не выбрав, — нормальный исход, не ошибка.
            if (!source) return;

            await api.setWorkspaceBackground(workspaceId, source);
            invalidateBackground(workspaceId);
            await refresh();
            await applyWorkspaceBackground(workspaceId);
            showToast('Фон обновлён');
        } catch (e) {
            showToast(String(e), 'error');
        } finally {
            uploadBtn.disabled = false;
        }
    });

    clearBtn.addEventListener('click', async () => {
        const ok = await confirmDialog({
            title: 'Сбросить фон?',
            message: 'Картинка будет удалена из папки приложения. Исходный файл на диске останется.',
            confirmText: 'Сбросить',
            danger: true,
        });
        if (!ok) return;

        clearBtn.disabled = true;
        try {
            await api.clearWorkspaceBackground(workspaceId);
            invalidateBackground(workspaceId);
            await refresh();
            await applyWorkspaceBackground(workspaceId);
            showToast('Фон сброшен');
        } catch (e) {
            showToast(String(e), 'error');
        } finally {
            clearBtn.disabled = false;
        }
    });

    await refresh();
}

/**
 * Backups panel.
 *
 * All of the user's data lives in a single SQLite file, so the app now snapshots
 * it on every start and keeps the last 10. This section exists so that safety
 * net is visible rather than a silent implementation detail — otherwise nobody
 * would know the copies are there when they need them.
 */
async function renderBackupSection(container) {
    container.appendChild(createElement('p', { className: 'form-hint settings-backup__intro' },
        'Копия базы создаётся автоматически при запуске приложения, но не чаще раза в час. Хранятся последние 10.'));

    renderExportRow(container);

    let backups = [];
    let dir = '';
    try {
        [backups, dir] = await Promise.all([api.getBackups(), api.getBackupDir()]);
    } catch (e) {
        container.appendChild(createElement('p', { className: 'form-hint' }, 'Не удалось прочитать папку с копиями'));
        return;
    }

    const pathRow = createElement('div', { className: 'settings-backup__path' });
    pathRow.appendChild(createElement('code', { className: 'settings-backup__path-value' }, dir));
    container.appendChild(pathRow);

    const count = backups.length;
    container.appendChild(createElement('div', { className: 'settings-backup__count' },
        count === 0
            ? 'Копий пока нет — первая появится при следующем запуске'
            : `${count} ${pluralize(count, ['копия', 'копии', 'копий'])}`));

    if (count > 0) {
        const list = createElement('div', { className: 'settings-backup__list' });
        for (const backup of backups) {
            const row = createElement('div', { className: 'settings-backup__row' });
            row.appendChild(createElement('span', { className: 'settings-backup__date' }, backup.created_at));
            row.appendChild(createElement('span', { className: 'settings-backup__size' }, formatBytes(backup.size_bytes)));
            list.appendChild(row);
        }
        container.appendChild(list);
    }

    const openBtn = createElement('button', {
        className: 'btn btn--secondary',
        innerHTML: `${Icons.archive} <span>Открыть папку с копиями</span>`
    });
    openBtn.addEventListener('click', async () => {
        try {
            await api.openBackupDir();
        } catch (e) {
            showToast('Не удалось открыть папку', 'error');
        }
    });
    container.appendChild(openBtn);

    container.appendChild(createElement('p', { className: 'form-hint settings-backup__note' },
        'Чтобы восстановиться из копии, закройте приложение и замените ею файл trello_clone.db в папке выше. Восстановление прямо из интерфейса появится позже.'));
}

/**
 * "Экспортировать данные" — a copy of the whole database wherever the user
 * wants it, as opposed to the automatic backups, which only ever live in the
 * app's own folder and rotate away after ten.
 */
function renderExportRow(container) {
    const block = createElement('div', { className: 'settings-export' });

    const btn = createElement('button', {
        className: 'btn btn--primary',
        innerHTML: `${Icons.download} <span>Экспортировать данные</span>`
    });

    btn.addEventListener('click', async () => {
        btn.disabled = true;
        try {
            const defaultName = await api.suggestExportName();
            const path = await api.showSaveDialog(defaultName, 'База данных TaskFlow', ['db']);
            // Cancelling the dialog is a normal outcome, not an error.
            if (!path) return;

            const result = await api.exportDatabase(path);
            showToast('Экспортировано: ' + [
                countPhrase(result.boards_active, result.boards, ['доска', 'доски', 'досок']),
                countPhrase(result.cards_active, result.cards, ['карточка', 'карточки', 'карточек']),
            ].join(', ') + ` · ${formatBytes(result.size_bytes)}`);
        } catch (e) {
            showToast(String(e), 'error');
        } finally {
            btn.disabled = false;
        }
    });

    block.appendChild(btn);
    block.appendChild(createElement('p', { className: 'form-hint' },
        'Полная копия базы в выбранный вами файл — её можно унести на другой диск. ' +
        'Открывается любым просмотрщиком SQLite.'));
    block.appendChild(createElement('p', { className: 'form-hint settings-export__warning' },
        'Этот файл не зашифрован — в отличие от самой базы и автоматических копий. ' +
        'Иначе он открывался бы только на этом компьютере и стал бы бесполезен ' +
        'как раз тогда, когда понадобится. Храните его там, куда не дотянется чужой.'));

    container.appendChild(block);
}

/**
 * "9 досок из 29 (остальные архивные и служебные)" — обе цифры сразу.
 *
 * В файл попадает всё, включая архив и скрытые доски Inbox, поэтому общее
 * число честнее. Но на хабе видно только активные, и сообщение с одним лишь
 * общим числом выглядело бы как ошибка. Когда числа совпадают, скобка не нужна.
 */
function countPhrase(active, total, forms) {
    if (active === total) return `${total} ${pluralize(total, forms)}`;
    return `${active} ${pluralize(active, forms)} из ${total} (остальные архивные и служебные)`;
}

function formatBytes(bytes) {
    if (bytes < 1024) return `${bytes} Б`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} КБ`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} МБ`;
}
