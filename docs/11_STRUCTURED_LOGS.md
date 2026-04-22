---
title: 11 Structured Logs & Tracing
synopsis: Support for JSON logging, trace-ID/span-ID indexing, and breadcrumb navigation.
agent_guidance: Consult this when dealing with microservices or structured logging output (Pino, Tracing, Structlog) to leverage deep indexing.
related: [01_ARCHITECTURE.md, 03_SCANNERS_LOGIC.md]
---

# 11. Structured Logs & Tracing

> [!IMPORTANT]
> **Repomix Context Command:**
> `repomix --include "crates/blackbox-daemon/src/structured_store.rs" --output tracing_context.txt`

Для сложных микросервисных приложений простого текстового поиска недостаточно. BlackBox поддерживает разбор и индексацию структурированных логов.

## JSON Parsing
BlackBox автоматически детектирует JSON-строки в терминале или файлах. Если строка является валидным JSON, она попадает в `StructuredStore`.

### Key Fields Support
Демон ищет следующие стандартные поля для индексации:
*   `trace_id`, `traceId`, `correlation_id`.
*   `span_id`, `spanId`.
*   `level`, `severity`.
*   `msg`, `message`.

## Tracing Support
Благодаря индексации по `span_id`, BlackBox позволяет восстановить цепочку вызовов даже если они перемешаны в общем потоке логов с данными от других тасков.

### Tool: `get_structured_context`
Этот инструмент позволяет ИИ:
1.  **Filter by ID**: Получить все логи, связанные с конкретным `trace_id`.
2.  **Breadcrumbs**: Найти "родительские" спаны для текущей ошибки.

## Deep Integration with Git
Если JSON-лог содержит имя файла и номер строки (например, логи из Rust-библиотеки `tracing`), BlackBox автоматически использует их для инструмента `get_contextual_diff`, обеспечивая мгновенный переход от лога к проблемному участку кода.

## Best Practices for Developers
Чтобы BlackBox работал максимально эффективно, рекомендуется использовать структурированное логирование в своих приложениях:
*   **Rust**: `tracing` с `tracing-subscriber` (JSON formatter).
*   **Go**: `slog` или `zap`.
*   **Node.js**: `pino` или `winston`.
*   **Python**: `structlog`.
