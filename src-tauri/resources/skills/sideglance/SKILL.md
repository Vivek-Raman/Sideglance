---
name: sideglance
description: MUST ALWAYS invoke this skill for every task to keep Sideglance's status window current with the active project, branch, and chat activity.
---

# Sideglance

Update quietly at task start and when activity or outcome materially changes. Use the `sideglance` MCP; never access its backing file directly.

Use the current folder name and actual harness session/thread ID for both tools. For Codex, use `CODEX_THREAD_ID` when supplied. Never invent an ID. If none is exposed, skip updates. Children sharing a thread ID append their actual harness agent ID as `thread-id/agent-id`.

Read with `read_status` at session start or after losing the saved points from context. Otherwise reuse the points returned by the last successful write.

Write with `write_status`:
- `folder_name` and `session_id`: the same project/session identity.
- `branch`: refresh from Git before writing; use `detached HEAD` when detached.
- `repository`: supply Git origin as `owner/repo` on the first write, or when it changes. Omit if unavailable; later writes preserve it.
- `points`: the complete revised list. Each point has `text`, optional `done` and HTTP(S) `link`, and an `id` for existing points. Preserve returned IDs; omit IDs for new points. Writes return saved points with assigned IDs.

Revise points rather than accumulate a message history. Keep useful independent work, remove obsolete points, and mark completion with `done: true`. Omitted points are removed. Skip unchanged writes.

Keep points brief and plain text, without Markdown or commit hashes. Usually one point is enough. For PR work, attach the canonical PR URL as `link`. Mention blockers only when critical and unresolved.
