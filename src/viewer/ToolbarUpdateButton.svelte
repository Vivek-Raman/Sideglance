<script lang="ts">
  import { check, type Update } from '@tauri-apps/plugin-updater';
  import { onMount } from 'svelte';
  import { command } from '../shared/commands';
  import { startUpdateSchedule } from '../shared/update-schedule';

  export let updatesEnabled = !import.meta.env.DEV;
  export let automaticChecks = true;
  export let onPersistentChange: (persistent: boolean) => void = () => {};

  type Stage =
    'check' | 'checking' | 'download' | 'downloading' | 'install' | 'installing' | 'restart';
  let stage: Stage = 'check';
  let resource: Update | null = null;
  let disposed = false;
  let version = '';
  let downloaded = 0;
  let contentLength: number | null = null;
  let message = '';
  let restarting = false;
  $: busy = stage === 'checking' || stage === 'downloading' || stage === 'installing' || restarting;
  $: persistent = ['download', 'downloading', 'install', 'installing', 'restart'].includes(stage);
  $: onPersistentChange(persistent);
  $: label =
    stage === 'checking'
      ? 'Checking for updates'
      : stage === 'download'
        ? 'Download update'
        : stage === 'downloading'
          ? 'Downloading update'
          : stage === 'install'
            ? 'Install update'
            : stage === 'installing'
              ? 'Installing update'
              : stage === 'restart'
                ? 'Restart to finish updating'
                : 'Check for updates';
  $: progress = contentLength
    ? `${Math.min(100, Math.round((downloaded / contentLength) * 100))}%`
    : `${Math.round(downloaded / 1024)} KB`;
  $: title = !updatesEnabled
    ? 'Updates are disabled in development mode.'
    : message ||
      (stage === 'download'
        ? `Download Sideglance ${version}`
        : stage === 'install'
          ? `Install Sideglance ${version} and restart. Restart agent sessions after updating.`
          : stage === 'downloading'
            ? `Downloading Sideglance ${version}: ${progress}`
            : label);

  function errorText(error: unknown) {
    return error instanceof Error ? error.message : String(error);
  }
  async function close(update: Update) {
    await update.close().catch(() => undefined);
  }
  async function checkForUpdates() {
    if (disposed || !updatesEnabled || stage !== 'check') return;
    stage = 'checking';
    message = '';
    try {
      const update = await check({ timeout: 30_000 });
      if (disposed) {
        if (update) await close(update);
        return;
      }
      if (update) {
        resource = update;
        version = update.version;
        stage = 'download';
        message = `Sideglance ${version} is available. Download update.`;
      } else {
        stage = 'check';
        message = 'Sideglance is up to date. Check again.';
      }
    } catch (error) {
      if (!disposed) {
        stage = 'check';
        message = `Could not check for updates: ${errorText(error)}. Click to retry.`;
      }
    }
  }
  async function download() {
    if (!resource || stage !== 'download') return;
    const update = resource;
    resource = null;
    stage = 'downloading';
    message = '';
    downloaded = 0;
    contentLength = null;
    try {
      await update.download(
        (event) => {
          if (disposed) return;
          if (event.event === 'Started') {
            downloaded = 0;
            contentLength = event.data.contentLength ?? null;
          } else if (event.event === 'Progress') downloaded += event.data.chunkLength;
        },
        { timeout: 120_000 },
      );
      if (!disposed) {
        stage = 'install';
        message = `Sideglance ${version} is downloaded. Install update and restart.`;
      }
    } catch (error) {
      if (!disposed) {
        stage = 'download';
        message = `Download failed: ${errorText(error)}. Click to retry.`;
      }
    } finally {
      if (disposed) await close(update);
      else resource = update;
    }
  }
  async function restart() {
    if (disposed || restarting) return;
    restarting = true;
    try {
      await command('restart_app');
    } catch (error) {
      if (!disposed)
        message = `Update installed. Could not restart: ${errorText(error)}. Click to retry restart.`;
    } finally {
      if (!disposed) restarting = false;
    }
  }
  async function install() {
    if (!resource || stage !== 'install') return;
    const update = resource;
    resource = null;
    stage = 'installing';
    message = '';
    let installed = false;
    try {
      await update.install();
      installed = true;
    } catch (error) {
      if (!disposed) {
        stage = 'install';
        message = `Installation failed: ${errorText(error)}. Click to retry.`;
      }
    } finally {
      if (disposed || installed) await close(update);
      else resource = update;
    }
    if (installed && !disposed) {
      stage = 'restart';
      message = `Sideglance ${version} is installed. Restart to finish updating.`;
      await restart();
    }
  }
  function activate() {
    if (disposed || !updatesEnabled || busy) return;
    if (stage === 'check') void checkForUpdates();
    else if (stage === 'download') void download();
    else if (stage === 'install') void install();
    else if (stage === 'restart') void restart();
  }
  onMount(() => {
    const stop =
      updatesEnabled && automaticChecks
        ? startUpdateSchedule(checkForUpdates, undefined, { immediate: false })
        : () => {};
    return () => {
      disposed = true;
      stop();
      const update = resource;
      resource = null;
      if (update) void close(update);
      // In-flight operations retain their own resource and close it on completion.
    };
  });
</script>

<button
  class="update-button"
  class:ready={persistent}
  disabled={!updatesEnabled || busy}
  aria-label={label}
  aria-busy={busy}
  {title}
  onclick={activate}
>
  <svg
    viewBox="0 0 24 24"
    width="15"
    height="15"
    fill="none"
    stroke="currentColor"
    stroke-width="1.7"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
    class:spinning={busy}
  >
    {#if busy || stage === 'check'}
      <path d="M20 7a9 9 0 0 0-15-2L3 7m0-5v5h5M4 17a9 9 0 0 0 15 2l2-2m0 5v-5h-5" />
    {:else if stage === 'download'}
      <path d="M12 3v12m-5-5 5 5 5-5M4 16v5h16v-5" />
    {:else if stage === 'install'}
      <path d="m12 3 9 5v9l-9 5-9-5V8l9-5Zm-9 5 9 5 9-5M12 13v9M7.5 5.5l9 5" />
    {:else}
      <path d="M4 10a8 8 0 1 1 1 7M4 4v6h6" />
    {/if}
  </svg>
</button>
<span class="announcement" role="status"
  >{message ||
    (stage === 'downloading' ? `Downloading update: ${progress}` : busy ? label : '')}</span
>

<style>
  .update-button {
    display: grid;
    place-items: center;
    min-width: 1.8rem;
    height: 1.65rem;
    padding: 0.28rem 0.35rem;
    border: 1px solid transparent;
    border-radius: 999px;
    color: var(--text);
    background: transparent;
    cursor: pointer;
  }
  .update-button.ready {
    color: var(--accent);
    background: color-mix(in srgb, var(--bg) 88%, transparent);
    backdrop-filter: blur(28px) saturate(1.15);
    -webkit-backdrop-filter: blur(28px) saturate(1.15);
  }
  button:not(:disabled):hover {
    border-color: var(--border);
    color: var(--accent);
  }
  button:disabled {
    cursor: default;
    opacity: 0.65;
  }
  button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .spinning {
    animation: spin 1s linear infinite;
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .spinning {
      animation: none;
    }
  }
  .announcement {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }
</style>
