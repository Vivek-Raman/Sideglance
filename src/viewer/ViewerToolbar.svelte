<script lang="ts">
  import ToolbarUpdateButton from './ToolbarUpdateButton.svelte';
  import type { Settings } from '../shared/types';
  export let settings: Settings | null;
  export let updatesEnabled = !import.meta.env.DEV;
  export let automaticUpdateChecks = true;
  let updatePersistent = false;
  export let windowFocused: boolean;
  export let onTheme: () => Promise<void>;
  export let onTextScale: (direction: -1 | 1) => Promise<void>;
  export let onSettings: () => Promise<void>;
  const textScales = [0.85, 1, 1.15, 1.35];
</script>

{#if import.meta.env.DEV}
  <span class="development-label">dev</span>
{/if}
<div class="document-actions" class:window-unfocused={!windowFocused}>
  <div class="update-control" class:persistent-update={updatePersistent}>
    <ToolbarUpdateButton
      {updatesEnabled}
      automaticChecks={automaticUpdateChecks}
      onPersistentChange={(value) => (updatePersistent = value)}
    />
  </div>
  <button
    class="icon-button"
    onclick={() => void onTheme()}
    aria-label={`Theme: ${settings?.theme ?? 'system'}. Click to change`}
    title={`Theme: ${settings?.theme ?? 'system'}`}
  >
    {settings?.theme === 'dark' ? '☾' : settings?.theme === 'light' ? '☀' : '◐'}
  </button>
  <button
    class="icon-button"
    onclick={() => void onTextScale(-1)}
    disabled={!settings || settings.textScale <= textScales[0]}
    aria-label="Decrease text size"
    title="Decrease text size">A−</button
  >
  <button
    class="icon-button"
    onclick={() => void onTextScale(1)}
    disabled={!settings || settings.textScale >= textScales[textScales.length - 1]}
    aria-label="Increase text size"
    title="Increase text size">A+</button
  >
  <button onclick={() => void onSettings()}>Settings</button>
</div>

<style>
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
    align-items: center;
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
  .document-actions > * {
    transition: opacity 160ms var(--ease-gentle-out);
  }
  .document-actions.window-unfocused > :not(.persistent-update) {
    pointer-events: none;
    opacity: 0;
  }
  @media (prefers-reduced-motion: reduce) {
    .document-actions > * {
      transition: none;
    }
  }
  .development-label {
    border-radius: 999px;
    color: var(--muted);
    background: color-mix(in srgb, var(--bg) 88%, transparent);
    backdrop-filter: blur(28px) saturate(1.15);
    -webkit-backdrop-filter: blur(28px) saturate(1.15);
    box-shadow:
      0 2px 8px color-mix(in srgb, var(--text) 8%, transparent),
      inset 0 0 0 1px color-mix(in srgb, var(--bg) 28%, transparent);
  }
</style>
