---
title: 03 Scanners Logic
synopsis: Details on how BlackBox processes data: Drain clustering, stack trace parsing (Rust, Python, Node, Java), Git state, manifest detection, and PII masking.
agent_guidance: Use this to understand how "raw" terminal output is transformed into structured insights like clusters or stack traces.
related: [04_DOCKER_AND_SYSTEM.md, 07_TESTING_AND_SECURITY.md]
---

# 03. Scanners Logic

> [!IMPORTANT]
> **Repomix Context Command:**
> `repomix --include "crates/blackbox-daemon/src/scanners/**" --output scanners_context.txt`

## 1. Log Clustering (Drain Algorithm)
Для сжатия логов используется модифицированный алгоритм **Drain**. Он позволяет хранить "шаблон" ошибки вместо тысяч идентичных строк, экономя контекстное окно ИИ.

*   **Logic**: Группировка строк по количеству токенов и схожести (threshold 0.5).
*   **Wildcarding**: Замена переменных частей (IP, ID, пути) на `*`.
*   **Result**: 1000 строк `Connection refused to 127.0.0.1:5432` превращаются в один шаблон.

## 2. Stacktrace Parsers
Модуль `stacktrace.rs` извлекает структурированную информацию из "сырого" текста терминала.
*   **Languages**: Rust, Python, Node.js/TS, Java.
*   **Cross-Reference**: Извлеченные пути к файлам используются в `get_contextual_diff` для автоматического подбора диффов.

## 3. Git Scanner (`git.rs`)
Глубокая интеграция с Git для понимания состояния проекта.
*   **State Detection**: Использует `gix` (Gitoxide) для мгновенного получения имени ветки и статуса HEAD.
*   **Diff Engine**: Вызывает `git` subprocess для генерации унифицированных диффов. Поддерживает `--no-index` для отслеживания изменений в новых (untracked) файлах.
*   **History Scraper**: Инструмент `get_recent_commits` парсит вывод `git log` с кастомным форматом, извлекая статистику (insertions/deletions) и список затронутых файлов.

## 4. Manifest Scanner (`manifests.rs`)
Автоматически определяет тип проекта, сканируя корень на наличие манифестов.
*   **Supported**: `Cargo.toml` (Rust), `package.json` (NPM/JS), `go.mod` (Go), `requirements.txt`/`pyproject.toml` (Python).
*   **Metadata**: Извлекает название проекта и зависимости для обогащения снимка `get_snapshot`.

## 5. Environment Scanner (`env.rs`)
Сканирует файлы `.env` и переменные окружения.
*   **PII Security**: Значения ключей **никогда** не передаются ИИ. Сканер возвращает только список имен переменных (например, `DATABASE_URL`, `STRIPE_KEY`), чтобы ИИ знал об их наличии, но не видел содержимое.
