<script lang="ts">
  import { command } from '../shared/commands';
  import type { RemovedPoint } from '../shared/types';
  export let onRestored: () => Promise<void>;
  let opened = false;
  let busy = false;
  let error = '';
  let items: RemovedPoint[] = [];
  let shown = 50;
  async function load() {
    busy = true;
    error = '';
    shown = 50;
    try {
      items = await command('get_recently_removed');
    } catch (failure) {
      error = String(failure);
    } finally {
      busy = false;
    }
  }
  async function restore(id: string) {
    busy = true;
    error = '';
    try {
      await command('restore_removed', { id });
      items = items.filter((item) => item.id !== id);
      await onRestored();
    } catch (failure) {
      error = String(failure);
    } finally {
      busy = false;
    }
  }
</script>

<details
  ontoggle={(event) => {
    opened = event.currentTarget.open;
    if (opened) void load();
  }}
>
  <summary>Recently removed</summary>
  {#if opened}
    <p>Automatically removed items are kept for up to 30 days within the local history limit.</p>
    {#if busy}<p role="status">Loading…</p>{/if}
    {#if error}<p role="alert">{error}</p>
      <button disabled={busy} onclick={() => void load()}>Try again</button>{/if}
    {#if !busy && !error && !items.length}<p>No recently removed items.</p>{/if}
    <ul>
      {#each items.slice(0, shown) as item (item.id)}
        <li>
          <span>{item.entry.points[0]?.text ?? 'Removed item'}</span>
          <small
            >{item.entry.folder_name} · {item.reason} ·
            <time datetime={item.removed_at}>{new Date(item.removed_at).toLocaleString()}</time
            ></small
          >
          <button
            disabled={busy}
            onclick={() => void restore(item.id)}
            aria-label={`Restore and pin ${item.entry.points[0]?.text ?? 'item'}`}
            >Restore and pin</button
          >
        </li>
      {/each}
    </ul>
    {#if items.length > shown}<button onclick={() => (shown += 50)}>Show more removed items</button
      >{/if}
  {/if}
</details>

<style>
  details {
    margin-top: 1rem;
    font-size: 0.85rem;
  }
  summary {
    cursor: pointer;
    color: var(--muted);
  }
  p {
    margin: 0.5rem 0;
  }
  li {
    margin: 0.7rem 0;
    overflow-wrap: anywhere;
  }
  small {
    display: block;
    color: var(--muted);
  }
  button {
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    margin-top: 0.3rem;
    color: var(--text);
    background: var(--bg);
    font: inherit;
    padding: 0.3rem 0.5rem;
    cursor: pointer;
  }
  button:focus-visible,
  summary:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
</style>
