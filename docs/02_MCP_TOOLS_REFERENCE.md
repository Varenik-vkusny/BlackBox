---
title: 02 MCP Tools Reference
synopsis: Categorized catalog of all available Model Context Protocol tools (Core, Diagnostics, Infrastructure, Tracking, Advanced).
agent_guidance: Consult this whenever you need to know which tool to use for a specific diagnostic task or what parameters are available.
related: [03_SCANNERS_LOGIC.md, 07_TESTING_AND_SECURITY.md]
---

# 02. MCP Tools Reference

> [!IMPORTANT]
> **Repomix Context Command:**
> `repomix --include "crates/blackbox-daemon/src/mcp/**,crates/blackbox-core/src/types.rs" --output mcp_context.txt`

## Protocol
BlackBox реализует стандартный **Model Context Protocol (MCP)** поверх `stdio`. Демон слушает входящие сообщения на `stdin` и отвечает в `stdout`.

## Tool Catalog (Categorized)

### 1. Базовые инструменты (Core)
| Инструмент | Описание | Основные параметры |
| :--- | :--- | :--- |
| `get_snapshot` | Быстрый срез статуса (uptime, git, ошибки). | - |
| `get_terminal_buffer`| Последние строки терминала. | `lines`, `terminal` (source filter) |
| `get_project_metadata`| Срез манифестов и ключей `.env`. | - |

### 2. Глубокая диагностика (Diagnostics)
| Инструмент | Описание | Основные параметры |
| :--- | :--- | :--- |
| `get_compressed_errors`| Шаблоны ошибок (Drain) и стек-трейсы. | `limit` (max clusters) |
| `get_contextual_diff` | Дифф файлов, упомянутых в ошибках. | - |
| `get_postmortem` | Таймлайн инцидента за период. | `minutes` (1-1440) |
| `read_file` | Безопасное чтение файлов проекта. | `path`, `from_line`, `to_line` |

### 3. Инфраструктура и Сеть (Infrastructure)
| Инструмент | Описание | Основные параметры |
| :--- | :--- | :--- |
| `get_container_logs` | Ошибки Docker-контейнеров. | `container_id` (opt) |
| `get_http_errors` | Логи HTTP-ошибок (4xx/5xx) через прокси. | `limit` (max 200) |
| `get_correlated_errors`| Корреляция терминала, Docker и HTTP. | `window_secs` (time window) |

### 4. Отслеживание и История (Tracking)
| Инструмент | Описание | Основные параметры |
| :--- | :--- | :--- |
| `get_recent_commits` | История коммитов (stats, автор, время). | `limit`, `path_filter` (opt) |
| `watch_log_file` | Добавить внешний файл под наблюдение. | `path` (absolute or relative) |
| `get_watched_files` | Список всех отслеживаемых файлов. | - |

### 5. Расширенная аналитика (Advanced)
| Инструмент | Описание | Основные параметры |
| :--- | :--- | :--- |
| `get_structured_context`| Поиск по структурированным логам/спанам. | `span_id` (opt), `limit` |
| `get_process_logs` | Логи конкретного процесса по PID. | `pid` (opt), `limit` |

## Fallback System
BlackBox использует цепочку фолбэков, чтобы ИИ **всегда** имел данные для анализа:
1. **Contextual Data** (например, Diff). Если пусто ->
2. **Aggregated Data** (например, Compressed Errors). Если пусто ->
3. **Raw Data** (Terminal Buffer).
4. **Insight**: Объяснение (например, "Docker не запущен, использую данные терминала").

## Security & Injection Protection
Все данные, возвращаемые инструментами `get_terminal_buffer` и `get_process_logs`, проходят через **TypedContext Guard**, который:
* Оборачивает вывод в теги `<untrusted_content source="...">`.
* Экранирует опасные символы внутри контента.
* Позволяет ИИ понимать границы между системными данными и собственными инструкциями.
