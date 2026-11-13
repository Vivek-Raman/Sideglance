<script lang="ts">
  const settings = import('./Settings.svelte');
</script>

{#await settings}
  <main class="route-state" aria-busy="true"><p role="status">Loading settings…</p></main>
{:then loaded}
  <svelte:component this={loaded.default} />
{:catch}
  <main class="route-state">
    <p role="alert">Settings could not be loaded.</p>
    <button onclick={() => window.location.reload()}>Reload settings</button>
  </main>
{/await}

<style>
  .route-state {
    padding: 2rem;
    gap: 1rem;
  }
  button {
    align-self: flex-start;
    font: inherit;
    padding: 0.5rem 1rem;
  }
</style>
