---
name: TaskFlow
description: Тёплое приложение — личный планировщик задач и привычек, мягкий и живой
colors:
  action: "#ff6a3d"
  action-hover: "#ff8a5f"
  action-pressed: "#f25424"
  action-ink: "#2a1208"
  action-text-light: "#b8430f"
  action-text-dark: "#ff9a6b"
  apricot-ground: "#fff3ec"
  apricot-sidebar: "#fdebe1"
  apricot-column: "#fbe7dc"
  card-white: "#ffffff"
  line-light: "#f0dcd0"
  ink-light: "#2b211d"
  ink-light-secondary: "#5e5049"
  ink-light-muted: "#7a6a61"
  charcoal-ground: "#18120f"
  charcoal-sidebar: "#1d1612"
  charcoal-column: "#221a16"
  charcoal-card: "#2a211c"
  charcoal-elevated: "#31261f"
  ink-dark: "#f7eee8"
  ink-dark-secondary: "#cdbdb3"
  ink-dark-muted: "#a89488"
  overdue-light: "#c2264b"
  overdue-dark: "#ff6b8b"
  time-light: "#8a5a00"
  time-dark: "#ffc14d"
  sunflower: "#ffc53d"
  done-light: "#13805a"
  done-dark: "#4fd39a"
  info-light: "#1f62c4"
  info-dark: "#6fb2ff"
  chat-mine-light: "#ffe2d6"
  chat-mine-dark: "#3d2a20"
typography:
  greeting:
    fontFamily: "Nunito, Segoe UI Variable Display, sans-serif"
    fontSize: "36px"
    fontWeight: 700
    lineHeight: 1.1
    letterSpacing: "-0.02em"
  page-title:
    fontFamily: "Nunito, Segoe UI Variable Display, sans-serif"
    fontSize: "28px"
    fontWeight: 700
    lineHeight: 1.2
    letterSpacing: "-0.02em"
  section-title:
    fontFamily: "Nunito, Segoe UI Variable Display, sans-serif"
    fontSize: "16px"
    fontWeight: 700
    lineHeight: 1.3
  body:
    fontFamily: "Segoe UI Variable Text, Segoe UI, sans-serif"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: 1.5
  label:
    fontFamily: "Segoe UI Variable Text, Segoe UI, sans-serif"
    fontSize: "12px"
    fontWeight: 500
    lineHeight: 1.4
  code:
    fontFamily: "Cascadia Mono, Consolas, monospace"
    fontSize: "13px"
    fontWeight: 400
    lineHeight: 1.6
rounded:
  xs: "6px"
  sm: "10px"
  md: "12px"
  lg: "16px"
  xl: "18px"
  2xl: "22px"
  pill: "999px"
spacing:
  2xs: "2px"
  xs: "4px"
  sm: "8px"
  md: "12px"
  lg: "16px"
  xl: "24px"
  2xl: "32px"
  3xl: "48px"
components:
  button-primary:
    backgroundColor: "{colors.action}"
    textColor: "{colors.action-ink}"
    rounded: "{rounded.md}"
    padding: "8px 16px"
  button-primary-hover:
    backgroundColor: "{colors.action-hover}"
  button-primary-active:
    backgroundColor: "{colors.action-pressed}"
  button-create:
    backgroundColor: "{colors.action}"
    textColor: "{colors.action-ink}"
    rounded: "{rounded.pill}"
    padding: "8px 16px"
  card:
    backgroundColor: "{colors.card-white}"
    rounded: "{rounded.lg}"
    padding: "12px"
  card-dark:
    backgroundColor: "{colors.charcoal-card}"
    rounded: "{rounded.lg}"
    padding: "12px"
  widget:
    backgroundColor: "{colors.card-white}"
    rounded: "{rounded.xl}"
    padding: "16px"
  chip-overdue:
    textColor: "{colors.overdue-light}"
    rounded: "{rounded.pill}"
    padding: "2px 8px"
  chip-time:
    textColor: "{colors.time-light}"
    rounded: "{rounded.pill}"
    padding: "2px 8px"
  sidebar-item-active:
    textColor: "{colors.action-text-light}"
    rounded: "{rounded.md}"
    padding: "8px 11px"
---

# Design System: TaskFlow

## Overview

**Тёплое приложение.** TaskFlow выглядит как дружелюбное потребительское приложение, а не как рабочий инструмент: мягкие белые карточки лежат на тёплом фоне, подписи звучат по-человечески, а единственный сочный цвет — мандариновый — есть только у того, что можно нажать. Обе темы на равных: абрикосовый день и тёплый уголь вечером, потому что владелец работает и днём на ноутбуке, и ночью в тёмной комнате.

Мир выбран владельцем 27.11.2026 после пояснения вкуса: «футуристично и мягко-объёмно, цвета 4 из 5, много анимации, обе темы, ориентиры Spotify, YouTube и Arknights: Endfield». Отсюда два отступления от каталожного «тёплого приложения»: цвета больше (у каждой доски свой оттенок обложки), а движение живое (пружина на нажатие, подъём карточек, «лопание» отметки).

Токены живут в `src/css/variables.css`, грамматика мира, которую токеном не выразить, — в `src/css/world.css` (подключён последним).

## Colors

### Primary

- **Мандарин действия** (`action`): главная кнопка, «+ Создать», выбранный пункт навигации, переключатель вида доски, фокус полей, чекбоксы. Текст на нём тёмный (`action-ink`, 5,5:1): белый на мандарине держит лишь 2,9:1. Мандарином как текстом пишут тёмной ступенью (`action-text-light` на светлом, `action-text-dark` на тёмном).

### Secondary

- **Цвет обложки доски** — данные человека, а не палитра интерфейса. Плитка доски на экране «Доски» несёт его шапкой (85 %), шапка самой доски — оттенком (16 %), строка задачи на Главной — плиткой-буквой (35 %). Новые обложки — восемь плоских сочных цветов (`js/utils.js`), старые градиенты в базе выводятся как есть.

### Tertiary

- **Цвета состояний** — всегда пилюлей с иконкой или словом, а не одним цветом: малиновый — просрочка и опасное (`overdue-*`), солнечный янтарь — время и внимание: «сегодня/завтра», номер сегодняшнего дня (`time-*`, `sunflower`), мята — сделано: отметка привычки, тепловая карта, прогресс (`done-*`).

### Neutral

- Светлая тема: фон `apricot-ground` (розовато-персиковый, не пергамент), сайдбар `apricot-sidebar`, лоток колонки `apricot-column`, карточки `card-white`, линии `line-light`, текст `ink-light` / `ink-light-secondary` / `ink-light-muted` (15,2 / 7,2 / 4,7:1).
- Тёмная тема: фон `charcoal-ground`, сайдбар `charcoal-sidebar`, лоток `charcoal-column`, карточки `charcoal-card`, всплывающее `charcoal-elevated`, текст `ink-dark` / `ink-dark-secondary` / `ink-dark-muted` (16,4 / 10,3 / 6,4:1).

### Named Rules

**The One Bright Rule.** Сочный мандарин — только у нажимаемого. Реплика человека в чате, прогресс, «сегодня» и данные — пастель или цвет состояния. Тест: если мандариновый элемент не реагирует на щелчок, это ошибка.

**The Crimson Not Orange Rule.** Просрочка — малиновая (розово-красная), а не красно-оранжевая: рядом с мандариновым действием её нельзя спутать с кнопкой.

**The Mint Means Done Rule.** Всё о сделанном — мята: отметка привычки, тепловая карта, полоска «отмечено сегодня», сводка «все привычки отмечены».

## Typography

Два голоса. **Nunito** — скруглённый гуманистический гротеск (OFL 1.1, переменный 200–1000, файл и лицензия в `src/assets/fonts/`, подключён через `@font-face`, сети не нужно) — заголовки, приветствие, название доски, логотип-надпись: скруглённые окончания и есть дружелюбный голос мира. **Segoe UI Variable Text** — весь рабочий текст, кнопки, поля и данные: у Nunito нет табличных цифр, а сроки и счётчики должны стоять ровной колонкой. Cascadia Mono — только код в Markdown.

### Hierarchy

- **Приветствие** (36 px, 700, −0,02 em): «Добрый вечер, Шахзод!» на Главной.
- **Заголовок экрана** (28 px, 700): «Список», «Привычки», название доски.
- **Заголовок раздела** (16 px, 700 — полужирный Nunito легче полужирного Segoe): «Сегодня и просрочено», «Недавние доски».
- **Основной текст** (14 px, 400, 1,5): названия задач, описания, чат.
- **Подпись** (12 px, 500): чипы, сроки, мета карточки.

### Named Rules

**The Plain Words Rule.** Подписи — как сказал бы человек: «2 задачи просрочены», «со сроком сегодня или завтра», «просрочек нет». Никаких прописных подписей-плакатов над заголовками.

## Layout

Сайдбар 256 px (сворачивается в рельс 64 px; на доске свёрнут по умолчанию), шапка 56 px, страница с отступами 24/32 px. Главная: приветствие, строка-сводка, затем «Сегодня и просрочено» во всю ширину и сетка виджетов в две колонки `minmax(0, 1fr)` (одна — уже 980 px). Окно приложения не бывает уже 900 px — это нижняя граница проверки.

## Elevation & Depth

Глубина — мягкие тени со смещением и размытием, тёплые (коричневые) на светлом фоне и тёмные на тёмном. Четыре ступени: карточка в покое, карточка под курсором (глубже), всплывающее (меню, палитра), модальное окно. Цветная тень есть только у мандариновой кнопки на светлой теме; на тёмной она нейтральная, чтобы не читаться свечением.

### Named Rules

**The Lift Rule.** Под курсором карточка поднимается на 2–3 px и её тень становится глубже. На доске подъём делается свойством `translate`, а не `transform`: transform при перестановке пишет Sortable.

## Shapes

Крупные мягкие радиусы: кнопка и поле 12 px, карточка задачи 16 px, виджет и плитка 18 px, модальное окно 22 px, чипы и «+ Создать» — пилюля. Аватары и кружки отметок — круги.

## Components

### Buttons

- **Главная** — мандарин с тёмным текстом, радиус 12 px, мягкая цветная тень (светлая тема). При нажатии сжимается до 0,95 и пружиной возвращается.
- **«+ Создать»** — та же главная кнопка, но пилюлей; единственная мандариновая кнопка в шапке.
- **Вторичная** — белая (тёмная — цвет карточки) с тонкой линией.

### Chips

- **Срок**: пилюля с часами; просрочка — малиновая, «сегодня/завтра» — янтарь, остальное — нейтральная.
- **Сводка Главной**: пилюли с точкой-маркером и жирным числом, каждая ведёт на свой экран; появляются по очереди с шагом 60 мс.

### Cards / Containers

- **Карточка задачи** — белая (тёмная `charcoal-card`) на лотке колонки, радиус 16 px, мягкая тень, без рамки; под курсором — подъём и линия.
- **Виджет Главной** — белая карточка радиусом 18 px с тонкой линией.
- **Плитка доски** — белая карточка, верхняя половина — цвет обложки, звёздочка — белый кружок.

### Inputs / Fields

Белое поле (тёмная тема — плёнка) радиусом 12 px; в фокусе — мандариновая линия и мягкое кольцо.

### Navigation

Пункты сайдбара радиусом 12 px; выбранный — мягкая мандариновая плашка, мандариновая иконка и полужирный текст. Подпункт пространства (Доски / Участники / Настройки) — нейтральная плёнка, чтобы не спорить с главным выбранным пунктом.

### Отметка привычки

Кружок: при отметке стрелка обходит его по кругу, рисуется галочка, расходится кольцо и по строке пробегает полоса света — всё мятой. Выполненное — мятная заливка, лайт-версия — мята вполсилы. Тепловая карта — четыре ступени мяты.

## Do's and Don'ts

### Do:

- Do держать мандарин только на нажимаемом и проверять это при каждом новом элементе.
- Do нести цвет доски туда, где доска упоминается: плитка, шапка, строка задачи.
- Do давать состояниям слово или иконку рядом с цветом.
- Do пружинить нажатие и поднимать карточки под курсором; под `prefers-reduced-motion` — без движения.

### Don't:

- Don't заливать мандарином сведения: реплики чата, прогресс, «сегодня», уверенность ИИ.
- Don't подпрыгивать при появлении: пружина (`--ease-press`) — только на нажатие, входы — плавным выходом.
- Don't пульсировать бесконечно: живые точки (таймер, уведомления, ассистент) делают три импульса и затихают.
- Don't делать фон кремово-пергаментным и брать серую тень на светлой теме.
