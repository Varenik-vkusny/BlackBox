# BlackBox — Полный обзор Phase 1 и Phase 2

## Что такое BlackBox

BlackBox — это MCP (Model Context Protocol) сервер-демон, который работает рядом с твоим редактором и даёт AI-агентам живой контекст о том, что происходит в проекте: что летит в терминал, какие ошибки есть, что изменилось в git, какие контейнеры падают. AI не читает файлы наугад — она спрашивает BlackBox и получает точный, токен-эффективный ответ.

---

## Архитектура: как всё устроено

```
VS Code Extension
      │  TCP 127.0.0.1:8765
      ▼
┌─────────────────────────────────────────────────────┐
│                  blackbox-daemon                    │
│                                                     │
│  tcp_bridge ──► push_line_and_drain                 │
│                      │                              │
│               ┌──────┴──────────┐                   │
│               ▼                 ▼                   │
│         SharedBuffer      SharedDrainState          │
│      (ring buffer 5000)  (cluster tree)             │
│                                                     │
│  docker/mod ──► SharedErrorStore                    │
│           (per-container VecDeque)                  │
│                                                     │
│  MCP stdio ──► DaemonState ──► 7 инструментов      │
│                                                     │
│  status_server (HTTP :8766) ──► TUI дашборд        │
└─────────────────────────────────────────────────────┘
      │  JSON-RPC 2.0 over stdio
      ▼
  AI-агент (Claude, GPT, etc.)
```

**Четыре параллельных задачи в `tokio::select!`:**
1. `tcp_bridge` — принимает терминальный вывод от VS Code
2. `status_server` — HTTP-сервер для TUI дашборда
3. `mcp_stdio` — JSON-RPC сервер для AI-агентов
4. `docker_monitor` — стримит логи из Docker контейнеров

---

## Phase 1 vs Phase 2: сравнение

| Аспект | Phase 1 | Phase 2 |
|--------|---------|---------|
| **MCP инструментов** | 4 | 7 |
| **Данные из терминала** | Сырые строки в кольцевом буфере | То же + кластеризация (Drain) |
| **Анализ ошибок** | Нет | Парсинг стек-трейсов (4 языка) |
| **Git интеграция** | Ветка + кол-во грязных файлов | Список изменённых файлов + diff hunks |
| **Docker** | Нет | Стриминг логов, фильтрация ERROR/WARN/FATAL |
| **Структура состояния** | Отдельные аргументы в функциях | `DaemonState` — единый клонируемый стейт |
| **Блокирующий I/O** | Некоторые вызовы блокировали async | Всё тяжёлое вынесено в `spawn_blocking` |
| **Фолбэк при пустом ответе** | Нет | Цепочка фолбэков с `fallback_source` |
| **Тестирование** | 7 секций | 11 секций |
| **Интерфейс sandbox** | 5 вкладок | 8 вкладок |

---

## DaemonState — центральный стейт (Phase 2)

```rust
#[derive(Clone)]
pub struct DaemonState {
    pub buf: SharedBuffer,          // Arc<RwLock<VecDeque<LogLine>>>
    pub drain: SharedDrainState,    // Arc<RwLock<DrainState>>
    pub error_store: SharedErrorStore, // Arc<RwLock<ErrorStore>>
    pub cwd: PathBuf,               // рабочая директория
    pub start_time: Instant,        // время старта демона
}
```

До Phase 2 каждая функция получала `buf`, `cwd`, `start_time` отдельными аргументами. С ростом количества данных это стало бы неуправляемым. `DaemonState` — единственный стейт, который клонируется в каждый task. Клонирование дешёвое, потому что все поля — `Arc` (счётчик ссылок) или `Copy`.

---

## Все 7 MCP инструментов

### 1. `get_snapshot`
**Что делает:** Быстрый срез текущего состояния проекта.

**Возвращает:**
```json
{
  "daemon_uptime_secs": 142,
  "project_type": "cargo",
  "git_branch": "phase-2",
  "git_dirty_files": 5,
  "buffer_lines": 312,
  "has_recent_errors": true
}
```

**Когда AI должна вызывать:** Первым делом в любой сессии, чтобы понять контекст.

---

### 2. `get_terminal_buffer`
**Что делает:** Возвращает последние N строк из кольцевого буфера (дефолт 100, макс 500). ANSI-коды очищены, вывод обёрнут в XML-теги для защиты от prompt injection.

**Аргументы:** `lines: integer` (опционально)

**Возвращает:**
```json
{
  "content": "<terminal_output source=\"vscode_bridge\" untrusted=\"true\">\nerror: cannot borrow...\n</terminal_output>",
  "lines_returned": 100
}
```

**Фолбэк:** Если буфер пуст → `fallback_source: "none"` + объяснение.

**Защита от injection:**
- Экранируются: `</terminal_output>`, `<script`, `<iframe`, `<object`
- Атрибут `untrusted="true"` — сигнал AI что контент ненадёжный

---

### 3. `get_project_metadata`
**Что делает:** Сканирует файловую систему и возвращает манифесты проекта + ключи из `.env` файлов (без значений).

**Возвращает:**
```json
{
  "manifests": [
    { "manifest_type": "cargo", "name": "blackbox-daemon", "version": "0.1.0", "path": "crates/blackbox-daemon/Cargo.toml" }
  ],
  "env_keys": ["DATABASE_URL", "SECRET_KEY", "PORT"]
}
```

**Приоритет манифестов:** Cargo.toml > go.mod > package.json

**Важно:** В Phase 2 исправлен баг — сканирование было синхронным и блокировало async runtime. Теперь завёрнуто в `spawn_blocking`.

---

### 4. `read_file`
**Что делает:** Читает файл в пределах `cwd`. Поддерживает диапазон строк.

**Аргументы:**
- `path: string` (обязательный) — относительный или абсолютный путь
- `from_line: integer` (опционально) — начальная строка (1-based)
- `to_line: integer` (опционально) — конечная строка

**Защита от path traversal:** `canonicalize()` + проверка `starts_with(cwd)`.

**Возвращает:**
```json
{
  "path": "src/main.rs",
  "content": "fn main() {\n    ...\n}",
  "from_line": 1,
  "to_line": 50
}
```

---

### 5. `get_compressed_errors` *(новый в Phase 2)*
**Что делает:** Вместо тысячи повторяющихся строк ошибок — сжатые кластеры + распознанные стек-трейсы.

**Аргументы:** `limit: integer` (дефолт 50)

**Возвращает:**
```json
{
  "clusters": [
    {
      "pattern": "error: connection refused to *",
      "count": 847,
      "level": "error",
      "first_seen_ms": 1700000000000,
      "last_seen_ms": 1700000100000,
      "example": "error: connection refused to 10.0.0.5"
    }
  ],
  "stack_traces": [
    {
      "language": "rust",
      "error_message": "thread 'main' panicked at 'index out of bounds'",
      "frames": [
        { "raw": "0: myapp::handler", "file": "src/handler.rs", "line": 42, "is_user_code": true },
        { "raw": "1: std::rt::lang_start", "file": null, "line": null, "is_user_code": false }
      ],
      "source_files": ["src/handler.rs"],
      "captured_at_ms": 1700000050000
    }
  ],
  "total_error_lines": 847
}
```

**Фолбэк:** Если кластеров и стек-трейсов нет → возвращает сырой буфер с `fallback_source: "terminal_buffer"`.

---

### 6. `get_contextual_diff` *(новый в Phase 2)*
**Что делает:** Хирургическая точность — берёт только те diff-хунки, файлы которых одновременно:
1. Упоминаются в стек-трейсах из буфера
2. Изменены в git (незакоммиченные изменения)

**Логика:**
```
last 500 строк буфера
    → extract_stack_traces()
    → extract_source_files() → ["src/db.rs", "src/handler.rs"]
                                    ∩ (пересечение)
git diff --name-status HEAD  → ["src/db.rs", "src/config.rs"]
                                    ↓
                            ["src/db.rs"]  ← только совпадения
                                    ↓
git diff HEAD -U3 -- src/db.rs  → diff hunks
```

**Возвращает:**
```json
{
  "diff_hunks": [
    {
      "file": "src/db.rs",
      "old_start": 42,
      "new_start": 44,
      "lines": [
        { "kind": "context", "text": "    let conn = pool.get();" },
        { "kind": "removed", "text": "    conn.execute(query)?" },
        { "kind": "added", "text": "    conn.execute(query).map_err(DbError::from)?" }
      ]
    }
  ],
  "files_cross_referenced": ["src/db.rs"],
  "truncated": false,
  "fallback_source": "none"
}
```

**Цепочка фолбэков (в порядке приоритета):**
1. Нет совпадений → данные из `get_compressed_errors`
2. Кластеров тоже нет → сырой буфер
3. Буфер пуст → `fallback_source: "none"` + объяснение

**Ограничения:** 50 хунков макс, 30 строк на хунк.

---

### 7. `get_container_logs` *(новый в Phase 2)*
**Что делает:** Возвращает отфильтрованные события из Docker контейнеров. Фильтруется всё кроме ERROR/WARN/FATAL.

**Аргументы:**
- `container_id: string` (опционально) — фильтр по имени/ID контейнера
- `limit: integer` (дефолт 50)

**Возвращает (Docker доступен):**
```json
{
  "containers": ["postgres", "redis", "api-server"],
  "events": [
    {
      "source": { "type": "docker", "container_id": "api-server" },
      "text": "{\"level\":\"error\",\"msg\":\"DB connection pool exhausted\"}",
      "timestamp_ms": 1700000050000,
      "level": "error"
    }
  ],
  "docker_available": true,
  "fallback_source": "none"
}
```

**Случаи фолбэка:**
- Docker доступен, ошибок нет → `docker_available: true`, `events: []`, `fallback_reason: "Docker подключён, ERROR/WARN/FATAL событий не было"`
- Docker недоступен + есть кластеры → `fallback_source: "compressed_errors"`
- Docker недоступен + нет кластеров → `fallback_source: "terminal_buffer"`

---

## Система фолбэков (добавлено в конце Phase 2)

Каждый инструмент теперь всегда возвращает что-то полезное. Поле `fallback_source` говорит AI откуда пришли данные, чтобы она могла запомнить какой источник чаще всего полезен для конкретной части проекта.

```
get_contextual_diff → (пусто) → compressed_errors → (пусто) → terminal_buffer → (пусто) → "none"
get_compressed_errors →          (пусто)           → terminal_buffer → (пусто) → "none"
get_container_logs   → (нет Docker) → compressed_errors → terminal_buffer → "none"
get_terminal_buffer  →                              (пусто) → "none"
```

---

## Новые модули Phase 2

### `scanners/drain.rs` — алгоритм Drain (дедупликация логов)

Алгоритм Drain группирует похожие строки в кластеры вместо того чтобы хранить тысячи одинаковых строк.

**Структура:**
```
prefix_tree: HashMap<usize, Vec<LogCluster>>
                      ▲
              ключ = кол-во токенов в строке
```

**Процесс обработки строки:**
1. Токенизация: `"error: timeout to 10.0.0.1"` → `["error:", "timeout", "to", "10.0.0.1"]` (4 токена)
2. Ищем bucket с ключом `4`
3. Для каждого кластера в bucket считаем similarity = `совпадающих_позиций / длина`
4. Если similarity ≥ 0.5 → обновляем кластер: несовпадающие токены заменяем на `*`
5. Если нет подходящего → создаём новый кластер
6. При 1000 кластерах → вытесняем самый старый (FIFO по `last_seen_ms`)

**Пример:**
```
Строка 1: "error: timeout connecting to 10.0.0.1"
Строка 2: "error: timeout connecting to 10.0.0.2"
Строка 3: "error: timeout connecting to 192.168.1.1"
            ↓
Кластер: "error: timeout connecting to *" (count=3)
```

**Ограничение:** Работает только со строками одинаковой длины (в токенах). Строки разной длины всегда попадают в разные кластеры.

---

### `scanners/stacktrace.rs` — парсер стек-трейсов

State-machine парсер для 4 языков. Все парсеры работают по одному принципу: детектируют trigger-строку, читают frame-строки, останавливаются на стоп-условии.

| Язык | Trigger | Frame pattern | Стоп |
|------|---------|---------------|------|
| **Rust** | `"panicked at"` / `"error["` | `"N: module::func"` | Строка не начинается с цифры |
| **Python** | `"Traceback (most recent call last):"` | `"File \"path\", line N"` | Не-индентированная строка |
| **Node.js** | `"TypeError:"` / `"Error:"` / etc | `"at Function (file:line:col)"` | Строка не начинается с `"at "` |
| **Java** | `"Exception in thread"` / `"Caused by:"` | `"at com.example.Class.method"` | Строка не начинается с `"at "` |

**Фильтрация stdlib фреймов:**
- Rust: `std::`, `core::`, `tokio::`, `alloc::`, `rustc_`
- Python: `site-packages`
- Node.js: `node_modules`, `node:internal`, `(internal/`
- Java: `java.`, `javax.`, `sun.`, `jdk.`, `org.junit.`

**Минимум 2 фрейма** — защита от ложных срабатываний (одиночные строки типа `"TypeError: undefined"` без фреймов игнорируются).

---

### `docker/` — мониторинг Docker контейнеров

**`docker/mod.rs`** — основной монитор:
- `bollard::Docker::connect_with_local_defaults()` — автоматически находит Docker (Unix socket / Windows named pipe)
- Получает список running контейнеров
- Для каждого запускает отдельную goroutine-подобную task для стриминга логов
- Retry loop каждые 10 секунд если Docker недоступен

**`docker/demux.rs`** — парсер multiplexed stream:
Docker отправляет логи с 8-байтным заголовком: `[stream_type(1), 0, 0, 0, size(4 BE)]`. Bollard парсит это автоматически, но модуль содержит утилиты для тестирования.

**`docker/log_filter.rs`** — фильтрация:
```
stdout JSON  →  читаем поле "level"/"severity"/"lvl"
                 ERROR/WARN/FATAL → пропускаем
                 INFO/DEBUG/TRACE → отбрасываем
stdout plain →  ищем ключевые слова error/warn/fatal
stderr       →  всегда пропускаем (stderr всегда важен)
```

**`docker/error_store.rs`** — хранилище:
```
HashMap<container_name, VecDeque<ErrorEvent>>
                              ↑
                        cap 500 событий (FIFO)
```

---

### `daemon_state.rs` — рефакторинг стейта

До Phase 2 каждая функция принимала отдельные параметры:
```rust
// Phase 1 (было)
fn get_snapshot(id, buf: SharedBuffer, cwd: PathBuf, start_time: Instant)
fn get_terminal_buffer(id, buf: SharedBuffer, args)
```

После:
```rust
// Phase 2 (стало)
fn get_snapshot(id, state: &DaemonState)
fn get_terminal_buffer(id, state: &DaemonState, args)
```

Это важно для масштабируемости — каждый новый компонент (drain, error_store) добавляется в один стейт, а не протаскивается через все сигнатуры.

---

## Ring Buffer + Drain: как они связаны

```rust
// tcp_bridge получает строку из VS Code → вызывает:
push_line_and_drain(buf, drain, text)
    │
    ├── push_line(buf, text)
    │       → ANSI strip
    │       → LogLine { text, timestamp_ms }
    │       → buf.push_back(line) [max 5000]
    │
    └── drain::ingest_line(drain, &line)
            → токенизация
            → поиск в prefix_tree
            → обновление/создание кластера
```

**Итог:** Буфер хранит всё (последние 5000 строк), Drain хранит сжатые кластеры. `get_terminal_buffer` читает буфер напрямую, `get_compressed_errors` читает Drain + перепарсивает стек-трейсы из буфера.

---

## Тестирование

**Стратегия:** Настоящий integration test — поднимаем `blackbox-daemon` как subprocess, шлём JSON-RPC через stdin/stdout.

**Секции в `blackbox-test`:**

| # | Что тестируется |
|---|----------------|
| 1 | `tools/list` возвращает ровно 7 инструментов |
| 2 | `get_snapshot` возвращает все ожидаемые поля |
| 3 | `get_terminal_buffer` возвращает injected строку |
| 4 | XML injection blocked (`<script>` экранируется) |
| 5 | `get_project_metadata` находит Cargo.toml |
| 6 | `read_file` читает файл, path traversal заблокирован |
| 7 | `get_contextual_diff` возвращает валидный JSON |
| 8 | 50 одинаковых ошибок → 1 кластер в Drain (count≥50) |
| 9 | Rust panic → `stack_traces` не пустой, user-code frame есть |
| 10 | `get_contextual_diff` возвращает корректный JSON |
| 11 | `get_container_logs` без Docker → `docker_available: false` |

---

## Что NOT сделано в Phase 2 (лежит в `improvements.md`)

### Баги перед Phase 3
- **VS Code TCP reconnect** — простой backoff, нужен exponential с jitter
- **Docker monitor** — при разрыве соединения перезапускает ВСЕ контейнеры, нужен per-container reconnect
- **`get_contextual_diff` path matching** — сравнение путей по строке, может не совпасть если один путь relative а другой absolute
- **Node.js single-frame errors** — порог в 2 фрейма может пропустить реальные ошибки Node.js с одним фреймом

### Phase 3 цели
- **Lock-free ring buffer** — заменить `Arc<RwLock<VecDeque>>` на Disruptor pattern
- **ANSI state-machine** — заменить regex на proper state machine
- **Typed context system** — вместо ручного экранирования XML-тегов
- **OS-level PTY interception** — перехват вывода на уровне псевдотерминала
- **PII masking** — маскировка персональных данных в логах

### Архитектурные улучшения (низкий приоритет)
- Слить `get_terminal_buffer` и `get_compressed_errors` в один инструмент с параметром `mode: "raw" | "compressed"`
- Заменить линейный скан Drain на trie/inverted index для высокой нагрузки
- Заменить `git diff` subprocess на pure-Rust `gix-diff` API

---

## Структура файлов

```
BlackBox/
├── crates/
│   ├── blackbox-core/src/
│   │   ├── types.rs          ← все shared типы (LogLine, LogCluster, DiffHunk, ErrorEvent...)
│   │   └── protocol.rs       ← JsonRpcRequest/Response
│   │
│   ├── blackbox-daemon/src/
│   │   ├── main.rs           ← точка входа, 4 tokio tasks
│   │   ├── daemon_state.rs   ← DaemonState struct [NEW Phase 2]
│   │   ├── buffer.rs         ← ring buffer + ANSI stripping + push_line_and_drain
│   │   ├── tcp_bridge.rs     ← TCP сервер :8765, принимает данные от VS Code
│   │   ├── status_server.rs  ← HTTP сервер :8766 для TUI
│   │   ├── mcp/
│   │   │   ├── mod.rs        ← JSON-RPC dispatcher (initialize/tools/list/tools/call)
│   │   │   └── tools.rs      ← все 7 инструментов + fallback цепочки
│   │   ├── scanners/
│   │   │   ├── drain.rs      ← Drain алгоритм [NEW Phase 2]
│   │   │   ├── stacktrace.rs ← парсер стек-трейсов [NEW Phase 2]
│   │   │   ├── git.rs        ← scan_git + get_changed_files + get_diff_hunks [расширен Phase 2]
│   │   │   ├── manifests.rs  ← сканер Cargo.toml/go.mod/package.json
│   │   │   └── env.rs        ← сканер .env ключей (без значений)
│   │   └── docker/
│   │       ├── mod.rs        ← run_docker_monitor [NEW Phase 2]
│   │       ├── demux.rs      ← парсер Docker multiplexed stream [NEW Phase 2]
│   │       ├── log_filter.rs ← фильтр ERROR/WARN/FATAL [NEW Phase 2]
│   │       └── error_store.rs ← SharedErrorStore [NEW Phase 2]
│   │
│   ├── blackbox-tui/         ← ratatui дашборд (мониторинг демона)
│   ├── blackbox-sandbox/     ← интерактивная песочница (8 вкладок)
│   └── blackbox-test/        ← integration тесты (11 секций)
│
├── CLAUDE.md                 ← инструкции для AI агентов по проекту
├── PHASE2_OVERVIEW.md        ← этот файл
└── improvements.md           ← технический долг и планы
```
