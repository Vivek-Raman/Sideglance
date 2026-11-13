import { mount, unmount } from 'svelte';
import { mockIPC } from '@tauri-apps/api/mocks';
import UpdateSettings from '../src/settings/UpdateSettings.svelte';
let releaseCheck;
let releaseInstall;
const closed = [];
let delayed = false;
mockIPC((name, args) => {
  if (name === 'plugin:app|version') return '2.5.1';
  if (name === 'plugin:updater|check') {
    const metadata = { rid: 7, version: '2.6.0', currentVersion: '2.5.1', body: 'Test update' };
    if (delayed)
      return new Promise((resolve) => {
        releaseCheck = () => resolve(metadata);
      });
    return metadata;
  }
  if (name === 'plugin:resources|close') {
    closed.push(args.rid);
    return null;
  }
  if (name === 'plugin:updater|download_and_install')
    return new Promise((resolve) => {
      releaseInstall = resolve;
    });
  return null;
});
const component = mount(UpdateSettings, { target: document.body, props: { updatesEnabled: true } });
window.updaterFixture = {
  closed,
  delayCheck() {
    delayed = true;
  },
  resolveCheck() {
    releaseCheck();
  },
  resolveInstall() {
    releaseInstall();
  },
  remove() {
    return unmount(component);
  },
};
