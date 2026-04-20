# 10. External Logs (File Watcher)

> [!IMPORTANT]
> **Repomix Context Command:**
> `repomix --include "crates/blackbox-daemon/src/file_watcher.rs" --output watcher_context.txt`

BlackBox позволяет ИИ подписываться на изменения в любых текстовых файлах (логов) на диске. Это полезно для мониторинга баз данных (PostgreSQL log), фоновых воркеров или системных журналов, которые не пишут в `stdout`.

## Mechanics
Модуль использует библиотеку `notify` для низкоуровневого отслеживания событий файловой системы (inotify на Linux, FSEvents на macOS, ReadDirectoryChangesW на Windows).

1.  **Subscription**: При вызове `watch_log_file(path)` демон открывает файл и переходит в режим `tail -f`.
2.  **Tagging**: Каждая строка, прочитанная из файла, попадает в общий буфер с тегом `file:<filename>`.
3.  **Unified Context**: Когда ИИ вызывает `get_terminal_buffer`, он может фильтровать вывод по тегу, чтобы видеть только логи конкретного файла или смешанный поток (терминал + файлы) в хронологическом порядке.

## MCP Tools
*   `watch_log_file(path)`: Начать отслеживание. Путь может быть относительным (от корня проекта) или абсолютным.
*   `get_watched_files()`: Показывает список активных подписок и статус (активен/ошибка доступа).

## Automatic Discovery
BlackBox автоматически пытается начать отслеживание следующих путей при старте (если они существуют):
*   `./*.log` (в корневом каталоге).
*   `./logs/*.log`.
*   `./npm-debug.log`.

## Usage Example
Если вы отлаживаете взаимодействие с базой данных:
1.  Вызовите `watch_log_file("/var/log/postgresql/postgresql-15-main.log")`.
2.  Запустите свой код.
3.  При возникновении ошибки ИИ увидит не только `Internal Server Error` в терминале, но и детальный лог `Statement violates foreign key constraint` из файла БД, скоррелированный по времени.
