<script lang="ts">
  import { getVersion } from '@tauri-apps/api/app';
  import { check, type Update } from '@tauri-apps/plugin-updater';
  import { onMount } from 'svelte';
  import { command } from '../shared/commands';
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
    if (state.status === 'checking' || state.status === 'downloading') return;
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
    if (!update || state.status !== 'available') return;
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
  {#if state.status === 'available'}
    <p>
      Version {state.version} is ready. Installing closes Sideglance and starts the new version.
    </p>
    {#if state.notes}<pre>{state.notes}</pre>{/if}
    <button onclick={() => void downloadAndInstall()}>Download and install</button>
  {:else if state.status === 'downloading'}
    <p>{progressLabel()}</p>
  {:else if state.status === 'ready-to-restart'}
    <p>Version {state.version} is installed. Restart Sideglance to use it.</p>
    <button onclick={() => void command('restart_app')}>Restart Sideglance</button>
  {:else}
    <button
      onclick={() => void checkForUpdates()}
      disabled={state.status === 'checking' || !state.currentVersion}
    >
      {state.status === 'checking' ? 'Checking…' : 'Check for updates'}
    </button>
    {#if state.status === 'up-to-date'}<p class="status">Sideglance is up to date.</p>{/if}
    {#if state.status === 'error'}<p class="error">{state.message}</p>{/if}
  {/if}
</section>

<style>
  pre {
    white-space: pre-wrap;
    font: inherit;
    color: var(--muted);
    margin: 0;
  }
</style>
