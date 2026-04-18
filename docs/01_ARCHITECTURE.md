# 01. Architecture

> [!IMPORTANT]
> **Repomix Context Command:**
> `repomix --include "Cargo.toml,crates/blackbox-core/src/**,crates/blackbox-daemon/src/main.rs,crates/blackbox-daemon/src/daemon_state.rs" --output architecture_context.txt`

## High-level Overview
BlackBox — это "бортовой самописец" (Flight Recorder) рабочего окружения разработчика. Он решает проблему **реактивности** существующих AI-агентов (таких как Claude Code или Cursor), которые видят только текущую сессию. 

BlackBox **пассивно** агрегирует системные потоки (Terminal, Docker logs, Git state) в реальном времени. Это позволяет AI получить моментальный снимок того, что произошло *до* запроса или в *параллельных* процессах, избавляя разработчика от ручного копирования ошибок из терминала.

## System Architecture
Система построена как фоновый демон на Rust, взаимодействующий с внешним миром через несколько каналов.

```mermaid
graph TD
    subgraph "IDE Layer"
        VS["VS Code Extension"]
    end

    subgraph "BlackBox Daemon (Rust)"
        TB["TCP Bridge (:8765)"]
        DS["DaemonState (Shared)"]
        RB["RingBuffer (5000 lines)"]
        DC["Drain Clustering"]
        ES["ErrorStore (Docker)"]
        MCP["MCP Engine (stdio)"]
        SS["Status Server (:8766)"]
        AA["Admin API (:8768)"]
    end

    subgraph "External Systems"
        Docker["Docker Engine"]
        Git["Gitoxide (gix)"]
    end

    subgraph "AI Client"
        Claude["Claude / Cursor"]
    end

    VS -- "Terminal Data (TCP)" --> TB
    TB --> RB
    TB --> DC
    DS -. "Manages" .-> RB
    DS -. "Manages" .-> DC
    DS -. "Manages" .-> ES
    Docker -- "bollard (Stream)" --> ES
    MCP -- "Context" --> Claude
    DS <-> MCP
    DS <-> SS
    DS <-> AA
```

### Core Components
1.  **SharedBuffer (`buffer.rs`)**:
    *   Реализован как `Arc<RwLock<VecDeque<LogLine>>>`.
    *   Кольцевой буфер с фиксированной емкостью **5000 строк**.
    *   Автоматически очищает входящие данные от ANSI-кодов и маскирует PII (Personal Identifiable Information) перед сохранением.
2.  **DaemonState (`daemon_state.rs`)**:
    *   Центральная структура, объединяющая ссылки на все хранилища данных (`buf`, `drain`, `error_store`).
    *   Клонируется для каждого асинхронного таска (дешёвое клонирование за счёт `Arc`).
3.  **TCP Bridge (`tcp_bridge.rs`)**:
    *   Листенер на порту 8765. Принимает поток данных из VS Code и передает их в `push_line_and_drain`.

## Data Flow
1.  **Ingestion**: VS Code расширение подписывается на `onDidWriteTerminalData` и пересылает каждый чанк по TCP на демон.
2.  **Processing**: Демон пропускает строку через:
    *   `ansi::strip_ansi` (очистка цветов/курсора).
    *   `pii_masker::mask_pii` (маскировка секретов и email).
    *   `drain::ingest_line` (группировка в кластеры).
3.  **Storage**: Обработанная строка (`LogLine`) попадает в `SharedBuffer`.
4.  **Retrieval**: AI через MCP вызывает инструменты (например, `get_terminal_buffer`), которые извлекают данные из `SharedBuffer` и оборачивают их в XML-теги безопасности перед отправкой.
