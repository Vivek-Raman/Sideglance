<script lang="ts">
  import RetentionPreview from './RetentionPreview.svelte';
  import { command } from '../shared/commands';
  import type { Settings } from '../shared/types';
  export let settings: Settings;
  export let saving = false;
  export let resetVersion = 0;
  export let onError: (error: unknown) => void;
  let enabled = settings.maxStatusItemsPerProject != null;
  let count = settings.maxStatusItemsPerProject ?? 10;
  let baseline = settings.maxStatusItemsPerProject;
  let observedReset = resetVersion;
  let pending = false;
  let revision = 0;
  $: if (baseline !== settings.maxStatusItemsPerProject || observedReset !== resetVersion) {
    baseline = settings.maxStatusItemsPerProject;
    observedReset = resetVersion;
    enabled = baseline != null;
    count = baseline ?? 10;
    revision += 1;
    pending = false;
  }
  $: confirmation = enabled && (baseline == null || count < baseline);
  async function save() {
    if (pending || saving) return;
    if (enabled && (!Number.isSafeInteger(count) || count < 1)) {
      onError('Maximum status items must be a whole number of at least 1.');
      return;
    }
    const current = ++revision;
    const next = enabled ? count : null;
    pending = true;
    try {
      await command('set_max_status_items_per_project', { maxStatusItemsPerProject: next });
      if (current !== revision) return;
      settings.maxStatusItemsPerProject = next;
      baseline = next;
    } catch (error) {
      if (current === revision) onError(error);
    } finally {
      if (current === revision) pending = false;
    }
  }
</script>

<div class="count-rule">
  <label>
    <input
      type="checkbox"
      bind:checked={enabled}
      disabled={saving || pending}
      onchange={() => {
        if (!enabled) void save();
      }}
    />
    Count-based deletion
  </label>
  {#if enabled}
    <label class="count-input">
      Maximum status items per project
      <input
        type="number"
        min="1"
        step="1"
        bind:value={count}
        disabled={saving || pending}
        onchange={() => {
          if (!confirmation) void save();
        }}
      />
    </label>
    {#if confirmation}
      <RetentionPreview
        autoClearAfterMinutes={settings.autoClearAfterMinutes}
        maxStatusItemsPerProject={count}
        disabled={saving || pending}
        onApply={save}
      />
    {/if}
  {/if}
</div>

<style>
  .count-rule {
    display: grid;
    gap: 0.7rem;
  }
  label {
    display: flex;
    align-items: center;
    gap: 0.55rem;
  }
  .count-input {
    flex-wrap: wrap;
  }
  input[type='number'] {
    width: 7rem;
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    padding: 0.55rem 0.65rem;
    color: var(--text);
    background: var(--bg);
    font: inherit;
  }
</style>
