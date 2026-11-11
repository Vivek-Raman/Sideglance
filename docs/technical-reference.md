# Sideglance technical reference

The MCP manages `~/.local/share/sideglance/status.json`, shared with the desktop app on all platforms. It works before the app is opened. Settings remain in `~/.config/sideglance/settings.json`. The settings page controls appearance, always-on-top behavior, project order, and agent integration. Old source-path, window, and Markdown editor preferences are ignored. Existing Markdown files remain untouched and are no longer loaded.

## Status schema and MCP

The schema is defined in [status.schema.json](../status.schema.json). Documents with version `1` remain readable. Successful writes migrate them to version `2`. Each entry stores `folder_name`, nullable `session_id`, `branch`, `points`, and a server-generated RFC 3339 `updated_at` timestamp. Points contain plain `text`, `done`, an optional HTTP or HTTPS `link`, a stable server-assigned `id`, and a `submitted_at` timestamp. An existing point ID keeps its identity through edits, and its submitted time is preserved when the caller omits a replacement timestamp. A caller-supplied valid timestamp is retained. Legacy points receive deterministic IDs on read, so refreshes do not remount them.

Both MCP tools require `folder_name` and a non-empty real `session_id`. `read_status` returns only the matching project's session record. `write_status` also accepts `branch` and the complete revised `points` list, replacing that record. Agents read first, then revise text, completion flags, and links. Omitted points are removed; superseded text is not retained as history. Anonymous writes and project-wide MCP reads are not supported. Existing records with a null session ID remain viewable for compatibility.

The supplied session ID identifies the record; it is not authentication of the calling harness. Writers hold an OS lock on a stable sidecar file for the entire read/modify/write operation, then atomically replace the JSON. Invalid JSON is reported and preserved. Documents are limited to 1 MiB.

The viewer renders points directly with Svelte and keeps the last valid view if a read fails. It groups by project and branch, with no session labels. Dismissal sends the point ID plus the entry snapshot timestamp; the old point index remains accepted for compatibility. Markdown rendering, source-file selection, and whole-document editing have been removed.

External edits to settings are read at startup. Restart the app after changing `settings.json` outside the app.

After upgrading the app, update installed copies of the skill through Settings. Older app and MCP processes cannot read version 2 documents, so upgrade and restart them before the first write. The previous Markdown `status` argument is no longer supported. This project does not migrate a user's status file until a successful write occurs.

## Updates and releases

Settings checks for updates only when the user selects **Check for updates**. The Tauri updater fetches the public `latest.json` asset from the latest stable GitHub release, verifies the update with the public key embedded in `src-tauri/tauri.conf.json`, then installs it after the user selects **Download and install**. The private counterpart is never committed. It must be stored in GitHub Actions secrets as `TAURI_SIGNING_PRIVATE_KEY`, with `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` when the key has one.

`.github/workflows/release.yml` runs manually against a version tag. It creates a draft release, builds signed macOS arm64, Linux x64, and Windows x64 bundles, uploads the signature files, writes a platform manifest, then publishes the release. It fails without all required signatures. Linux self-update supports AppImage only. Debian and RPM users should update with their package manager.

The bundled MCP executable is replaced with the app. A running agent client can keep using the old process until it restarts, so users should restart their client after updating. The updater does not replace copied skills or alter agent configuration.

## Development

Source is organized by responsibility. The frontend keeps app entry points at `src/`, with viewer code in `src/viewer/`, settings in `src/settings/`, integrations in `src/integrations/`, and shared types and Tauri helpers in `src/shared/`. Rust puts status storage and refresh code in `src-tauri/src/status/`, preferences in `preferences/`, desktop commands and windows in `desktop/`, and integration adapters in `integrations/`.

```sh
npm install
npm run dev
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
npm test
npm run format:check
```

The local checks need Node 22 or newer for `--experimental-strip-types`, npm, Rust, and the Tauri platform prerequisites. Install the Playwright browser once with `npx playwright install chromium` before running `npm run test:motion`. The motion test uses the fixture under `tests/` and does not access real app data.

To build a desktop bundle, install Rust and the Tauri prerequisites, then run `npm run tauri build`.

For frame-by-frame viewer motion regression checks, run `npm run dev` and open
`http://localhost:1420/tests/viewer-motion.html`. The tab title reports pass/fail;
`await window.motionResults` in the browser console returns each result.
The fixture mocks Tauri IPC and does not change your saved statuses.
