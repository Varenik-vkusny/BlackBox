# Documentation Strategy & Knowledge Base

This project uses the `docs/` folder as its authoritative Knowledge Base (KB).

## 1. Structure
- All documentation files in `docs/` must start with YAML frontmatter metadata.
- Metadata requirements:
    - `title`: Short descriptive title.
    - `synopsis`: 1-2 sentence summary of content.
    - `agent_guidance`: Specific scenarios when an AI agent should read this file.
    - `related`: Internal links to related documents.

## 2. Agent Consumption
- Agents should NOT read all documents at once.
- Agents should use `list_dir` on `docs/` and then peek at the metadata/headers to decide which file is relevant to their current task.

## 3. Synchronization
- When modifying core system logic (Daemon, MCP, UI), the agent MUST check if corresponding documentation in `docs/` needs updating.
- Documentation must always reflect the current state of Phase 3 implementation.
