<script lang="ts">
  import './toolbar-button.css';
  import SettingsIcon from '@lucide/svelte/icons/settings';
  import Moon from '@lucide/svelte/icons/moon';
  import Sun from '@lucide/svelte/icons/sun';
  import Monitor from '@lucide/svelte/icons/monitor';
  import ZoomOut from '@lucide/svelte/icons/zoom-out';
  import ZoomIn from '@lucide/svelte/icons/zoom-in';
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
  <button
    class="toolbar-button"
    onclick={() => void onTheme()}
    aria-label={`Theme: ${settings?.theme ?? 'system'}. Click to change`}
    title={`Theme: ${settings?.theme ?? 'system'}`}
  >
    {#if settings?.theme === 'dark'}
      <Moon size={15} stroke-width={1.7} aria-hidden="true" />
    {:else if settings?.theme === 'light'}
      <Sun size={15} stroke-width={1.7} aria-hidden="true" />
    {:else}
      <Monitor size={15} stroke-width={1.7} aria-hidden="true" />
    {/if}
  </button>
  <button
    class="toolbar-button"
    onclick={() => void onTextScale(-1)}
    disabled={!settings || settings.textScale <= textScales[0]}
    aria-label="Decrease text size"
    title="Decrease text size"><ZoomOut size={15} stroke-width={1.7} aria-hidden="true" /></button
  >
  <button
    class="toolbar-button"
    onclick={() => void onTextScale(1)}
    disabled={!settings || settings.textScale >= textScales[textScales.length - 1]}
    aria-label="Increase text size"
    title="Increase text size"><ZoomIn size={15} stroke-width={1.7} aria-hidden="true" /></button
  >
  <button
    class="toolbar-button"
    onclick={() => void onSettings()}
    aria-label="Settings"
    title="Settings"><SettingsIcon size={15} stroke-width={1.7} aria-hidden="true" /></button
  >
  <div class="update-control" class:persistent-update={updatePersistent}>
    <ToolbarUpdateButton
      {updatesEnabled}
      automaticChecks={automaticUpdateChecks}
      onPersistentChange={(value) => (updatePersistent = value)}
    />
  </div>
</div>

<style>
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
  .update-control {
    display: flex;
    align-items: center;
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
