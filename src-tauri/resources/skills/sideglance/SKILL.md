---
name: sideglance
description: MUST ALWAYS invoke this skill for every task to keep Sideglance's status window current with the active project, branch, and chat activity.
---

# Sideglance

Update quietly at task start and when activity or outcome materially changes. Use the `sideglance` MCP; never access its backing file directly.

Use your absolute working directory and actual harness session/thread ID for both tools. The server resolves the Git checkout root, current branch, and origin repository locally using Git. Do not discover or supply these fields yourself. For Codex, use `CODEX_THREAD_ID` when supplied. Never invent an ID. If none is exposed, skip updates. Children sharing a thread ID append their actual harness agent ID as `thread-id/agent-id`.

Read with `read_status` at session start or after losing the saved points from context. Otherwise reuse the points returned by the last successful write.

Write with `write_status`:
- `working_directory`: your absolute working directory in the project. Subdirectories resolve to the checkout root; outside Git, the directory name is used with no repository and an empty branch.
- `session_id`: the same actual harness session/thread ID used for reads.
- `points`: the complete revised list. Each point has `text`, optional `done` and HTTP(S) `link`, and an `id` for existing points. Preserve returned IDs; omit IDs for new points. Writes return saved points with assigned IDs.

Revise points rather than accumulate a message history. Keep useful independent work, remove obsolete points, and mark completion with `done: true`. Omitted points are removed. Skip unchanged writes.

Keep points brief and plain text, without Markdown or commit hashes. Usually one point is enough. For PR work, attach the canonical PR URL as `link`. Mention blockers only when critical and unresolved.
