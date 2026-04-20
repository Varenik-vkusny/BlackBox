# 07. Testing and Security

> [!IMPORTANT]
> **Repomix Context Command:**
> `repomix --include "crates/blackbox-test/**,crates/blackbox-daemon/src/mcp/tools.rs" --output security_context.txt`

Безопасность и надежность — критические требования для системы, которая агрегирует логи разработки.

## Integration Testing (`blackbox-test`)
Вместо обычных Unit-тестов, BlackBox полагается на интеграционные сценарии:
1.  **Protocol Verification**: Тесты симулируют ИИ, отправляя JSON-RPC запросы в `stdin` демона.
2.  **Bridge Injection**: Тесты впрыскивают данные (паники, логи) по TCP и проверяют реакцию сканеров.
3.  **HTTP Proxy Testing**: Верификация того, что прокси корректно перехватывает 500-е ошибки и маскирует их.

## Security Layers

### 1. Unified PII Masking (`pii_masker.rs`)
Входящие логи и **тела HTTP-запросов** проходят через фильтр:
*   **Regex Scan**: Маскирует email, JWT, Bearer-токены.
*   **Secret Detection**: Ищет `password=SECRET`, `API_KEY: value`.
*   **Entropy Scanner**: Токены с энтропией выше **4.5** считаются случайными ключами и заменяются на `<SECRET_MASKED>`.

### 2. Typed Context Guard (Injection Protection)
Все данные в MCP-ответах оборачиваются в XML-теги с атрибутами доверия:
*   **Encapsulation**: Вывод терминала помещается в `<untrusted_content source="terminal">`.
*   **Escaping**: Потенциально опасные символы внутри контента экранируются, чтобы данные не могли быть интерпретированы как инструкции для ИИ.

### 3. Local-Only Network
*   **Daemon**: Принимает соединения только на `127.0.0.1`.
*   **Admin API**: Защищен CORS (разрешен только localhost), что предотвращает доступ к логам из внешних веб-сайтов.
