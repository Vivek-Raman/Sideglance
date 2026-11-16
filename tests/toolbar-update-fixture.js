import { mount, unmount } from 'svelte';
import { mockIPC } from '@tauri-apps/api/mocks';
import Toolbar from './toolbar-update-fixture.svelte';
import '../src/styles.css';
const calls = [];
const deferred = new Set();
const failures = new Set();
const pending = new Map();
let available = true;
const names = {
  check: 'plugin:updater|check',
  download: 'plugin:updater|download',
  install: 'plugin:updater|install',
  restart: 'restart_app',
};
mockIPC((name, args) => {
  calls.push({ name, rid: args?.rid, updateRid: args?.updateRid, bytesRid: args?.bytesRid });
  const kind = Object.keys(names).find((key) => names[key] === name);
  if (failures.delete(kind)) throw new Error(`${kind} unavailable`);
  const result =
    kind === 'check'
      ? available
        ? { rid: 11, version: '2.6.0', currentVersion: '2.5.1' }
        : null
      : kind === 'download'
        ? 22
        : null;
  if (kind === 'download') {
    args.onEvent.onmessage({ event: 'Started', data: { contentLength: 100 } });
    args.onEvent.onmessage({ event: 'Progress', data: { chunkLength: 45 } });
  }
  if (deferred.delete(kind))
    return new Promise((resolve) => pending.set(kind, () => resolve(result)));
  return result;
});
const component = mount(Toolbar, { target: document.body });
window.toolbarFixture = {
  calls,
  defer(kind) {
    deferred.add(kind);
  },
  resolve(kind) {
    pending.get(kind)();
  },
  fail(kind) {
    failures.add(kind);
  },
  noUpdate() {
    available = false;
  },
  remove() {
    return unmount(component);
  },
};
