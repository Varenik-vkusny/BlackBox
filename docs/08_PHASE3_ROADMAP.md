---
title: 08 Phase 3 Roadmap (COMPLETED)
synopsis: History of Phase 3 accomplishments and the backlog for Phase 4 (Intelligence, SQLite storage, Distributed context).
agent_guidance: Use this to understand what features are currently implemented vs. planned "backlog" features.
related: []
---

# 08. Phase 3 Roadmap (COMPLETED)

BlackBox Phase 3 завершена. Основные цели по нативному перехвату, расширенной аналитике и сетевому проксированию достигнуты.

## 1. Native Aggregation (Done)
*   **File Watcher**: Прямое чтение внешних логов без посредников.
*   **Network Interception**: Полноценный HTTP-прокси для ловли ошибок API.
*   **Native PTY Capture**: Автоматический захват консоли через `portable-pty` (ConPTY/PTY).

## 2. Advanced Correlation & Performance (Done)
*   **Cross-Source Timeline**: Объединение Terminal + Docker + HTTP в единую временную шкалу.
*   **Structured Parsing**: Глубокий разбор JSON-логов и поддержка Trace-ID/Span-ID.
*   **Lock-Free Buffer**: Переход на Producer-Consumer архитектуру с использованием `crossbeam-queue` для устранения задержек при записи.

## 3. Security Hardening (Done)
*   **Injection Shield**: Усиленная семантическая изоляция данных в XML-обертке с защитными метаданными.
*   **Unified PII Masking**: Расширенная база регулярных выражений и энтропийный сканер (ML-модель отложена для сохранения легкости MCP-сервера).

---

## Phase 4: Intelligence & Scale (Backlog)

### 1. AI-Driven Filtering
Текущая фильтрация основана на правилах. Планируется переход на локальную LLM (через `candle`) для умного "рейтингования" важности логов.

### 2. Persistent Storage (SQLite)
Переход от RingBuffer в памяти к SQLite для хранения истории инцидентов за дни и недели.

### 3. Distributed Context
Поддержка сбора данных с нескольких удаленных серверов/демонов в единый контекст для ИИ.
