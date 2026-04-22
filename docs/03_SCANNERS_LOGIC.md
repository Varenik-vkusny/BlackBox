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

## 1. Log Clustering (Drain3 Algorithm)
Для сжатия логов используется production-ready реализация алгоритма **Drain3**. Она позволяет хранить "шаблон" ошибки вместо тысяч идентичных строк, экономя контекстное окно ИИ.

*   **Pre-masking**: Перед токенизацией динамические значения заменяются статическими токенами (в порядке применения):
    *   Таймстампы: `2024-01-15T10:30:00Z` → `<TIMESTAMP>`, `Jan 15 10:30:00` → `<TIMESTAMP>`
    *   URL: `https://api.example.com/v1` → `<URL>`
    *   Email: `admin@example.com` → `<EMAIL>`
    *   UUID: `5f4dcc3b-2c00-...` → `<UUID>`
    *   IP: `192.168.1.1` → `<IP>`
    *   Git SHA: `a1b2c3d` → `<GIT_SHA>`
    *   Пути: `/tmp/data.txt`, `C:\Users\file.txt`, `./src/main.rs` → `<PATH>`
    *   Hex: `0xdeadbeef`, `cafe` → `<HEX>`
    *   Числа: `8080` → `<NUM>`
    *   Это уменьшает дисперсию длины шаблонов и повышает точность кластеризации.
*   **Trie Routing**: Вместо линейного поиска внутри bucket'ов по количеству токенов используется Prefix Tree (Trie) фиксированной глубины (до 4 токенов). Строка маршрутизируется по первым токенам к маленькому листу с кандидатами.
*   **Similarity**: `matching_tokens / token_count >= 0.5`. Проверяется только на кластерах внутри одного листа Trie.
*   **Wildcarding**: Отличающиеся токены заменяются на `*`. Благодаря pre-masking IP/UUID/числа не становятся `*`, а сохраняют семантику `<IP>`.
*   **Cap & Eviction**: Жёсткий лимит 1000 кластеров; при переполнении вытесняется least-recently-used через DFS-обход дерева.
*   **Result**: 1000 строк `Connection refused to 127.0.0.1:5432` превращаются в один шаблон `Connection refused to <IP>:<NUM>`.

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
