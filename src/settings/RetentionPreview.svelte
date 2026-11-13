<script lang="ts">
  import { onDestroy } from 'svelte';
  import { command } from '../shared/commands';
  export let autoClearAfterMinutes: number | null;
  export let maxStatusItemsPerProject: number | null;
  export let disabled = false;
  export let onApply: () => Promise<void>;
  let count: number | null = null;
  let busy = false;
  let error = '';
  let generation = 0;
  let previous = '';
  $: key = JSON.stringify([autoClearAfterMinutes, maxStatusItemsPerProject]);
  $: if (key !== previous) {
    previous = key;
    generation++;
    count = null;
    busy = false;
    error = '';
  }
  onDestroy(() => generation++);
  async function review() {
    const current = ++generation;
    busy = true;
    error = '';
    try {
      const value = await command('preview_retention', {
        autoClearAfterMinutes,
        maxStatusItemsPerProject,
      });
      if (current === generation) count = value;
    } catch (failure) {
      if (current === generation) error = String(failure);
    } finally {
      if (current === generation) busy = false;
    }
  }
</script>

<div class="retention-preview">
  {#if count === null}
    <button disabled={disabled || busy} onclick={() => void review()}
      >{busy ? 'Checking items…' : 'Review removal'}</button
    >
  {:else}
    <p role="status">
      About {count}
      {count === 1 ? 'item would' : 'items would'} be removed now. Incoming updates may change this count.
      Removed items can be recovered from Recently removed.
    </p>
    <button disabled={disabled || busy} onclick={() => void onApply()}>Apply deletion rule</button>
  {/if}
  {#if error}<p role="alert">{error}</p>{/if}
</div>

<style>
  .retention-preview {
    display: grid;
    gap: 0.5rem;
  }
  p {
    margin: 0;
    font-size: 0.85rem;
    line-height: 1.4;
  }
  button {
    justify-self: start;
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    padding: 0.5rem 0.7rem;
    color: var(--text);
    background: var(--bg);
    font: inherit;
    font-size: 0.8rem;
    font-weight: 600;
    cursor: pointer;
  }
  button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
</style>
