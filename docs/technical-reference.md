# Sideglance technical reference

The MCP manages `~/.local/share/sideglance/status.json`, shared with the desktop app on all platforms. It works before the app is opened. Settings remain in `~/.config/sideglance/settings.json`. The settings page controls appearance, always-on-top behavior, project order, and agent integration. Old source-path, window, and Markdown editor preferences are ignored. Existing Markdown files remain untouched and are no longer loaded.

## Status schema and MCP

The schema is defined in [status.schema.json](../status.schema.json). Documents with version `1` remain readable. Successful writes migrate them to version `2`. Each entry stores `folder_name`, nullable `session_id`, `branch`, `points`, and a server-generated RFC 3339 `updated_at` timestamp. Points contain plain `text`, `done`, an optional HTTP or HTTPS `link`, a stable server-assigned `id`, and a `submitted_at` timestamp. An existing point ID keeps its identity through edits, and its submitted time is preserved when the caller omits a replacement timestamp. Agent payloads contain text, completion state, link and optional ID. Storage timestamps and pin state are server-owned and excluded from the agent protocol. Legacy points receive deterministic IDs on read, so refreshes do not remount them.

Both MCP tools require `folder_name` and a non-empty real `session_id`. `read_status` returns only the matching project's session record. `write_status` also accepts `branch` and the complete revised `points` list, replacing that record. Agents read first, then revise text, completion flags, and links. Omitted points are removed; superseded text is not retained as history. Anonymous writes and project-wide MCP reads are not supported. Existing records with a null session ID remain viewable for compatibility.

The supplied session ID identifies the record; it is not authentication of the calling harness. Writers hold an OS lock on a stable sidecar file for the entire read/modify/write operation, then atomically replace the JSON. Invalid JSON is reported and preserved. Documents are limited to 1 MiB.

The viewer renders points directly with Svelte and keeps the last valid view if a read fails. It groups by project or repository and branch, and disambiguates concurrent sessions. Health notices identify waiting, stale, and failed reads; text selection pauses layout until cleared or explicitly resumed. Collapsed groups persist locally. Dismissal sends the point ID plus the entry snapshot timestamp; the old point index remains accepted for compatibility. Markdown rendering, source-file selection, and whole-document editing have been removed.

External edits to settings are read at startup. Restart the app after changing `settings.json` outside the app.

After upgrading the app, update installed copies of the skill through Settings and restart agent clients to load the replaced executable. The shared v2 file omits pin fields and pinned points, preserving compatibility with the pre-pin v2 parser. User pins live in `status.private.json` beside `status.json`, with complete point records so an older writer cannot erase them. The new viewer overlays those records. Older viewers cannot display these private pins. The first read of the inline-pin format backs up its original bytes and converts it to the compatible shared format.

Pin changes use the stable status lock and a recovery journal in the private file. On recovery, a pending write completes only if the raw file still matches its original bytes, or finalizes if the intended bytes already landed. A newer legacy write is preserved, and recovery conservatively retains both previously committed pins and newly pending pins so a crash after the raw commit cannot discard the only copy. Interrupted unpins may remain pinned until retried. Keep both status files together when backing up or recovering; do not delete the private file or copy an old raw backup over active newer data. Lock acquisition fails with an actionable retry error after two seconds.

Automatic deletion archives each removed point before changing status. Recently removed retains up to 30 days and 5 MiB, evicting oldest records first; it is bounded history, not a permanent backup. Archive failures prevent pruning. Restoring an item pins it so the same deletion rule cannot remove it immediately. Conflicting active points are preserved and reported. Retention previews are estimates because incoming reports can change the affected count.

Invalid settings are preserved in a unique `settings.invalid-*.backup` file, then validated defaults are loaded with a visible recovery warning. Read/permission errors are reported rather than overwritten. Skill replacements retain hidden `.sideglance.backup-*` directories beside the installation; if replacement is interrupted, quit agents and restore the backup directory to its original name. Config edits preserve permissions and detect changed source bytes immediately before replacement. External editors do not participate in Sideglance locking, so the final check and rename cannot provide an atomic compare-and-swap against arbitrary writers.

Agent integration distinguishes configured, structurally valid and explicitly tested connections. Test connection performs only MCP initialize and tools/list, with a five-second deadline and a 64 KiB output cap. It creates no status report and terminates its test process. Repair shows the existing target path and replaces Sideglance's registration. A successful fresh connection does not prove an existing agent session has reloaded. The previous Markdown `status` argument is unsupported.

## Updates and releases

The toolbar starts with a check icon and checks when clicked. Background checks run every three hours with jitter. An available update changes the icon to download; downloading retains the package until the user selects install, which installs and restarts the app. Download and install actions remain visible when the window loses focus. Settings also checks when the user selects **Check for updates**. The Tauri updater fetches the public `latest.json` asset from the latest stable GitHub release, verifies the update with the public key embedded in `src-tauri/tauri.conf.json`, then installs it after the user selects **Download and install**. The private counterpart is never committed. It must be stored in GitHub Actions secrets as `TAURI_SIGNING_PRIVATE_KEY`, with `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` when the key has one.

`.github/workflows/checks.yml` verifies frontend, browser, formatting, and native Rust tests on pull requests and main pushes. The Rust matrix covers macOS, Linux, and Windows. `.github/workflows/release.yml` starts when a GitHub release is created, verifies the tagged source through that same workflow, then builds signed macOS arm64, Linux x64, and Windows x64 bundles in parallel. A final job uploads one platform manifest after all three builds pass. Update checks may temporarily fail while a new release builds, because `latest.json` is not available until that final job completes. Linux self-update supports AppImage only. Debian and RPM users should update with their package manager.

The bundled MCP executable is replaced with the app. A running agent client can keep using the old process until it restarts, so users should restart their client after updating. The updater does not replace copied skills or alter agent configuration. Available updater resources are closed when settings closes, including checks that resolve after unmount; an in-flight installation retains its resource until completion.

## Development

Source is organized by responsibility. The frontend keeps app entry points at `src/`, with viewer code in `src/viewer/`, settings in `src/settings/`, integrations in `src/integrations/`, and shared types and Tauri helpers in `src/shared/`. Rust puts status storage and refresh code in `src-tauri/src/status/`, preferences in `preferences/`, desktop commands and windows in `desktop/`, and integration adapters in `integrations/`.

```sh
npm ci
npx playwright install chromium
npm run dev
# Run verification separately from the development server:
npm run verify
cargo fmt --manifest-path src-tauri/Cargo.toml --check
cargo clippy --manifest-path src-tauri/Cargo.toml --locked --all-targets -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --locked
```

The local checks need Node 24.11.1 or newer, npm, Rust, and the Tauri platform prerequisites. CI uses Rust 1.99.0. Install the Playwright browser once with `npx playwright install chromium` before running `npm run test:motion`. The motion test uses the fixture under `tests/` and does not access real app data.

To build a desktop bundle, install Rust and the Tauri prerequisites, then run `npm run tauri build`.

For frame-by-frame viewer motion regression checks, run `npm run dev` and open
`http://localhost:1420/tests/viewer-motion.html`. The tab title reports pass/fail;
`await window.motionResults` in the browser console returns each result.
The fixture mocks Tauri IPC and does not change your saved statuses.

## Resource usage and validation limits

The headless MCP uses a current-thread runtime and blocking workers for storage. A three-client macOS Apple M4 sample reduced thread counts from 13 per process to 3–4 while retaining similar write latency. Frontend settings and Markdown rendering load as a separate route chunk. Decorative SVG playback pauses on blur, document hiding and reduced-motion preference; merely hiding the animation element with CSS is insufficient. Background throttling remains disabled until native event-delivery checks justify changing it.

Release builds strip symbols. A local macOS executable experiment reduced 18.85 MB to 13.43 MB and passed the temporary-home MCP workload. These are executable sizes, not installer or whole-app measurements. Local target caches and tracked generated schemas are separate from distributed bundle size. LTO and codegen-unit changes have not been adopted without further measurements.

PR checks cover frontend unit/browser/type/format checks, a high-severity npm audit gate, and Rust format/Clippy/tests across macOS, Linux, and Windows. The current npm audit retains two low findings through the latest Svelte build plugin's deepmerge dependency, which handles trusted local compiler configuration. There is no upstream fix reported by npm; recheck the exception on updates. RustSec tooling was unavailable during local remediation. Platform CI and native visibility/minimize/event-delivery behavior require their own verification; Chromium fixtures establish frontend behavior, not native WebKit behavior.
