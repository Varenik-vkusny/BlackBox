# 03. Scanners Logic (Drain & Stacktrace)

> [!IMPORTANT]
> **Repomix Context Command:**
> `repomix --include "crates/blackbox-daemon/src/scanners/**" --output scanners_context.txt`

## Drain Algorithm
Для сжатия логов используется модифицированный алгоритм **Drain**. Он позволяет хранить "шаблон" ошибки вместо тысяч идентичных строк, экономя контекстное окно ИИ.

### Логика работы:
1.  **Tokenization**: Строка разбивается на токены по пробелам.
2.  **Grouping**: Кластеры группируются по количеству токенов. Это ускоряет поиск.
3.  **Similarity Check**: Для каждой новой строки ищется кластер с тем же числом токенов, где совпадает не менее **50% (threshold 0.5)** позиций.
4.  **Wildcarding**: Если строка подходит кластеру, несовпадающие токены в шаблоне заменяются на `*`.
5.  **Eviction**: Вместимость ограничена **1000 кластерами**. При переполнении удаляется самый старый (LRU по `last_seen_ms`).

**Результат:** Тысячи строк `Connection refused to 127.0.0.1:5432`, `Connection refused to 127.0.0.1:5433` превратятся в один кластер: `Connection refused to *`.

## Stacktrace Parsers
Модуль `stacktrace.rs` содержит State-machine парсеры для извлечения структурированной информации об ошибках.

### Поддерживаемые языки:
*   **Rust**: Детектирует `panicked at` и извлекает фреймы `N: module::func`, а также ссылки на файлы `at src/main.rs`.
*   **Python**: Распознает блоки `Traceback (most recent call last):` и фреймы `File "...", line N`.
*   **Node.js / TS**: Сканирует блоки, начинающиеся с `Error:` или `TypeError:`, и фреймы `at ... (file:line:col)`.
*   **Java**: Обрабатывает `Exception in thread`, `Caused by:` и фреймы `at class.method(file:line)`.

### Фильтрация и Кросс-референс:
*   **Whitelist/Blacklist**: Парсер автоматически помечает фреймы как `is_user_code: false`, если они принадлежат стандартным библиотекам (например, `std::`, `site-packages`, `node_modules`).
*   **File Extraction**: Список всех файлов, упомянутых в "пользовательских" фреймах, собирается в массив `source_files`. Это поле используется инструментом `get_contextual_diff` для автоматического подбора релевантных диффов.
*   **Security**: Минимум 2 фрейма требуется для подтверждения того, что блок является стек-трейсом (защита от ложных срабатываний).
