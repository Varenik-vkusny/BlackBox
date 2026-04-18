# 04. Docker and System Integration

> [!IMPORTANT]
> **Repomix Context Command:**
> `repomix --include "crates/blackbox-daemon/src/docker/**" --output docker_context.txt`

## Docker Streaming
BlackBox интегрируется с Docker Engine через библиотеку **Bollard**, используя локальные сокеты (Unix socket на Linux/macOS или Named Pipes на Windows).

*   **Discovery**: При старте (и каждые 10 секунд при потере связи) демон сканирует запущенные контейнеры.
*   **Per-container tasks**: Для каждого контейнера запускается отдельный асинхронный таск, который слушает поток логов (`logs --follow`).
*   **Resilience**: Если контейнер перезапускается или падает, таск мониторинга этого конкретного контейнера уходит в цикл ретраев, не затрагивая мониторинг остальных.

## Log Demuxing
Docker передает логи в мультиплексированном формате: каждый пакет начинается с 8-байтного заголовка, где первый байт указывает тип стрима (1 - stdout, 2 - stderr), а последние 4 байта — размер полезной нагрузки.
Модуль `docker::demux` (и обертки в `bollard`) корректно разделяют эти потоки, позволяя BlackBox применять разные правила фильтрации.

## Filtering Logic (`log_filter.rs`)
Чтобы не забивать память и контекст ИИ лишним шумом, BlackBox сохраняет только критические события из Docker:

1.  **Stderr**: Всегда сохраняется полностью (в stderr обычно пишутся системные ошибки и паники).
2.  **Stdout (JSON)**: Если лог в формате JSON (например, Logrus, Zap), парсер ищет поля `level`, `severity` или `lvl`. Сохраняются только: `ERROR`, `FATAL`, `WARN`.
3.  **Stdout (Plain text)**: Поиск ключевых слов (case-insensitive): `error`, `panic`, `fatal`, `exception`, `warn`.
4.  **Discard**: Обычные `INFO` и `DEBUG` сообщения игнорируются.

## Error Store (`error_store.rs`)
События сохраняются в `SharedErrorStore` — конкурентном хранилище на базе `HashMap`.
*   **Capacity**: По умолчанию хранится последних **500 событий** на каждый контейнер.
*   **Querying**: Инструмент `get_container_logs` может возвращать логи конкретного контейнера или общую ленту всех системных ошибок, отсортированную по времени.
