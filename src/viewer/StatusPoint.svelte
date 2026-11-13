<script lang="ts">
  import type { StatusEntry, StatusPoint } from '../shared/types';
  import { relativeTime } from './rows';
  export let sessionLabel: string | null = null;
  export let entry: StatusEntry;
  export let point: StatusPoint;
  export let last: boolean;
  export let removing: boolean;
  export let dismissing: boolean;
  export let expiring: boolean;
  export let now: number;
  export let onPin: () => Promise<void>;
  export let onDismiss: () => Promise<void>;
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
  $: link = safeLink(point.link);
</script>

<ul class:branch-end={last}>
  <li class="status-point" class:removing class:expiring-soon={expiring}>
    {#if sessionLabel}<span class="session-label" title={entry.session_id ?? 'Legacy session'}
        >{sessionLabel}<span class="sr-only">: {entry.session_id}</span></span
      >{/if}
    <span class="point-text"
      >{#if point.done}<strong>DONE</strong>: {' '}{/if}{point.text}{#if link}{' '}<a
          href={link}
          target="_blank"
          title={link}
          rel="noopener noreferrer">{linkLabel(link)}</a
        >{/if}</span
    >
    <span class="point-actions" class:pinned={point.pinned}>
      <button
        class="pin-point"
        class:active={point.pinned}
        onclick={() => void onPin()}
        disabled={dismissing || removing}
        aria-label={`${point.pinned ? 'Unpin' : 'Pin'} ${point.text}`}
        aria-pressed={!!point.pinned}
        title={point.pinned ? 'Unpin point' : 'Pin point'}
      >
        <svg
          width="12"
          height="12"
          viewBox="0 0 24 24"
          fill={point.pinned ? 'currentColor' : 'none'}
          stroke="currentColor"
          stroke-width="2"
          aria-hidden="true"><path d="m16 3 5 5-4 1-4 4v4l-3-3-7 7 7-7-3-3h4l4-4z" /></svg
        >
      </button>
      <span class="point-age">{relativeTime(point.submitted_at ?? entry.updated_at, now)}</span>
      <button
        class="dismiss-point"
        onclick={() => void onDismiss()}
        disabled={!!point.pinned || dismissing || removing}
        aria-label={`Dismiss ${point.text}`}
        title="Dismiss point">×</button
      >
    </span>
  </li>
</ul>

<style>
  .session-label {
    color: var(--muted);
    font-size: 0.75em;
    margin-right: 0.4rem;
  }
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
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
  ul {
    margin-bottom: 0;
  }
  ul:not(.branch-end) {
    padding-bottom: 0.5em;
  }
  ul.branch-end {
    padding-bottom: 1.1em;
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
  .point-actions.pinned {
    opacity: 1;
    pointer-events: auto;
  }
  .pin-point {
    display: inline-flex;
    padding: 0.15rem;
    color: inherit;
  }
  .pin-point.active,
  .pin-point:hover {
    color: var(--accent);
  }
  .pin-point:focus-visible {
    outline: 2px solid var(--accent);
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
</style>
