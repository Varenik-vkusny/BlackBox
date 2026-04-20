# 01. Architecture

> [!IMPORTANT]
> **Repomix Context Command:**
> `repomix --include "Cargo.toml,crates/blackbox-core/src/**,crates/blackbox-daemon/src/main.rs,crates/blackbox-daemon/src/daemon_state.rs" --output architecture_context.txt`

## High-level Overview
BlackBox — это "бортовой самописец" (Flight Recorder) рабочего окружения разработчика. Он решает проблему **реактивности** существующих AI-агентов (таких как Claude Code или Cursor), которые видят только текущую сессию. 

BlackBox **пассивно** агрегирует системные потоки (Terminal, Docker logs, Git state, HTTP traffic) в реальном времени. Это позволяет AI получить моментальный снимок того, что произошло *до* запроса или в *параллельных* процессах, избавляя разработчика от ручного копирования ошибок из терминала.

## System Architecture
Система построена как фоновый демон на Rust, взаимодействующий с внешним миром через несколько каналов.

```mermaid
graph TD
    subgraph "IDE Layer"
        VS["VS Code Extension"]
    end

    subgraph "BlackBox Daemon (Rust)"
        TB["TCP Bridge (:8765)"]
        HP["HTTP Proxy (:8769)"]
        FW["File Watcher"]
        DS["DaemonState (Shared)"]
        RB["RingBuffer (5000 lines)"]
        DC["Drain Clustering"]
        ES["ErrorStore (Docker)"]
        SS["StructuredStore (JSON)"]
        HS["HttpStore (4xx/5xx)"]
        MCP["MCP Engine (stdio)"]
        TC["TypedContext (XML Guard)"]
    end

    subgraph "External Systems"
        Docker["Docker Engine"]
        Git["Gitoxide (gix)"]
        Network["Network Traffic"]
        Files["External Logs"]
    end

    VS -- "Terminal Data (TCP)" --> TB
    Network -- "HTTP Proxy" --> HP
    Files -- "inotify/Notify" --> FW
    
    TB --> RB
    HP --> HS
    FW --> RB
    
    RB --> DC
    RB --> SS
    
    DS -. "Manages" .-> RB
    DS -. "Manages" .-> DC
    DS -. "Manages" .-> ES
    DS -. "Manages" .-> SS
    DS -. "Manages" .-> HS
    
    Docker -- "bollard (Stream)" --> ES
    MCP -- "Wrapped Context" --> AI["AI Client (Claude Code)"]
    AI -- "Secure output" --> TC
    TC -- "Safe Data" --> AI
    
    DS <-> MCP
```

### Core Components
1.  **SharedBuffer (`buffer.rs`)**:
    *   Реализован как **Producer-Consumer** очередь (`crossbeam_queue::ArrayQueue`) с фоновой записью в `RwLock<VecDeque<LogLine>>`.
    *   Это обеспечивает **Lock-free ingestion**: терминалы и другие источники никогда не блокируются при записи логов.
    *   Кольцевой буфер с фиксированной емкостью **5000 строк**.
    *   Автоматически очищает входящие данные от ANSI-кодов и маскирует PII (Regex + Entropy) перед сохранением.
1a. **Native Capture (`pty_capture.rs`)**:
    *   Использует `portable-pty` для прямого захвата сессий (ConPTY на Windows).
    *   Позволяет BlackBox работать автономно без плагинов IDE.
2.  **HttpProxy (`http_proxy.rs`)**:
    *   Слушает на порту 8769. Позволяет перехватывать HTTP-ошибки (4xx/5xx).
    *   Поддерживает прозрачное проксирование и заголовок `X-Proxy-Target`.
3.  **File Watcher (`file_watcher.rs`)**:
    *   Мониторит произвольные файлы на диске. Новые строки попадают в общий буфер с соответствующим тегом источника.
4.  **StructuredStore (`structured_store.rs`)**:
    *   Специализированное хранилище для JSON-логов. Позволяет выполнять поиск по `span_id` и коррелировать события между разными сервисами.
5.  **TypedContext (`typed_context.rs`)**:
    *   Слой безопасности (Guard), который оборачивает все данные, отправляемые ИИ, в XML-теги с атрибутами доверия (`untrusted="true"`). Это предотвращает Prompt Injection.

## Data Flow (Unified Ingestion)
1.  **Ingestion**: Данные поступают из VS Code (TCP), HTTP-трафика (Proxy) и файловой системы (Watcher).
2.  **Processing**: Демон пропускает каждую сущность через:
    *   `ansi::strip_ansi` (очистка).
    *   `pii_masker::mask_pii` (маскировка секретов, включая тела HTTP-запросов).
    *   `drain::ingest_line` (кластеризация).
3.  **Cross-Source Correlation**: Благодаря единому источнику времени, инструмент `get_correlated_errors` может показать, что ошибка в терминале совпала по времени с падением Docker-контейнера и 500-й ошибкой в API.
4.  **Retrieval**: AI получает контекст через MCP инструменты, защищенные `TypedContext`.
