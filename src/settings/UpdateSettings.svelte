<script lang="ts">
  import { getVersion } from '@tauri-apps/api/app';
  import { check, type Update } from '@tauri-apps/plugin-updater';
  import { onMount } from 'svelte';
  import { command } from '../shared/commands';
  import { renderReleaseNotes } from './release-notes';
  import type { UpdateState } from '../shared/types';

  let state: UpdateState = { status: 'idle', currentVersion: '' };
  let update: Update | null = null;

  function errorText(error: unknown) {
    return error instanceof Error ? error.message : String(error);
  }

  onMount(() => {
    void getVersion()
      .then((currentVersion) => (state = { status: 'idle', currentVersion }))
      .catch(
        (error) =>
          (state = { status: 'error', currentVersion: 'Unknown', message: errorText(error) }),
      );
  });

  async function checkForUpdates() {
    if (import.meta.env.DEV || state.status === 'checking' || state.status === 'downloading')
      return;
    const currentVersion = state.currentVersion;
    state = { status: 'checking', currentVersion };
    update = null;
    try {
      const result = await check();
      if (!result) {
        state = { status: 'up-to-date', currentVersion };
        return;
      }
      update = result;
      state = {
        status: 'available',
        currentVersion,
        version: result.version,
        notes: result.body?.trim() || null,
      };
    } catch (error) {
      state = {
        status: 'error',
        currentVersion,
        message: `Could not check for updates: ${errorText(error)}`,
      };
    }
  }

  async function downloadAndInstall() {
    if (import.meta.env.DEV || !update || state.status !== 'available') return;
    const { currentVersion, version } = state;
    state = { status: 'downloading', currentVersion, version, downloaded: 0, contentLength: null };
    try {
      await update.downloadAndInstall((event) => {
        if (event.event === 'Started') {
          state = {
            status: 'downloading',
            currentVersion,
            version,
            downloaded: 0,
            contentLength: event.data.contentLength ?? null,
          };
        } else if (event.event === 'Progress' && state.status === 'downloading') {
          state = { ...state, downloaded: state.downloaded + event.data.chunkLength };
        }
      });
      state = { status: 'ready-to-restart', currentVersion, version };
    } catch (error) {
      state = {
        status: 'error',
        currentVersion,
        message: `Could not install the update: ${errorText(error)}`,
      };
    } finally {
      await update?.close().catch(() => undefined);
      update = null;
    }
  }

  function progressLabel() {
    if (state.status !== 'downloading') return '';
    if (!state.contentLength) return `${Math.round(state.downloaded / 1024)} KB downloaded`;
    return `${Math.round((state.downloaded / state.contentLength) * 100)}% downloaded`;
  }
</script>

<section class="settings-group">
  <div class="group-heading">
    <h2>Updates</h2>
    <p>Version {state.currentVersion || 'Loading…'}</p>
  </div>
  {#if import.meta.env.DEV}<p class="status">Updates are disabled in development mode.</p>{/if}
  {#if state.status === 'available'}
    <p>
      Version {state.version} is ready. Installing closes Sideglance and starts the new version.
    </p>
    {#if state.notes}
      <div class="release-notes">{@html renderReleaseNotes(state.notes)}</div>
    {/if}
    <button disabled={import.meta.env.DEV} onclick={() => void downloadAndInstall()}
      >Download and install</button
    >
  {:else if state.status === 'downloading'}
    <p>{progressLabel()}</p>
  {:else if state.status === 'ready-to-restart'}
    <p>Version {state.version} is installed. Restart Sideglance to use it.</p>
    <button disabled={import.meta.env.DEV} onclick={() => void command('restart_app')}
      >Restart Sideglance</button
    >
  {:else}
    <button
      onclick={() => void checkForUpdates()}
      disabled={import.meta.env.DEV || state.status === 'checking' || !state.currentVersion}
    >
      {state.status === 'checking' ? 'Checking…' : 'Check for updates'}
    </button>
    {#if state.status === 'up-to-date'}<p class="status">Sideglance is up to date.</p>{/if}
    {#if state.status === 'error'}<p class="error">{state.message}</p>{/if}
  {/if}
</section>

<style>
  .settings-group {
    display: grid;
    gap: 1rem;
    min-width: 0;
    margin: 0 0 24px;
    padding: 1.1rem;
    border: 1px solid var(--border);
    border-radius: 0.65rem;
  }
  h2 {
    margin: 0;
    font-size: 1rem;
  }
  p {
    margin: 0;
    line-height: 1.5;
  }
  .group-heading p {
    color: var(--muted);
    margin-top: 0.4rem;
  }
  button {
    justify-self: start;
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
  button:disabled {
    cursor: wait;
    opacity: 0.65;
  }
  button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .status {
    color: var(--muted);
  }
  .error {
    color: #c43d35;
    overflow-wrap: anywhere;
  }
  .release-notes {
    min-width: 0;
    max-height: 24rem;
    overflow: auto;
    overflow-wrap: anywhere;
    border-top: 1px solid var(--border);
    padding-top: 1rem;
    font-size: 0.9rem;
    line-height: 1.6;
  }
  .release-notes :global(h1),
  .release-notes :global(h2),
  .release-notes :global(h3),
  .release-notes :global(h4),
  .release-notes :global(h5),
  .release-notes :global(h6) {
    font-size: 1rem;
    line-height: 1.4;
    margin: 1rem 0 0.5rem;
  }
  .release-notes :global(p),
  .release-notes :global(ul),
  .release-notes :global(ol),
  .release-notes :global(pre),
  .release-notes :global(blockquote) {
    margin: 0.5rem 0;
  }
  .release-notes :global(ul),
  .release-notes :global(ol) {
    padding-left: 1.5rem;
  }
  .release-notes :global(a) {
    color: var(--accent);
    text-decoration: underline;
  }
  .release-notes :global(pre) {
    overflow-x: auto;
    padding: 0.6rem;
    background: color-mix(in srgb, var(--text) 5%, transparent);
    border-radius: 0.3rem;
  }
  .release-notes :global(code) {
    font-size: 0.85em;
  }
  .release-notes :global(blockquote) {
    border-left: 2px solid var(--border);
    padding-left: 0.75rem;
    color: var(--muted);
  }
  .release-notes :global(> :first-child) {
    margin-top: 0;
  }
  .release-notes :global(> :last-child) {
    margin-bottom: 0;
  }
</style>
