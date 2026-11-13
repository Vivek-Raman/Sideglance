<script lang="ts">
  import {
    collapsedRowKeys,
    countExpiryKeys,
    pointKey,
    projectNames,
    relativeTime,
    viewerRows,
  } from './rows';
  import { onMount, tick } from 'svelte';
  import { fly } from 'svelte/transition';
  import { isUndoShortcut } from './undo-shortcut';
  import { command } from '../shared/commands';
  import { applyPrimaryColor, applyTheme, systemTheme } from '../shared/theme';
  import { ViewerController, browserTimers, expirySoon } from './controller';
  import ViewerArtwork from './ViewerArtwork.svelte';
  import ViewerToolbar from './ViewerToolbar.svelte';
  import StatusPoint from './StatusPoint.svelte';
  import { RowMotion } from './row-motion';
  import { startEventReadLifecycle } from '../shared/event-lifecycle';
  import { listenTauri } from '../shared/tauri-events';
  import { subscribeSettings } from '../settings/subscription';
  import RecentlyRemoved from './RecentlyRemoved.svelte';
  import { loadCollapsed, saveCollapsed } from './collapse-state';
  import type { Snapshot } from '../shared/types';

  const timers = browserTimers();
  const motion = new RowMotion(tick, (ms) => (timers.reducedMotion() ? 0 : ms));
  const controller = new ViewerController(timers, motion);
  const textScales = [0.85, 1, 1.15, 1.35];
  const themes = ['system', 'light', 'dark'] as const;
  let collapsed = loadCollapsed();
  let hoveredHeadings = new Set<string>();
  function hoverRow(key: string) {
    const headings = new Set<string>();
    for (let index = rows.findIndex((row) => row.key === key); index >= 0; index--) {
      const row = rows[index];
      if (row.kind === 'branch' && !headings.size) headings.add(row.key);
      if (row.kind === 'project') {
        headings.add(row.key);
        break;
      }
    }
    hoveredHeadings = headings;
  }
  function toggleCollapsed(key: string) {
    const next = new Set(collapsed);
    if (next.has(key)) next.delete(key);
    else next.add(key);
    collapsed = next;
    saveCollapsed(collapsed);
  }
  $: hiddenRows = collapsedRowKeys(rows, collapsed);
  let windowFocused = true;
  let now = controller.state.now;
  let entries = controller.state.entries;
  let settings = controller.state.settings;
  let pendingRemoval = controller.state.pendingRemoval;
  let dismissing = controller.state.dismissing;
  let undoCount = controller.state.undoCount;
  let undoing = controller.state.undoing;
  let statusError = controller.state.statusError;
  let health = controller.state.health;
  let lastSuccessfulReadAt = controller.state.lastSuccessfulReadAt;
  let recoveryWarning = controller.state.recoveryWarning;
  let selectedText = false;
  let revision = -1;
  let snapshotPending = false;
  function resumeUpdates() {
    document.getSelection()?.removeAllRanges();
    controller.setSelection(false);
  }
  async function refreshSnapshot() {
    controller.acceptSnapshot(await command('get_snapshot'));
  }

  let appearanceKey = '';
  let undoNotice = false;
  let noticeVersion = 0;
  let noticeTimer: ReturnType<typeof setTimeout> | undefined;
  function showUndoNotice() {
    if (noticeTimer !== undefined) window.clearTimeout(noticeTimer);
    undoNotice = true;
    noticeVersion += 1;
    noticeTimer = window.setTimeout(() => {
      undoNotice = false;
      noticeTimer = undefined;
    }, 5000);
  }
  async function undoDismissal() {
    await controller.undo(
      (args) => command('restore_point', args),
      () => command('get_snapshot'),
    );
    if (!controller.state.undoCount) undoNotice = false;
  }
  $: projects = projectNames(
    entries,
    settings?.projectSortOrder ?? 'project-directory-asc',
    settings?.statusGrouping,
  );
  $: sessionLabels = new Map(
    entries
      .filter(
        (entry) =>
          entry.session_id &&
          entries.some(
            (other) =>
              other !== entry &&
              other.folder_name === entry.folder_name &&
              other.branch === entry.branch &&
              other.session_id !== entry.session_id,
          ),
      )
      .map((entry, index) => [entry.session_id!, `Session ${index + 1}`]),
  );
  $: rows = viewerRows(entries, projects, settings?.statusSortOrder, settings?.statusGrouping);
  $: if (revision >= 0 && health === 'current' && !snapshotPending) {
    const valid = new Set(rows.filter((row) => row.kind !== 'point').map((row) => row.key));
    const next = new Set([...collapsed].filter((key) => valid.has(key)));
    if (next.size !== collapsed.size) {
      collapsed = next;
      saveCollapsed(collapsed);
    }
  }
  $: countExpiring = countExpiryKeys(entries, settings?.maxStatusItemsPerProject ?? null);
  async function dismissPoint(entry: (typeof entries)[number], pointIndex: number) {
    const point = entry.points[pointIndex];
    await controller.dismiss(
      entry,
      point,
      pointIndex,
      pointKey(entry, point, pointIndex),
      async (args) => {
        await command('dismiss_point', args);
        showUndoNotice();
      },
      () => command('get_snapshot'),
    );
  }
  async function togglePointPin(entry: (typeof entries)[number], pointIndex: number) {
    const point = entry.points[pointIndex];
    if (!point.id) return;
    try {
      await command('set_point_pinned', {
        folderName: entry.folder_name,
        sessionId: entry.session_id,
        branch: entry.branch,
        updatedAt: entry.updated_at,
        pointId: point.id,
        pinned: !point.pinned,
      });
      controller.acceptSnapshot(await command('get_snapshot'));
    } catch (error) {
      controller.setErrorForView(error);
    }
  }
  async function cycleTheme() {
    if (!settings) return;
    const index = themes.indexOf(settings.theme);
    try {
      await command('set_appearance_settings', {
        theme: themes[(index + 1) % themes.length],
        primaryColor: settings.primaryColor,
        textScale: settings.textScale,
      });
    } catch (error) {
      controller.setErrorForView(error);
    }
  }
  async function changeTextScale(direction: -1 | 1) {
    if (!settings) return;
    const index = textScales.indexOf(settings.textScale);
    const next = Math.max(0, Math.min(textScales.length - 1, index + direction));
    if (next !== index) {
      try {
        await command('set_appearance_settings', {
          theme: settings.theme,
          primaryColor: settings.primaryColor,
          textScale: textScales[next],
        });
      } catch (error) {
        controller.setErrorForView(error);
      }
    }
  }
  async function openSettings() {
    try {
      await command('open_settings');
    } catch (error) {
      controller.setErrorForView(error);
    }
  }
  onMount(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (!isUndoShortcut(event) || !controller.state.undoCount) return;
      event.preventDefault();
      void undoDismissal();
    };
    window.addEventListener('keydown', onKeyDown);
    const onFocus = () => {
      windowFocused = true;
    };
    const onBlur = () => {
      windowFocused = false;
    };
    const onThemeChange = () => {
      if (controller.state.settings?.theme === 'system') applyTheme('system');
    };
    const onSelection = () => controller.setSelection(Boolean(document.getSelection()?.toString()));
    windowFocused = document.hasFocus();
    window.addEventListener('focus', onFocus);
    window.addEventListener('blur', onBlur);
    systemTheme.addEventListener('change', onThemeChange);
    document.addEventListener('selectionchange', onSelection);
    let tornDown = false;
    const unsubscribeController = controller.subscribe((next) => {
      ({ health, lastSuccessfulReadAt, recoveryWarning, selectedText, revision } = next);
      snapshotPending = next.pendingSnapshot !== null;
      ({ entries, settings, pendingRemoval, dismissing, undoCount, undoing, statusError, now } =
        next);
    });
    const sourceLifecycle = startEventReadLifecycle<Snapshot>({
      listen: (onValue) => listenTauri('source-update', onValue),
      read: () => command('get_snapshot'),
      onValue: (snapshot) => {
        if (!tornDown) controller.acceptSnapshot(snapshot);
      },
      onError: (error) => {
        if (!tornDown) controller.setErrorForView(error);
      },
    });
    const settingsLifecycle = subscribeSettings(
      (value) => {
        if (!tornDown) controller.acceptSettings(value);
      },
      (error) => {
        if (!tornDown) controller.setErrorForView(error);
      },
      () => command('get_settings'),
    );
    const refreshNow = window.setInterval(() => controller.refreshNow(), 60_000);
    const applySettings = controller.subscribe((next) => {
      if (next.settings) {
        const nextKey = `${next.settings.theme}|${next.settings.primaryColor}|${next.settings.textScale}`;
        if (nextKey !== appearanceKey) {
          appearanceKey = nextKey;
          applyTheme(next.settings.theme);
          applyPrimaryColor(next.settings.primaryColor);
          document.documentElement.style.setProperty(
            '--text-scale',
            String(next.settings.textScale),
          );
        }
      }
    });
    return () => {
      tornDown = true;
      window.removeEventListener('keydown', onKeyDown);
      if (noticeTimer !== undefined) window.clearTimeout(noticeTimer);
      window.clearInterval(refreshNow);
      controller.dispose();
      unsubscribeController();
      applySettings();
      sourceLifecycle.dispose();
      settingsLifecycle.dispose();
      window.removeEventListener('focus', onFocus);
      window.removeEventListener('blur', onBlur);
      systemTheme.removeEventListener('change', onThemeChange);
      document.removeEventListener('selectionchange', onSelection);
    };
  });
</script>

<main>
  <ViewerArtwork />
  <div class="app-chrome" data-tauri-drag-region aria-hidden="true"></div>
  <ViewerToolbar
    {settings}
    {windowFocused}
    onTheme={cycleTheme}
    onTextScale={changeTextScale}
    onSettings={openSettings}
  />
  {#if undoNotice}
    <div
      class="undo-notice"
      role="status"
      transition:fly={{ y: 12, duration: timers.reducedMotion() ? 0 : 160 }}
    >
      <span>Line dismissed</span>
      <button
        onclick={() => void undoDismissal()}
        disabled={undoing || !undoCount}
        title="Undo dismissal (⌘Z / Ctrl+Z)">Undo</button
      >
      {#key noticeVersion}
        <span class="undo-countdown" aria-hidden="true"></span>
      {/key}
    </div>
  {/if}
  <section
    class="document"
    aria-label="Project status"
    onpointerleave={() => (hoveredHeadings = new Set())}
    onselectstart={(event) => {
      const target =
        event.target instanceof Element ? event.target : (event.target as Node).parentElement;
      if (!target?.closest('button, a, input, select, textarea, summary'))
        controller.setSelection(true);
    }}
  >
    {#if health !== 'current'}
      <div class="health-notice" role="status">
        <strong
          >{health === 'waiting'
            ? 'Waiting for updates'
            : health === 'stale'
              ? 'Updates are stale'
              : 'Updates need attention'}</strong
        >
        {#if entries.length}<p>Showing the last available status.</p>{/if}
        {#if lastSuccessfulReadAt}<p>
            Last read {relativeTime(lastSuccessfulReadAt, now)} ago.
          </p>{/if}
      </div>
    {/if}
    {#if recoveryWarning}<p class="health-notice" role="alert">{recoveryWarning}</p>{/if}
    {#if selectedText}
      <div class="health-notice selection-notice" role="status">
        Updates are paused while text is selected.
        <button onclick={resumeUpdates}>Resume updates</button>
      </div>
    {/if}
    {#if !rows.length}
      <div class="empty-state">
        <p>
          {health === 'current' ? 'No status items yet.' : 'Your agent updates will appear here.'}
        </p>
        <button onclick={() => void openSettings()}>Set up agent integration</button>
      </div>
    {/if}
    {#each rows as row (row.key)}
      <div
        class="motion-row"
        class:collapsed-row={hiddenRows.has(row.key)}
        inert={hiddenRows.has(row.key)}
        aria-hidden={hiddenRows.has(row.key) ? true : undefined}
        onpointerenter={() => hoverRow(row.key)}
        data-row-key={row.key}
        use:motion.row={row.key}
      >
        <div class="row-content">
          {#if row.kind !== 'point'}
            <div
              class="group-heading"
              class:project-heading={row.kind === 'project'}
              class:is-hovered={hoveredHeadings.has(row.key)}
            >
              {#if row.kind === 'project'}
                <h2>{row.folder}</h2>
              {:else}
                <p>{row.branch}</p>
              {/if}
              <button
                class="collapse-toggle"
                class:is-collapsed={collapsed.has(row.key)}
                aria-expanded={!collapsed.has(row.key)}
                aria-label={`${collapsed.has(row.key) ? 'Expand' : 'Collapse'} ${row.kind === 'project' ? row.folder : row.branch}`}
                title={collapsed.has(row.key) ? 'Expand group' : 'Collapse group'}
                onclick={() => toggleCollapsed(row.key)}
              >
                <svg
                  width="12"
                  height="12"
                  viewBox="0 0 16 16"
                  fill="none"
                  stroke="currentColor"
                  stroke-width="1.5"
                  aria-hidden="true"
                >
                  <path d="m6 3 5 5-5 5" />
                </svg>
              </button>
            </div>
          {:else}
            {@const { entry, point, pointIndex } = row}
            <StatusPoint
              {entry}
              {point}
              {now}
              sessionLabel={sessionLabels.get(entry.session_id ?? '') ?? null}
              last={row.last}
              removing={pendingRemoval.has(row.key)}
              dismissing={dismissing.has(row.key)}
              expiring={!point.pinned &&
                (countExpiring.has(row.key) ||
                  expirySoon(
                    point.submitted_at ?? entry.updated_at,
                    settings?.autoClearAfterMinutes ?? null,
                    now,
                  ))}
              onPin={() => togglePointPin(entry, pointIndex)}
              onDismiss={() => dismissPoint(entry, pointIndex)}
            />
          {/if}
        </div>
      </div>
    {/each}
    <RecentlyRemoved onRestored={refreshSnapshot} />
    {#if statusError}<p class="status-error" role="alert">{statusError}</p>{/if}
  </section>
</main>

<style>
  :global(:root) {
    --ease-gentle-out: cubic-bezier(0.22, 1, 0.36, 1);
  }
  .document {
    --bottom-text-fade: clamp(64px, 20vh, 124px);
    overflow-anchor: none;
    padding: 17px 19px;
    padding-bottom: calc(var(--bottom-text-fade) + 24px);
    /* Fade the scrolling content so the background artwork stays crisp beneath it. */
    -webkit-mask-image: linear-gradient(
      to bottom,
      black calc(100% - var(--bottom-text-fade)),
      rgb(0 0 0 / 75%) calc(100% - var(--bottom-text-fade) * 0.55),
      rgb(0 0 0 / 25%) calc(100% - var(--bottom-text-fade) * 0.2),
      transparent 100%
    );
    mask-image: linear-gradient(
      to bottom,
      black calc(100% - var(--bottom-text-fade)),
      rgb(0 0 0 / 75%) calc(100% - var(--bottom-text-fade) * 0.55),
      rgb(0 0 0 / 25%) calc(100% - var(--bottom-text-fade) * 0.2),
      transparent 100%
    );
  }
  .motion-row {
    display: grid;
    grid-template-rows: 1fr;
    opacity: 1;
    transition:
      grid-template-rows 220ms var(--ease-gentle-out),
      opacity 160ms ease;
  }
  .motion-row.collapsed-row {
    grid-template-rows: 0fr;
    opacity: 0;
    pointer-events: none;
  }
  .row-content {
    min-height: 0;
    overflow: clip;
    display: flow-root;
  }
  .group-heading {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    margin-bottom: 0.7em;
  }
  .group-heading h2,
  .group-heading p {
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .collapse-toggle {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    padding: 3px;
    color: var(--muted);
    opacity: 0;
    transition:
      opacity 120ms ease,
      background 120ms ease;
  }
  .collapse-toggle svg {
    transform: rotate(90deg);
    transition: transform 220ms var(--ease-gentle-out);
  }
  .collapse-toggle.is-collapsed svg {
    transform: rotate(0);
  }
  .group-heading.is-hovered .collapse-toggle,
  .group-heading:hover .collapse-toggle,
  .group-heading:focus-within .collapse-toggle,
  .collapse-toggle.is-collapsed {
    opacity: 1;
  }
  .collapse-toggle:hover {
    background: color-mix(in srgb, var(--text) 6%, transparent);
    color: var(--text);
  }
  .collapse-toggle:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
  @media (hover: none) {
    .collapse-toggle {
      opacity: 1;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .motion-row,
    .collapse-toggle,
    .collapse-toggle svg {
      transition: none;
    }
  }
  .group-heading.project-heading {
    margin-bottom: 0.2em;
  }
  .motion-row + .motion-row .project-heading {
    margin-top: 0.8em;
  }
  .group-heading h2,
  .group-heading p {
    margin: 0;
  }
  .motion-row p {
    color: var(--muted);
  }
  button {
    flex: none;
    border: 1px solid transparent;
    border-radius: 0.4rem;
    padding: 0.5rem 0.7rem;
    color: var(--text);
    background: transparent;
    font: inherit;
    font-size: 0.8rem;
    font-weight: 600;
    cursor: pointer;
  }
  button:disabled {
    cursor: default;
    opacity: 0.65;
  }
  .undo-notice {
    position: fixed;
    z-index: 3;
    bottom: 20px;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 1rem;
    overflow: hidden;
    padding: 0.35rem 0.6rem 0.35rem 1rem;
    border: 1px solid var(--border);
    border-radius: 0.65rem;
    background: var(--bg);
    color: var(--text);
    box-shadow: 0 4px 20px rgb(0 0 0 / 15%);
    font-size: 0.8rem;
    white-space: nowrap;
  }
  .undo-notice button {
    color: var(--accent);
  }
  .undo-notice button:focus-visible {
    outline: 2px solid var(--accent);
  }
  .undo-countdown {
    position: absolute;
    bottom: 0;
    left: 0;
    width: 100%;
    height: 2px;
    background: var(--accent);
    transform-origin: left;
    animation: undo-countdown 5s linear forwards;
  }
  @keyframes undo-countdown {
    to {
      transform: scaleX(0);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .undo-countdown {
      animation: none;
    }
  }
  .selection-notice {
    position: fixed;
    bottom: 12px;
    left: 12px;
    right: 12px;
    z-index: 4;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 6px;
  }
  .health-notice,
  .empty-state {
    margin-bottom: 0.8rem;
    padding: 0.65rem;
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    font-size: 0.85rem;
    overflow-wrap: anywhere;
  }
  .health-notice p {
    margin-top: 0.3rem;
  }
  .health-notice button,
  .empty-state button {
    color: var(--accent);
  }
  button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .status-error {
    color: #c43d35;
    font-size: 0.8rem;
  }
</style>
