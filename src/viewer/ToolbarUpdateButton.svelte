<script lang="ts">
  import './toolbar-button.css';
  import RefreshCw from '@lucide/svelte/icons/refresh-cw';
  import Download from '@lucide/svelte/icons/download';
  import Package from '@lucide/svelte/icons/package';
  import RotateCcw from '@lucide/svelte/icons/rotate-ccw';
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
  class="toolbar-button"
  class:ready={persistent}
  disabled={!updatesEnabled || busy}
  aria-label={label}
  aria-busy={busy}
  {title}
  onclick={activate}
>
  <span class="update-icon" class:spinning={busy}>
    {#if busy || stage === 'check'}
      <RefreshCw size={15} stroke-width={1.7} aria-hidden="true" />
    {:else if stage === 'download'}
      <Download size={15} stroke-width={1.7} aria-hidden="true" />
    {:else if stage === 'install'}
      <Package size={15} stroke-width={1.7} aria-hidden="true" />
    {:else}
      <RotateCcw size={15} stroke-width={1.7} aria-hidden="true" />
    {/if}
  </span>
</button>
<span class="announcement" role="status"
  >{message ||
    (stage === 'downloading' ? `Downloading update: ${progress}` : busy ? label : '')}</span
>

<style>
  .update-icon {
    display: inline-flex;
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
