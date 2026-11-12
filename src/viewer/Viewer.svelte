<script lang="ts">
  import { countExpiryKeys, pointKey, projectNames, relativeTime, viewerRows } from './rows';
  import { onMount, tick } from 'svelte';
  import { fly } from 'svelte/transition';
  import { isUndoShortcut } from './undo-shortcut';
  import { command } from '../shared/commands';
  import { applyPrimaryColor, applyTheme, systemTheme } from '../shared/theme';
  import { ViewerController, browserTimers } from './controller';
  import { RowMotion } from './row-motion';
  import { startEventReadLifecycle } from '../shared/event-lifecycle';
  import { listenTauri } from '../shared/tauri-events';
  import { subscribeSettings } from '../settings/subscription';
  import type { Snapshot } from '../shared/types';

  const timers = browserTimers();
  const motion = new RowMotion(tick, (ms) => (timers.reducedMotion() ? 0 : ms));
  const controller = new ViewerController(timers, motion);
  const textScales = [0.85, 1, 1.15, 1.35];
  const themes = ['system', 'light', 'dark'] as const;
  let windowFocused = true;
  let now = controller.state.now;
  let entries = controller.state.entries;
  let settings = controller.state.settings;
  let pendingRemoval = controller.state.pendingRemoval;
  let dismissing = controller.state.dismissing;
  let undoCount = controller.state.undoCount;
  let undoing = controller.state.undoing;
  let statusError = controller.state.statusError;
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
  $: rows = viewerRows(entries, projects, settings?.statusSortOrder, settings?.statusGrouping);
  $: countExpiring = countExpiryKeys(entries, settings?.maxStatusItemsPerProject ?? null);
  function safeLink(link: string | null) {
    if (!link) return null;
    try {
      const url = new URL(link);
      return ['https:', 'http:'].includes(url.protocol) ? url.href : null;
    } catch {
      return null;
    }
  }
  function linkLabel(link: string) {
    const url = new URL(link);
    const pullRequest =
      url.hostname === 'github.com' && url.pathname.match(/^\/[^/]+\/[^/]+\/pull\/(\d+)\/?$/);
    return pullRequest ? `PR #${pullRequest[1]}` : 'View link';
  }
  function expirySoon(
    submittedAt: string,
    autoClearAfterMinutes: number | null,
    currentNow: number,
  ) {
    return (
      autoClearAfterMinutes != null &&
      new Date(submittedAt).getTime() + autoClearAfterMinutes * 60_000 - currentNow <= 10 * 60_000
    );
  }
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
  <svg
    class="line-art"
    viewBox="0 0 1000 480"
    preserveAspectRatio="xMaxYMin meet"
    aria-hidden="true"
  >
    <g fill="none" stroke="currentColor" stroke-linecap="round" vector-effect="non-scaling-stroke">
      <path
        d="M70-40C190-8 230 124 360 120C520 116 610 10 742 50C880 91 910 168 1060 140"
        stroke-width="2.5"
        ><animate
          attributeName="opacity"
          values="0.55;0.9;0.55"
          keyTimes="0;0.42;1"
          begin="-4s"
          dur="18s"
          repeatCount="indefinite"
        /></path
      >
      <path
        class="line-art-medium"
        d="M190-40C290 19 290 171 420 168C555 165 650 84 780 123C910 165 938 246 1060 236"
        stroke-width="2.5"
        ><animate
          attributeName="opacity"
          values="0.75;0.46;0.75"
          keyTimes="0;0.64;1"
          begin="-11s"
          dur="22s"
          repeatCount="indefinite"
        /></path
      >
      <path
        class="line-art-medium"
        d="M310-40C385 42 380 226 500 220C630 214 706 159 826 201C930 238 968 315 1060 307"
        stroke-width="2.5"
        ><animate
          attributeName="opacity"
          values="0.38;0.76;0.38"
          keyTimes="0;0.31;1"
          begin="-7s"
          dur="20s"
          repeatCount="indefinite"
        /></path
      >
      <path
        class="line-art-extra"
        d="M420-40C478 72 474 283 583 273C694 263 758 213 859 251C944 284 982 357 1055 350"
        stroke-width="2.5"
        ><animate
          attributeName="opacity"
          values="0.62;0.34;0.62"
          keyTimes="0;0.57;1"
          begin="-17s"
          dur="24s"
          repeatCount="indefinite"
        /></path
      >
    </g>
  </svg>
  <svg
    class="bottom-line-art"
    viewBox="0 0 1000 420"
    preserveAspectRatio="xMidYMax meet"
    aria-hidden="true"
  >
    <g fill="none" stroke="currentColor" stroke-linecap="round" vector-effect="non-scaling-stroke">
      <path d="M-60 392C154 326 273 336 460 386C650 437 797 303 1060 365" stroke-width="2.5"
        ><animate
          attributeName="opacity"
          values="0.45;0.72;0.45"
          keyTimes="0;0.48;1"
          begin="-9s"
          dur="20s"
          repeatCount="indefinite"
        /></path
      >
      <path d="M-45 444C176 352 360 378 544 431C745 488 862 361 1050 398" stroke-width="2.5"
        ><animate
          attributeName="opacity"
          values="0.36;0.62;0.36"
          keyTimes="0;0.69;1"
          begin="-15s"
          dur="24s"
          repeatCount="indefinite"
        /></path
      >
    </g>
  </svg>
  <div class="app-chrome" data-tauri-drag-region aria-hidden="true"></div>
  {#if import.meta.env.DEV}
    <span class="development-label">dev</span>
  {/if}
  <div class="document-actions" class:window-unfocused={!windowFocused}>
    <button
      class="icon-button"
      onclick={() => void cycleTheme()}
      aria-label={`Theme: ${settings?.theme ?? 'system'}. Click to change`}
      title={`Theme: ${settings?.theme ?? 'system'}`}
    >
      {settings?.theme === 'dark' ? '☾' : settings?.theme === 'light' ? '☀' : '◐'}
    </button>
    <button
      class="icon-button"
      onclick={() => void changeTextScale(-1)}
      disabled={!settings || settings.textScale <= textScales[0]}
      aria-label="Decrease text size"
      title="Decrease text size">A−</button
    >
    <button
      class="icon-button"
      onclick={() => void changeTextScale(1)}
      disabled={!settings || settings.textScale >= textScales[textScales.length - 1]}
      aria-label="Increase text size"
      title="Increase text size">A+</button
    >
    <button onclick={() => void openSettings()}>Settings</button>
  </div>
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
    onselectstart={() => controller.setSelection(true)}
  >
    {#each rows as row (row.key)}
      <div class="motion-row" data-row-key={row.key} use:motion.row={row.key}>
        <div>
          {#if row.kind === 'project'}
            <h2>{row.folder}</h2>
          {:else if row.kind === 'branch'}
            <p>{row.branch}</p>
          {:else}
            {@const { entry, point, pointIndex } = row}
            {@const link = safeLink(point.link)}
            <ul class:branch-end={row.last}>
              <li
                class="status-point"
                class:removing={pendingRemoval.has(row.key)}
                class:expiring-soon={countExpiring.has(row.key) ||
                  expirySoon(
                    point.submitted_at ?? entry.updated_at,
                    settings?.autoClearAfterMinutes ?? null,
                    now,
                  )}
              >
                <span class="point-text"
                  >{#if point.done}<strong>DONE</strong>: {' '}{/if}{point.text}{#if link}{' '}<a
                      href={link}
                      target="_blank"
                      title={link}
                      rel="noopener noreferrer">{linkLabel(link)}</a
                    >{/if}</span
                >
                <span class="point-actions">
                  <span class="point-age"
                    >{relativeTime(point.submitted_at ?? entry.updated_at, now)}</span
                  >
                  <button
                    class="dismiss-point"
                    onclick={() => void dismissPoint(entry, pointIndex)}
                    disabled={dismissing.has(row.key) || pendingRemoval.has(row.key)}
                    aria-label={`Dismiss ${point.text}`}
                    title="Dismiss point">×</button
                  >
                </span>
              </li>
            </ul>
          {/if}
        </div>
      </div>
    {/each}
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
    display: flow-root;
  }
  .motion-row h2 {
    margin: 0 0 0.2em;
  }
  .motion-row + .motion-row h2 {
    margin-top: 0.8em;
  }
  .motion-row p {
    margin: 0 0 0.7em;
    color: var(--muted);
  }
  .motion-row ul {
    margin-bottom: 0;
  }
  .motion-row ul:not(.branch-end) {
    padding-bottom: 0.5em;
  }
  .motion-row ul.branch-end {
    padding-bottom: 1.1em;
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
  .line-art,
  .bottom-line-art {
    position: absolute;
    z-index: 0;
    height: auto;
    color: var(--accent);
    pointer-events: none;
  }
  .line-art {
    top: 0;
    right: 0;
    width: min(880px, 80%);
    aspect-ratio: 1000 / 480;
    opacity: 0.48;
  }
  .bottom-line-art {
    right: 0;
    bottom: 0;
    width: 100%;
    aspect-ratio: 1000 / 420;
    opacity: 0.55;
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
  .development-label {
    position: absolute;
    z-index: 2;
    top: 18px;
    right: 16px;
    transform: translateY(-50%);
    padding: 0.14rem 0.5rem;
    color: var(--muted);
    font-size: 0.65rem;
    letter-spacing: 0.04em;
    pointer-events: none;
    user-select: none;
  }
  .development-label + .document-actions {
    right: 60px;
  }
  .document-actions {
    position: fixed;
    z-index: 2;
    top: 8px;
    right: 10px;
    display: flex;
    gap: 0.35rem;
    opacity: 1;
    transition: opacity 160ms var(--ease-gentle-out);
  }
  .document-actions button {
    padding: 0.28rem 0.5rem;
    font-size: 0.7rem;
  }
  .document-actions button:not(:disabled):hover {
    border-color: var(--border);
    color: var(--accent);
  }
  .document-actions button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .icon-button {
    min-width: 1.8rem;
    padding-inline: 0.35rem !important;
  }
  .document-actions.window-unfocused {
    pointer-events: none;
    opacity: 0;
  }
  .status-error {
    color: #c43d35;
    font-size: 0.8rem;
  }
  .status-point {
    position: relative;
    line-height: 1.25;
    transition: opacity 160ms var(--ease-gentle-out);
  }
  .status-point.removing {
    pointer-events: none;
  }
  .status-point.expiring-soon {
    opacity: 0.42;
  }
  .development-label,
  .point-actions {
    border-radius: 999px;
    color: var(--muted);
    background: color-mix(in srgb, var(--bg) 88%, transparent);
    backdrop-filter: blur(28px) saturate(1.15);
    -webkit-backdrop-filter: blur(28px) saturate(1.15);
    box-shadow:
      0 2px 8px color-mix(in srgb, var(--text) 8%, transparent),
      inset 0 0 0 1px color-mix(in srgb, var(--bg) 28%, transparent);
  }
  .point-actions {
    position: absolute;
    top: 50%;
    right: 0;
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    padding: 0.14rem 0.28rem 0.14rem 0.5rem;
    transform: translateY(-50%);
    opacity: 0;
    pointer-events: none;
    font-size: 0.78em;
    white-space: nowrap;
    transition: opacity 120ms var(--ease-gentle-out);
  }
  .status-point:hover .point-actions,
  .status-point:focus-within .point-actions {
    opacity: 1;
    pointer-events: auto;
  }
  .dismiss-point {
    padding: 0 0.28rem;
    border: 0;
    border-radius: 0.3rem;
    color: inherit;
    background: transparent;
    font-size: 1.25em;
    line-height: 1;
  }
  .dismiss-point:not(:disabled):hover {
    color: #c43d35;
    background: color-mix(in srgb, #c43d35 12%, transparent);
  }
  .dismiss-point:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  @media (max-width: 440px), (max-height: 360px) {
    .line-art-extra {
      display: none;
    }
  }
  @media (max-width: 340px), (max-height: 280px) {
    .line-art-medium {
      display: none;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .line-art animate,
    .bottom-line-art animate {
      display: none;
    }
  }
</style>
