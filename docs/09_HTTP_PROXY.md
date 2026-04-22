---
title: 09 HTTP Proxy Integration
synopsis: Detailed documentation for the built-in HTTP proxy (:8769) for intercepting API errors.
agent_guidance: Use this when asked to monitor network traffic or when explaining how to set up HTTP_PROXY for an application.
related: [04_DOCKER_AND_SYSTEM.md, 07_TESTING_AND_SECURITY.md]
---

# 09. HTTP Proxy Integration

> [!IMPORTANT]
> **Repomix Context Command:**
> `repomix --include "crates/blackbox-daemon/src/http_proxy.rs" --output http_proxy_context.txt`

BlackBox включает в себя встроенный HTTP-прокси-сервер, позволяющий ИИ видеть ошибки сетевого взаимодействия, которые обычно остаются скрытыми внутри приложения.

## Overview
*   **Port**: `8769` (по умолчанию).
*   **Protocol**: HTTP/1.1 (поддержка `Upgrade` для WebSocket в разработке).
*   **Storage**: Сохраняет только запросы, завершившиеся с кодом `4xx` или `5xx`.

## How to Use

### 1. Transparent Proxy
Вы можете настроить свое приложение на использование BlackBox как стандартного HTTP-прокси:
```bash
# Пример для Node.js / Python
export HTTP_PROXY=http://127.0.0.1:8769
```

### 2. X-Proxy-Target Header
Если ваше приложение не поддерживает настройку прокси, вы можете отправлять запросы напрямую на порт BlackBox, указав реальный адрес в заголовке:
```http
GET /api/users HTTP/1.1
Host: 127.0.0.1:8769
X-Proxy-Target: https://api.production.internal
```

## Security & PII Masking
Прокси-сервер применяет те же правила безопасности, что и терминальный сканер, но с расширением на HTTP-специфику:
1.  **Headers**: Маскируются `Authorization`, `Cookie`, `Set-Cookie`.
2.  **Request Body**: Если `Content-Type: application/json`, парсер маскирует ключи `password`, `token`, `secret`, `key`.
3.  **Response Body**: Тело ошибки также сканируется на наличие JWT и секретов перед сохранением.

## MCP Integration
Инструмент `get_http_errors` возвращает список неудачных запросов. ИИ может использовать эти данные, чтобы понять, например, что локальный тест упал из-за 401 ошибки от сервиса аутентификации.

```json
{
  "timestamp": "2026-04-19T12:00:00Z",
  "method": "POST",
  "url": "https://api.stripe.com/v1/charges",
  "status": 400,
  "error_body": "{\"error\": {\"message\": \"Missing required param: amount\"}}"
}
```
