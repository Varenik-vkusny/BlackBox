---
title: 06 IDE Integration (VS Code Bridge)
synopsis: Methods for capturing terminal data: VS Code bridge vs. Native PTY capture (blackbox-run) and Shell Hooks.
agent_guidance: Relevant when diagnosing why logs aren't appearing or when choosing between blackbox-run and general terminal capture.
related: [01_ARCHITECTURE.md, 10_EXTERNAL_LOGS.md]
---

# 06. IDE Integration (VS Code Bridge)

> [!IMPORTANT]
> **Repomix Context Command:**
> `repomix --include "vscode-extension/src/**" --output vscode_context.txt`

Хотя VS Code является удобным источником данных, BlackBox в Фазе 3 поддерживает полностью автономный захват данных на системном уровне.

## 1. VS Code Bridge (Legacy-Hybrid)
Плагин BlackBox для VS Code минималистичен и спроектирован для нулевого влияния на производительность редактора.

*   **API**: Использует (предложенное) API `window.onDidWriteTerminalData`. Это позволяет получать именно те байты, которые выводятся в терминал VS Code.
*   **Bridge Protocol**: Связь через **TCP сокет (localhost:8765)**.

## 2. Native Aggregation (Phase 3)
Для работы без плагинов BlackBox использует нативные механизмы захвата.

*   **PTY Capture (`portable-pty`)**: Демон может напрямую запускать процессы и захватывать их вывод через управляющий терминал (ConPTY на Windows). Это используется в утилите `blackbox-run`.
*   **Shell Hooks**: Настройка `.zshrc` или `.bashrc` для автоматической пересылки всех команд и их вывода в BlackBox через `curl` или `nc` на порт 8765.

## Security & Privacy
Причина выбора такого подхода:
*   **Isolation**: Мы не читаем открытые вкладки. Только то, что было явно выведено в терминал.
*   **Visibility**: Весь поток данных из IDE в BlackBox можно увидеть, подключившись к порту 8765.
