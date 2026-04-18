# BlackBox — Roadmap Ideas

Идеи для следующих сессий. Каждая описана достаточно детально чтобы реализовать без дополнительного контекста.

---

## Идея 1 — File Watcher (логи в файлах)

### Проблема
Большинство production-приложений пишут логи не в терминал, а в файл: `app.log`, `error.log`, `debug.log`, `logs/server.log`. Сейчас BlackBox их вообще не видит — VS Code bridge ловит только то что выводится в терминал IDE. Если разработчик запустил сервер через systemd, Docker (без compose в терминале), или просто редиректнул `> app.log`, агент слепой.

### Что реализовать
Добавить в `DaemonState` список watched файлов и фоновую задачу на каждый файл.

**Механизм:**
- Crate `notify` (4.x или 5.x, cross-platform) — обёртка над inotify (Linux), FSEvents (macOS), ReadDirectoryChangesW (Windows)
- При старте демона: автоматически искать `*.log`, `logs/`, `log/` в cwd (эвристика)
- Читать только новые байты (tail-режим): хранить `file_offset: u64` на каждый файл, при событии изменения читать от offset до EOF
- Каждую новую строку прогонять через тот же pipeline: ANSI strip → PII mask → `push_line_and_drain`
- Источник логов помечать как `source: "file:<relative_path>"` в LogLine (потребует добавить поле source в LogLine)

**Новый MCP инструмент:** `watch_log_file(path: str)` — подписаться на конкретный файл вручную (если автоэвристика не нашла).

**Admin API endpoint:** `POST /api/watch` `{ "path": "logs/app.log" }` — для ручного добавления через TUI/Lab UI.

**Граничные случаи:**
- Log rotation (файл пересоздаётся) — detect by inode change, reset offset to 0
- Файл растёт быстро (verbose debug) — rate limit: не более 1000 строк/сек на файл
- Бинарные файлы — проверить первые 512 байт на UTF-8 валидность перед подпиской

---

## Идея 2 — Process stdout/stderr Capture

### Проблема
Разработчик запускает `node server.js`, `python manage.py runserver`, `./my-binary` — не через VS Code терминал, а через npm scripts, Makefile, systemd unit, launchd plist, или просто в отдельном окне терминала вне IDE. Логи этих процессов невидимы для BlackBox.

### Что реализовать
Механизм подписки на stdout/stderr конкретного процесса по PID.

**Вариант A (проще) — `/proc/<pid>/fd/1` на Linux/macOS:**
- На Linux можно читать `/proc/<pid>/fd/1` (stdout pipe) если процесс наш потомок, или использовать `ptrace`/`strace -p <pid> -e write` для чужого процесса
- macOS: `dtrace` или `lldb attach`
- Сложно, требует прав

**Вариант B (рекомендуется) — wrapper script:**
- Демон создаёт скрипт-обёртку `blackbox-run` который запускает процесс и пайпит его вывод одновременно в stdout И в TCP 127.0.0.1:8765
- Разработчик: `blackbox-run node server.js` вместо `node server.js`
- Реализация через `tee`-паттерн: читать stdout/stderr дочернего процесса, писать в обе стороны
- Можно добавить `blackbox-run` как alias в shell hooks

**Новый MCP инструмент:** `get_process_logs(pid: int)` — логи конкретного процесса отдельно от общего буфера.

**Admin API:** `POST /api/attach` `{ "pid": 12345 }` — подписаться на процесс по PID.

---

## Идея 3 — HTTP Request Logger (перехват запросов)

### Проблема
Stack trace говорит `at handleRequest (server.js:87)` — но агент не знает ЧТО за запрос вызвал ошибку: какой метод, какой endpoint, какое тело. Связь между HTTP-ошибкой и кодом отсутствует. Агент видит симптом но не причину.

### Что реализовать
Локальный HTTP proxy на порту 8769 который перехватывает запросы и логирует только ошибки (4xx, 5xx).

**Механизм:**
- Запустить `axum`-based reverse proxy внутри демона (дополнительный порт 8769)
- Разработчик меняет свой app на `HTTP_PROXY=http://127.0.0.1:8769` или `HTTPS_PROXY`
- Proxy пропускает запросы транзитом, логирует: `method`, `url`, `status_code`, `latency_ms`, `request_body` (первые 512 байт), `response_body` (первые 512 байт при 4xx/5xx)
- Только ошибочные запросы сохраняются (не все — иначе переполнение)

**Корреляция с terminal ошибками:**
- Каждый HTTP event получает `timestamp_ms`
- `get_correlated_errors` уже умеет cross-reference по времени — HTTP events добавятся как третий источник
- Агент видит: `ERROR in server.js:87` произошло в то же время что `POST /api/users → 422`

**Граничный вопрос — контекст агента:**
Без фильтрации это может генерировать тысячи строк и переполнить контекст. Решение:
- Хранить только 4xx/5xx ответы (не весь трафик)
- Cap: 200 HTTP events в store (ring buffer как у Docker)
- `get_http_errors(limit: int)` возвращает только ошибочные запросы с телами truncated до 512 байт
- В `get_correlated_errors` добавить HTTP events как опциональный третий источник

---

## Идея 4 — get_recent_commits (история изменений)

### Проблема
Агент видит что файл сломан через `get_contextual_diff`, но не знает: это новый баг или регрессия? Когда последний раз менялся этот файл? Какой коммит мог сломать поведение? Без истории агент не может сказать "откатись к коммиту X".

### Что реализовать
Простой инструмент `get_recent_commits` — только сообщения и метаданные, **без diff'ов** (иначе переполнение контекста).

**Возвращаемые данные на коммит:**
```json
{
  "hash": "b77a266",
  "message": "Phase 3 completed: first version is ready to use",
  "author": "Varenik-vkusny",
  "timestamp_iso": "2026-04-17T14:23:11Z",
  "changed_files": ["crates/blackbox-daemon/src/mcp/tools.rs", "..."],
  "insertions": 142,
  "deletions": 38
}
```

**Почему НЕ будет проблемы с контекстом:**
- Только `hash` + `message` + список файлов (не сами diff'ы)
- Default limit: последние 20 коммитов
- Без `changed_files` это ~50 токенов на коммит = 1000 токенов на 20 коммитов — ничтожно мало
- С `changed_files` ~200 токенов на коммит = 4000 токенов — всё ещё нормально

**Killer feature — связь с текущими ошибками:**
Если stack trace упоминает `tools.rs`, `get_recent_commits` может автоматически фильтровать историю: показывать только коммиты которые трогали `tools.rs`. Агент сразу видит "последний раз tools.rs менялся 2 коммита назад, вот сообщение коммита".

**Реализация:** `git log --oneline --stat -n 20` через subprocess, парсинг в `ChangedFileCommit` struct. Добавить в `scanners/git.rs`.

**Параметры инструмента:** `get_recent_commits(limit: int = 20, path_filter: str = null)` — если передан путь, только коммиты затрагивающие этот файл.

---

## Идея 5 — Structured Log Parsing (tracing/winston/logrus)

### Проблема
Современные приложения используют structured logging: Rust `tracing`, Node.js `winston`/`pino`, Python `structlog`, Go `logrus`/`zap`. Они пишут JSON строки вида:
```json
{"level":"error","msg":"db query failed","span_id":"abc123","request_id":"req-456","query":"SELECT...","latency_ms":2300}
```
Сейчас BlackBox видит эту строку как одну строку текста. Парсит только `level` поле для фильтрации. Остальные поля (`span_id`, `request_id`, `query`, `latency_ms`) теряются.

### Что реализовать
В pipeline обработки строки (`push_line_and_drain`): после ANSI strip, перед PII mask — попытаться распарсить JSON.

**Если строка валидный JSON:**
- Извлечь поля: `level`, `msg`/`message`, `span_id`, `trace_id`, `request_id`, `error`, `stack`
- Сохранить как `StructuredLogLine { raw_json, level, message, fields: HashMap<String, Value> }`
- PII mask применять к `message` и string-значениям в `fields`

**Новый MCP инструмент** `get_structured_context(span_id: str)`:
- Агент видит span_id в stack trace → запрашивает все события с этим span_id
- Возвращает цепочку: `request received → db query started → db query failed → handler threw`
- Это настоящий distributed tracing без Jaeger/Zipkin — локально, без инфраструктуры

**Корреляция:**
- `get_compressed_errors` для structured logs группирует по `msg`-шаблону (не по raw строке)
- Разные span_id с одинаковым `msg: "db query failed"` → один кластер с `count: 47`
- Поле `error_store` расширяется: хранить `request_id` при наличии

**Поддерживаемые форматы** (автодетект по ключам):
- `tracing` (Rust): `{"timestamp":..., "level":"ERROR", "fields":{...}, "target":"..."}`
- `pino` (Node): `{"level":50, "time":..., "msg":"...", "pid":...}` — level числовой (50=error)
- `logrus` (Go): `{"level":"error", "msg":"...", "time":"..."}`
- `structlog` (Python): `{"event":"...", "level":"error", ...}`
