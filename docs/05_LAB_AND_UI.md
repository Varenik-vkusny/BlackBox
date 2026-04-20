# 05. Lab and UI

> [!IMPORTANT]
> **Repomix Context Command:**
> `repomix --include "blackbox-lab/src/**" --output ui_context.txt`

Для мониторинга работы BlackBox и проверки того, что видит ИИ, реализованы три интерфейса.

## 1. BlackBox Lab (Web Interface)
Веб-интерфейс на **React + Vite**. Предназначен для визуализации накопленного контекста.
*   **Grid Dashboard**: Состояние демона и статус Docker.
*   **LogExplorer**: Просмотр логов терминала и внешних файлов с фильтрацией.
*   **GitLens**: Визуализация незакоммиченных изменений и связанных с ними ошибок.
*   **InjectionStation**: Панель для ручной "инъекции" логов в демон для тестирования сценариев.

## Communication Architecture
1.  **Admin API (HTTP :8768)**: REST-сервер (Axum). Эндпоинты `/api/status`, `/api/terminal`, `/api/inject`. Поддерживает CORS.
2.  **Terminal Bridge (TCP :8765)**: Принимает входящие логи. Лаборатория использует `/api/inject` для проксирования логов в этот мост.
