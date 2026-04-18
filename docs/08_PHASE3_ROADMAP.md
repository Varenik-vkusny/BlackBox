# 08. Phase 3 Roadmap

> [!IMPORTANT]
> **Repomix Context Command:**
> `repomix --include "improvements.md,PHASE2_OVERVIEW.md" --output roadmap_context.txt`

BlackBox Phase 2 заложила фундамент для глубокой аналитики. Phase 3 сфокусирована на производительности, нативном перехвате данных и приватности.

## 1. Native OS Interception (PTY)
Текущая зависимость от VS Code расширения ограничивает BlackBox только редактором.
*   **Target**: Прямой перехват системного PTY (Linux/macOS) и ConPTY (Windows).
*   **Result**: BlackBox начнет видеть логи даже если разработчик работает в обычном терминале (iTerm2, Alacritty, PowerShell), а не только внутри VS Code.

## 2. Lock-free Architecture
Для работы под высокой нагрузкой (десятки тысяч строк логов в секунду) текущий `Arc<RwLock<VecDeque>>` будет заменен на паттерн **LMAX Disruptor**.
*   **Optimization**: Читатели (MCP инструменты) не будут блокировать писателей (Terminal Bridge) даже при интенсивном анализе.
*   **Zero-Copy**: Использование `Bytes` и слайсов памяти для минимизации аллокаций при передаче данных между модулями.

## 3. Advanced PII Masking (ML Based)
Текущая маскировка на регулярных выражениях хорошо ловит стандартные ключи, но пропускает сложные контекстные данные.
*   **Implementation**: Интеграция локальной легковесной ML-модели (на базе `candle` или `rust-bert`) для Named Entity Recognition (NER).
*   **Privacy**: Модель будет работать полностью локально на CPU, удаляя имена людей, адреса и специфические названия сущностей из логов перед отправкой во внешние LLM.

## 4. Architectural Refactoring
*   **Tool Fusion**: Объединение `get_terminal_buffer` и `get_compressed_errors` в единый интерфейс запросов с флагами детальности.
*   **Pure-Rust Git**: Замена вызовов `git` subprocess на прямое использование API `gix` (Gitoxide) для ускорения работы `get_contextual_diff`.
*   **ANSI State Machine**: Переход от регулярных выражений к полноценному конечному автомату для корректной обработки сложных escape-последовательностей (например, управление цветами фона или сложные перемещения курсора).
