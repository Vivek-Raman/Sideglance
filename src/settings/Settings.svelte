<script lang="ts">
  import './settings-card.css';
  import { onMount } from 'svelte';
  import { command } from '../shared/commands';
  import { saveAppearance, savePinned } from './appearance';
  import { subscribeSettings } from './subscription';
  import { applyPrimaryColor, applyTheme, systemTheme } from '../shared/theme';
  import type { Settings } from '../shared/types';
  import AppearanceSettings from './AppearanceSettings.svelte';
  import RetentionSettings from './RetentionSettings.svelte';
  import UpdateSettings from './UpdateSettings.svelte';
  import AgentIntegrations from '../integrations/AgentIntegrations.svelte';
  const sections = [
    { id: 'appearance', label: 'Appearance' },
    { id: 'board', label: 'Board' },
    { id: 'agents', label: 'Agents' },
    { id: 'app', label: 'App' },
  ];
  function sectionFromHash() {
    const value = new URLSearchParams(window.location.hash.split('?')[1] ?? '').get('section');
    return sections.some((section) => section.id === value) ? value! : 'appearance';
  }
  let activeSection = sectionFromHash();
  function navigateTo(id: string) {
    activeSection = id;
    window.location.hash = `/settings?section=${id}`;
  }
  let settings: Settings | null = null;
  let saving = false;
  let appearanceError = '';
  let boardError = '';
  let appError = '';
  let loadError = '';
  let integrationsError = '';
  let resetVersion = 0;
  let settingsGeneration = 0;
  const primaryColors = [
    { name: 'Terracotta', value: '#c98c5a' },
    { name: 'Rose', value: '#c96b7b' },
    { name: 'Coral', value: '#d97757' },
    { name: 'Gold', value: '#c59a38' },
    { name: 'Olive', value: '#8b9b50' },
    { name: 'Sage', value: '#5f9b85' },
    { name: 'Teal', value: '#3f9a9a' },
    { name: 'Blue', value: '#5d8fc2' },
    { name: 'Violet', value: '#8a75bc' },
  ];
  function errorText(error: unknown) {
    return error instanceof Error ? error.message : String(error);
  }
  async function saveAppearanceSettings() {
    if (!settings) return;
    saving = true;
    appearanceError = '';
    try {
      await saveAppearance(settings);
    } catch (error) {
      appearanceError = errorText(error);
    } finally {
      saving = false;
    }
  }
  async function saveGrouping() {
    if (!settings) return;
    boardError = '';
    try {
      await command('set_status_grouping', {
        statusGrouping: settings.statusGrouping,
      });
    } catch (error) {
      boardError = errorText(error);
    }
  }
  async function saveUndoLimit(value: number) {
    if (!Number.isInteger(value) || value < 1 || value > 100) {
      appError = 'Undo history must be between 1 and 100 items';
      return;
    }
    saving = true;
    appError = '';
    try {
      await command('set_undo_limit', { undoLimit: value });
    } catch (error) {
      appError = errorText(error);
    } finally {
      saving = false;
    }
  }
  async function saveSortOrder() {
    if (!settings) return;
    saving = true;
    boardError = '';
    try {
      await command('set_project_sort_order', { projectSortOrder: settings.projectSortOrder });
    } catch (error) {
      boardError = errorText(error);
    } finally {
      saving = false;
    }
  }
  async function saveStatusSortOrder() {
    if (!settings) return;
    saving = true;
    boardError = '';
    try {
      await command('set_status_sort_order', { statusSortOrder: settings.statusSortOrder });
    } catch (error) {
      boardError = errorText(error);
    } finally {
      saving = false;
    }
  }
  async function savePinnedSetting(pinned: boolean) {
    const editGeneration = settingsGeneration;
    saving = true;
    boardError = '';
    try {
      await savePinned(pinned);
    } catch (error) {
      boardError = errorText(error);
      if (settings && settingsGeneration === editGeneration && settings.pinned === pinned) {
        settings.pinned = !pinned;
      }
    } finally {
      saving = false;
    }
  }
  async function resetSettings() {
    saving = true;
    appError = '';
    try {
      settings = await command('reset_settings');
      resetVersion += 1;
      applyTheme(settings.theme);
      applyPrimaryColor(settings.primaryColor);
    } catch (error) {
      appError = errorText(error);
    } finally {
      saving = false;
    }
  }
  function applySettings(value: Settings) {
    settingsGeneration += 1;
    settings = value;
    applyTheme(value.theme);
    applyPrimaryColor(value.primaryColor);
  }
  onMount(() => {
    let disposed = false;
    const onThemeChange = () => {
      if (settings?.theme === 'system') applyTheme(settings.theme);
    };
    systemTheme.addEventListener('change', onThemeChange);
    const subscription = subscribeSettings(
      (value) => {
        if (!disposed) applySettings(value);
      },
      (error) => {
        if (!disposed) loadError = errorText(error);
      },
      () => command('get_settings'),
    );
    return () => {
      disposed = true;
      systemTheme.removeEventListener('change', onThemeChange);
      subscription.dispose();
    };
  });
</script>

<svelte:window onhashchange={() => (activeSection = sectionFromHash())} />
<div class="settings-layout">
  <aside class="settings-sidebar">
    <h1>Settings</h1>
    <nav aria-label="Settings sections">
      {#each sections as section}
        <button
          class:active={activeSection === section.id}
          aria-current={activeSection === section.id ? 'page' : undefined}
          onclick={() => navigateTo(section.id)}>{section.label}</button
        >
      {/each}
    </nav>
  </aside>
  <main class="settings-page">
    <div class="settings-content">
      <header>
        <h2 class="page-title">
          {sections.find((section) => section.id === activeSection)?.label}
        </h2>
      </header>
      {#if settings}
        {#if activeSection === 'appearance'}
          <AppearanceSettings
            {settings}
            {primaryColors}
            onSave={() => void saveAppearanceSettings()}
          />
        {:else if activeSection === 'board'}
          <section class="settings-group">
            <div class="setting-row">
              <label for="status-grouping">Group status items by</label>
              <select
                id="status-grouping"
                bind:value={settings.statusGrouping}
                onchange={() => void saveGrouping()}
              >
                <option value="project-directory">Directory → branch</option><option
                  value="repository">Repository → branch</option
                >
              </select>
            </div>
            <label class="checkbox"
              ><input
                type="checkbox"
                bind:checked={settings.pinned}
                onchange={() => void savePinnedSetting(settings!.pinned)}
              /> Keep viewer always on top</label
            >
          </section>
          <RetentionSettings
            {settings}
            {saving}
            {resetVersion}
            onSaved={(minutes) => {
              if (settings) settings.autoClearAfterMinutes = minutes;
            }}
            onError={(error) => (boardError = errorText(error))}
            onSortChange={() => void saveSortOrder()}
            onStatusSortChange={() => void saveStatusSortOrder()}
          />
        {:else if activeSection === 'agents'}
          <AgentIntegrations onError={(error) => (integrationsError = errorText(error))} />
          {#if integrationsError}<p class="error" role="alert">{integrationsError}</p>{/if}
        {:else}
          <UpdateSettings />
          <details class="settings-group">
            <summary>Advanced</summary>
            <label
              >Undo history size <input
                type="number"
                min="1"
                max="100"
                step="1"
                value={settings.undoLimit}
                disabled={saving}
                onchange={(event) => void saveUndoLimit(Number(event.currentTarget.value))}
              /></label
            >
            <p>Keep recently dismissed items for this viewer session.</p>
          </details>
          <section class="reset settings-group">
            <div>
              <h2>Reset settings</h2>
              <p>Restore appearance, viewer, and status settings.</p>
            </div>
            <button onclick={() => void resetSettings()} disabled={saving}>Reset settings</button>
          </section>
        {/if}
        {#if saving}<p class="status" role="status">Saving…</p>{/if}
      {/if}
      {#if loadError}<p class="error" role="alert">{loadError}</p>{/if}
      {#if activeSection === 'appearance' && appearanceError}<p class="error" role="alert">
          {appearanceError}
        </p>{/if}
      {#if activeSection === 'board' && boardError}<p class="error" role="alert">
          {boardError}
        </p>{/if}
      {#if activeSection === 'app' && appError}<p class="error" role="alert">{appError}</p>{/if}
    </div>
  </main>
</div>

<style>
  .settings-layout {
    width: 100%;
    display: grid;
    grid-template-columns: 190px minmax(0, 1fr);
    height: 100dvh;
    overflow: hidden;
  }
  .settings-sidebar {
    padding: 28px 14px;
    border-right: 1px solid var(--border);
    overflow-y: auto;
  }
  .settings-sidebar h1 {
    margin: 0 12px 24px;
    font-size: 1.25rem;
  }
  nav {
    display: grid;
    gap: 4px;
  }
  nav button {
    padding: 10px 12px;
    text-align: left;
    color: var(--muted);
    background: transparent;
    border-left: 3px solid transparent;
    font-weight: 500;
  }
  nav button:hover {
    background: color-mix(in srgb, var(--text) 5%, transparent);
  }
  nav button.active {
    color: var(--text);
    border-left-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 14%, transparent);
  }
  nav button:focus-visible {
    outline: 2px solid var(--focus-ring, var(--accent));
    outline-offset: -2px;
  }
  .settings-page {
    display: block;
    min-height: 0;
    min-width: 0;
    overflow-y: auto;
    padding: 28px 32px;
  }
  .settings-content {
    max-width: 820px;
    margin: 0 auto;
  }
  .settings-page .page-title {
    font-size: 1.5rem;
  }
  .settings-page header {
    margin-bottom: 28px;
  }
  .settings-page h2 {
    margin: 0;
  }
  .settings-page h2 {
    font-size: 1rem;
  }
  .reset p {
    color: var(--muted);
    margin: 0.4rem 0 0;
  }
  .setting-row {
    display: grid;
    grid-template-columns: minmax(150px, 1fr) minmax(0, 2fr);
    align-items: center;
    gap: 1rem;
  }
  select,
  input[type='number'] {
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    padding: 0.55rem;
    color: var(--text);
    background: var(--bg);
    font: inherit;
  }
  details p {
    color: var(--muted);
  }
  .checkbox {
    display: flex;
    align-items: center;
    gap: 0.55rem;
  }
  .reset {
    grid-template-columns: 1fr auto;
    align-items: center;
    margin-bottom: 0;
  }
  .reset button,
  button {
    border: 0;
    border-radius: 0.4rem;
    padding: 0.5rem 0.7rem;
    color: #111;
    background: #f2f2f2;
    font: inherit;
    font-size: 0.8rem;
    font-weight: 600;
    cursor: pointer;
  }
  .reset button:disabled {
    cursor: wait;
    opacity: 0.65;
  }
  .error {
    color: #c43d35;
  }
  .status {
    color: var(--muted);
  }
  @media (max-width: 720px) {
    .settings-layout {
      grid-template-columns: 155px minmax(0, 1fr);
    }
    .settings-sidebar {
      padding: 20px 8px;
    }
    nav button {
      padding: 10px 8px;
    }
    .settings-page {
      padding: 20px;
    }
    .reset {
      grid-template-columns: 1fr;
      gap: 0.6rem;
    }
  }
</style>
