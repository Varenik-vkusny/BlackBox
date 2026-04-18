# 02. MCP Tools Reference

> [!IMPORTANT]
> **Repomix Context Command:**
> `repomix --include "crates/blackbox-daemon/src/mcp/**,crates/blackbox-core/src/types.rs" --output mcp_context.txt`

## Protocol
BlackBox реализует стандартный **Model Context Protocol (MCP)** поверх `stdio`. Транспорт использует JSON-RPC 2.0. Демон слушает входящие сообщения на `stdin` и отвечает в `stdout`.

## Tool Catalog

| Инструмент | Описание | Основные параметры |
| :--- | :--- | :--- |
| `get_snapshot` | Быстрый срез статуса системы. | - |
| `get_terminal_buffer` | Последние строки терминала (ANSI-cleaned). | `lines` (default: 100, max: 500) |
| `get_project_metadata` | Список манифестов и ключи из `.env`. | - |
| `read_file` | Безопасное чтение файлов проекта. | `path` (req), `from_line`, `to_line` |
| `get_compressed_errors` | Кластеры ошибок и стек-трейсы. | `limit` (default: 50) |
| `get_contextual_diff` | Дифф файлов, упомянутых в ошибках. | - |
| `get_container_logs` | Фильтрованные логи Docker (ERROR/WARN). | `container_id` (opt), `limit` |
| `get_postmortem` | Анализ инцидента за период (таймлайн). | `minutes` (default: 30) |
| `get_correlated_errors`| Корреляция терминала и Docker по времени. | `window_secs` (default: 5) |

### Key Tools Depth

#### `get_snapshot`
Возвращает `daemon_uptime_secs`, `project_type` (cargo, npm, go), текущую `git_branch`, количество `git_dirty_files` и статистику буфера. ИИ должен вызывать этот инструмент первым.

#### `get_compressed_errors`
Использует алгоритм **Drain** для группировки сотен похожих ошибок в один кластер. Также содержит `stack_traces` — структурированные данные о падениях (Rust, Python, Node, Java).

#### `get_contextual_diff`
Самый "умный" инструмент. Он находит файлы, которые:
1. Имеют незакоммиченные изменения в Git.
2. Одновременно фигурируют в последних стек-трейсах из терминала.
Это позволяет ИИ сразу видеть код, который, вероятно, вызвал ошибку.

## Fallback System
BlackBox проектировался так, чтобы ИИ **никогда** не получал пустой или бесполезный ответ. Инструменты имеют цепочку фолбэков:

1. **Smart Data** (например, Contextual Diff). Если данных нет →
2. **Intermediate Data** (Compressed Errors). Если данных нет →
3. **Raw Data** (Terminal Buffer). Если данных нет →
4. **Explanation**: Ответ с полем `fallback_source: "none"` и причиной (например, "буфер еще пуст").

Это гарантирует, что AI-агент всегда имеет отправную точку для исследования.
