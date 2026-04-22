# Antigravity Agentic Protocol

This rule defines the core behavior of the Antigravity agent when working on the BlackBox project.

## 1. Skill-First Paradigm
- Before implementing any significant feature, refactoring, or complex bug fix, the agent MUST invoke a relevant Skill tool.
- Recommended starting skills: `brainstorming`, `writing-plans`, `subagent-driven-development`.
- Logic: Ensures a high-level strategic approach rather than reactive code editing.

## 2. Situational Awareness (BlackBox Tools)
- Before proposing any diagnostic solution, the agent MUST use BlackBox MCP tools to understand the current runtime environment.
- Mandatory check: `get_snapshot` at the start of a session.
- Investigative tools: `get_terminal_buffer`, `get_compressed_errors`, `get_container_logs`.

## 3. Verification Mandate
- No task is considered "Done" until it has been verified on the user's system.
- Use the `verification-before-completion` skill to run tests, start services, or check UI rendering.
- Agent must provide terminal/browser output evidence of success.

## 4. Zero-Placeholder Policy
- For UI components or demos, use the `generate_image` tool to create actual assets.
- Never write `// TODO: image here` or similar placeholders.

## 5. Coding Integrity
- Adhere strictly to the `user_global` rules provided in the system prompt.
- Prioritize Immutability, Small Files, and Comprehensive Error Handling.
