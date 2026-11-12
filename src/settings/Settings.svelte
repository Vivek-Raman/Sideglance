<script lang="ts">
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
    { id: 'viewer', label: 'Viewer' },
    { id: 'undo', label: 'Undo dismissals' },
    { id: 'status', label: 'Status' },
    { id: 'integrations', label: 'Agent integration' },
    { id: 'updates', label: 'Updates' },
    { id: 'reset', label: 'Reset settings' },
  ];
  let scrollContainer: HTMLElement;
  let activeSection = 'appearance';
  function navigateTo(id: string) {
    const target = scrollContainer.querySelector<HTMLElement>(`[data-section="${id}"]`);
    if (!target) return;
    scrollContainer.scrollTop +=
      target.getBoundingClientRect().top - scrollContainer.getBoundingClientRect().top - 28;
    activeSection = id;
  }
  function trackSection() {
    const top = scrollContainer.getBoundingClientRect().top + 60;
    let current = sections[0].id;
    for (const section of sections) {
      const target = scrollContainer.querySelector(`[data-section="${section.id}"]`);
      if (target && target.getBoundingClientRect().top <= top) current = section.id;
    }
    activeSection = current;
  }
  let settings: Settings | null = null;
  let saving = false;
  let settingsError = '';
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
    settingsError = '';
    try {
      await saveAppearance(settings);
    } catch (error) {
      settingsError = errorText(error);
    } finally {
      saving = false;
    }
  }
  async function saveHeaderLayout() {
    if (!settings) return;
    settingsError = '';
    try {
      await command('set_header_settings', {
        headerTitle: settings.headerTitle,
        headerSubtitle: settings.headerSubtitle,
      });
    } catch (error) {
      settingsError = errorText(error);
    }
  }
  async function saveUndoLimit(value: number) {
    if (!Number.isInteger(value) || value < 1 || value > 100) {
      settingsError = 'Undo history must be between 1 and 100 items';
      return;
    }
    saving = true;
    settingsError = '';
    try {
      await command('set_undo_limit', { undoLimit: value });
    } catch (error) {
      settingsError = errorText(error);
    } finally {
      saving = false;
    }
  }
  async function saveSortOrder() {
    if (!settings) return;
    saving = true;
    settingsError = '';
    try {
      await command('set_project_sort_order', { projectSortOrder: settings.projectSortOrder });
    } catch (error) {
      settingsError = errorText(error);
    } finally {
      saving = false;
    }
  }
  async function saveStatusSortOrder() {
    if (!settings) return;
    saving = true;
    settingsError = '';
    try {
      await command('set_status_sort_order', { statusSortOrder: settings.statusSortOrder });
    } catch (error) {
      settingsError = errorText(error);
    } finally {
      saving = false;
    }
  }
  async function savePinnedSetting(pinned: boolean) {
    const editGeneration = settingsGeneration;
    saving = true;
    settingsError = '';
    try {
      await savePinned(pinned);
    } catch (error) {
      settingsError = errorText(error);
      if (settings && settingsGeneration === editGeneration && settings.pinned === pinned) {
        settings.pinned = !pinned;
      }
    } finally {
      saving = false;
    }
  }
  async function resetSettings() {
    saving = true;
    settingsError = '';
    try {
      settings = await command('reset_settings');
      resetVersion += 1;
      applyTheme(settings.theme);
      applyPrimaryColor(settings.primaryColor);
    } catch (error) {
      settingsError = errorText(error);
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
        if (!disposed) settingsError = errorText(error);
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

<div class="settings-layout">
  <aside class="settings-sidebar">
    <h1>Settings</h1>
    <nav aria-label="Settings sections">
      {#each sections as section}
        <button
          class:active={activeSection === section.id}
          aria-current={activeSection === section.id ? 'location' : undefined}
          disabled={!settings}
          onclick={() => navigateTo(section.id)}>{section.label}</button
        >
      {/each}
    </nav>
  </aside>
  <main class="settings-page" bind:this={scrollContainer} onscroll={trackSection}>
    <div class="settings-content">
      <header>
        <h2 class="page-title">Preferences</h2>
        <p>Choose what Sideglance shows and how it appears.</p>
      </header>
      {#if settings}
        <div data-section="appearance">
          <AppearanceSettings
            {settings}
            {primaryColors}
            onHeaderSave={() => void saveHeaderLayout()}
            onSave={() => void saveAppearanceSettings()}
          />
        </div>
        <section data-section="viewer" class="settings-group">
          <div class="group-heading">
            <h2>Viewer</h2>
            <p>Keep the viewer above other windows.</p>
          </div>
          <label class="checkbox"
            ><input
              type="checkbox"
              bind:checked={settings.pinned}
              onchange={() => void savePinnedSetting(settings!.pinned)}
            /> Keep viewer always on top</label
          >
        </section>
        <section data-section="undo" class="settings-group">
          <div class="group-heading">
            <h2>Undo dismissals</h2>
            <p>Keep the most recent manually dismissed lines for this viewer session.</p>
          </div>
          <label
            >Undo history size
            <input
              type="number"
              min="1"
              max="100"
              step="1"
              value={settings.undoLimit}
              disabled={saving}
              onchange={(event) => void saveUndoLimit(Number(event.currentTarget.value))}
            />
          </label>
        </section>
        <div data-section="status">
          <RetentionSettings
            {settings}
            {saving}
            {resetVersion}
            onSaved={(minutes) => {
              if (settings) settings.autoClearAfterMinutes = minutes;
            }}
            onError={(error) => (settingsError = errorText(error))}
            onSortChange={() => void saveSortOrder()}
            onStatusSortChange={() => void saveStatusSortOrder()}
          />
        </div>
        <div data-section="integrations">
          <AgentIntegrations onError={(error) => (integrationsError = errorText(error))} />
          {#if integrationsError}<p class="error">{integrationsError}</p>{/if}
        </div>
        <div data-section="updates"><UpdateSettings /></div>
        <section data-section="reset" class="reset settings-group">
          <div>
            <h2>Reset settings</h2>
            <p>Restore appearance, viewer, and status settings.</p>
          </div>
          <button onclick={() => void resetSettings()} disabled={saving}>Reset settings</button>
        </section>
        {#if saving}<p class="status">Saving…</p>{/if}{#if settingsError}<p class="error">
            {settingsError}
          </p>{/if}
      {/if}
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
    outline: 2px solid var(--accent);
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
    padding-bottom: calc(100dvh - 120px);
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
  .settings-page header p,
  .reset p {
    color: var(--muted);
    margin: 0.4rem 0 0;
  }
  .settings-group {
    display: grid;
    gap: 1rem;
    margin: 0 0 24px;
    padding: 1.1rem;
    border: 1px solid var(--border);
    border-radius: 0.65rem;
  }
  .group-heading {
    display: grid;
    gap: 0.1rem;
  }
  .group-heading p {
    color: var(--muted);
    margin: 0.4rem 0 0;
  }
  .checkbox {
    display: flex;
    align-items: center;
    gap: 0.55rem;
  }
  .reset {
    grid-template-columns: 1fr auto;
    align-items: center;
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
