# Documentation Actualization Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Актуализировать всю документацию проекта BlackBox до состояния Phase 3, добавив описание новых инструментов и модулей (HTTP Proxy, File Watcher, Structured Logs).

**Architecture:** Переход от плоского списка инструментов к категоризированному справочнику, обновление архитектурных схем и создание глубоких погружений в специфические фичи.

**Tech Stack:** Markdown, Mermaid.

---

### Task 1: Обновление 01_ARCHITECTURE.md

**Files:**
- Modify: `docs/01_ARCHITECTURE.md`

**Step 1: Обновить Mermaid диаграмму**
Добавить блоки `HttpProxy`, `FileWatcher`, `StructuredStore` и связи между ними.

**Step 2: Актуализировать описание Core Components**
Добавить упоминание `StructuredStore` и `TypedContext`.

**Step 3: Обновить Data Flow**
Описать, как теперь данные поступают не только из терминала, но и через HTTP-прокси и внешние файлы.

**Step 4: Commit**
```bash
git add docs/01_ARCHITECTURE.md
git commit -m "docs: update architecture overview and diagrams"
```

### Task 2: Обновление 02_MCP_TOOLS_REFERENCE.md (Категоризация)

**Files:**
- Modify: `docs/02_MCP_TOOLS_REFERENCE.md`

**Step 1: Переработать таблицу инструментов**
Разбить на 5 групп: Core, Diagnostics, Infrastructure, Tracking, Advanced.

**Step 2: Добавить описания новых инструментов**
Добавить `get_recent_commits`, `watch_log_file`, `get_watched_files`, `get_http_errors`, `get_structured_context`, `get_process_logs`.

**Step 3: Обновить существующие инструменты**
Актуализировать описание `get_correlated_errors` (теперь включает HTTP) и `get_terminal_buffer` (параметр `terminal`).

**Step 4: Commit**
```bash
git add docs/02_MCP_TOOLS_REFERENCE.md
git commit -m "docs: categorize MCP tools and add new tool references"
```

### Task 3: Обновление 03_SCANNERS_LOGIC.md

**Files:**
- Modify: `docs/03_SCANNERS_LOGIC.md`

**Step 1: Добавить описание Git Scanner**
Описать использование `gix` и логику извлечения диффов.

**Step 2: Добавить описание Manifests & Env Scanners**
Описать детекцию типа проекта и маскировку `.env` файлов.

**Step 3: Commit**
```bash
git add docs/03_SCANNERS_LOGIC.md
git commit -m "docs: document git, manifest and env scanners"
```

### Task 4: Обновление 04_DOCKER_AND_SYSTEM.md

**Files:**
- Modify: `docs/04_DOCKER_AND_SYSTEM.md`

**Step 1: Актуализировать Discovery & Resilience**
Описать текущую логику переподключения к Docker.

**Step 2: Добавить Cross-source correlation**
Упомянуть связь логов Docker с HTTP-ошибками.

**Step 3: Commit**
```bash
git add docs/04_DOCKER_AND_SYSTEM.md
git commit -m "docs: update docker integration details"
```

### Task 5: Обновление остальных существующих файлов

**Files:**
- Modify: `docs/05_LAB_AND_UI.md`
- Modify: `docs/06_IDE_INTEGRATION.md`
- Modify: `docs/07_TESTING_AND_SECURITY.md`
- Modify: `docs/08_PHASE3_ROADMAP.md`

**Step 1: Добавить инфо про Lab UI компоненты**
(GitLens, LogExplorer) в `05_LAB_AND_UI.md`.

**Step 2: Обновить 07_TESTING_AND_SECURITY.md**
Добавить описание `Typed Context` и маскировки HTTP-тел.

**Step 3: Актуализировать Roadmap**
Phase 3 -> DONE. Добавить Phase 4 идеи.

**Step 4: Commit**
```bash
git add docs/05_LAB_AND_UI.md docs/06_IDE_INTEGRATION.md docs/07_TESTING_AND_SECURITY.md docs/08_PHASE3_ROADMAP.md
git commit -m "docs: finalize existing docs updates and roadmap"
```

### Task 6: Создание 09_HTTP_PROXY.md

**Files:**
- Create: `docs/09_HTTP_PROXY.md`

**Step 1: Описать логику работы прокси**
Порт 8769, заголовок `X-Proxy-Target`.

**Step 2: Описать маскировку данных**
Как маскируются JSON-поля в телах запросов/ответов.

**Step 3: Commit**
```bash
git add docs/09_HTTP_PROXY.md
git commit -m "docs: add detailed http proxy guide"
```

### Task 7: Создание 10_EXTERNAL_LOGS.md

**Files:**
- Create: `docs/10_EXTERNAL_LOGS.md`

**Step 1: Описать инструмент watch_log_file**
Как добавить произвольный файл для мониторинга.

**Step 2: Описать механику очистки ANSI и PII**
Для внешних файлов.

**Step 3: Commit**
```bash
git add docs/10_EXTERNAL_LOGS.md
git commit -m "docs: add file watcher documentation"
```

### Task 8: Создание 11_STRUCTURED_LOGS.md

**Files:**
- Create: `docs/11_STRUCTURED_LOGS.md`

**Step 1: Описать парсинг JSON-логов**
Как BlackBox находит Span-ID и коррелирует события.

**Step 2: Описать использование инструмента get_structured_context**
Примеры запросов по `span_id`.

**Step 3: Commit**
```bash
git add docs/11_STRUCTURED_LOGS.md
git commit -m "docs: add structured logging documentation"
```
