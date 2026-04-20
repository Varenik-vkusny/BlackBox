# 04. Docker and System Integration

> [!IMPORTANT]
> **Repomix Context Command:**
> `repomix --include "crates/blackbox-daemon/src/docker/**" --output docker_context.txt`

## 1. Docker Streaming
BlackBox интегрируется с Docker Engine через библиотеку **Bollard**.
*   **Discovery**: Каждые 10 секунд демон проверяет наличие новых контейнеров.
*   **Resilience**: Если контейнер упал или Docker Desktop был перезапущен, таски мониторинга автоматически восстанавливаются без перезагрузки самого демона.
*   **Log Demuxing**: Корректное разделение `stdout` и `stderr` потоков с разбором 8-байтовых заголовков Docker.

## 2. Smart Filtering (`log_filter.rs`)
BlackBox не хранит все логи контейнеров. Сохраняются только:
1.  **Stderr** (полностью).
2.  **Stdout** с уровнями: `ERROR`, `FATAL`, `WARN`.
3.  **Keywords**: Текстовые логи, содержащие `panic`, `exception`, `failed`.

## 3. Error Store & Querying
События сохраняются в `SharedErrorStore` (последние 500 событий на контейнер).
*   Инструмент `get_container_logs` возвращает ленту ошибок с метаданными контейнера.

## 4. Cross-Source Correlation (Unified Timeline)
Ключевая фича Phase 3 — объединение данных Docker с другими источниками.
*   **Tool**: `get_correlated_errors`.
*   **Logic**: Если вы видите ошибку в терминале, BlackBox ищет события в Docker и HTTP-прокси в окне **±5 секунд**.
*   **Insight**: Это позволяет сразу понять, что `502 Bad Gateway` в браузере был вызван тем, что Docker-контейнер упал со `OOMKilled` ровно в ту же секунду.
