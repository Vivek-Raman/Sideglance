import { mount } from 'svelte';
import { mockIPC, mockWindows } from '@tauri-apps/api/mocks';
import Viewer from '../src/viewer/Viewer.svelte';
import RetentionSettings from '../src/settings/RetentionSettings.svelte';
import AgentIntegrations from '../src/integrations/AgentIntegrations.svelte';
import '../src/styles.css';

const listeners = new Map();
const settings = {
  theme: 'light',
  primaryColor: '#c98c5a',
  textScale: 1,
  pinned: true,
  projectSortOrder: 'project-directory-asc',
  statusSortOrder: 'submitted-at-desc',
  statusGrouping: 'project-directory',
  autoClearAfterMinutes: null,
  maxStatusItemsPerProject: null,
  undoLimit: 3,
};
let revision = 1;
let entries = [];
let health = 'waiting';
let removed = [];
const at = new Date().toISOString();
const snapshot = () => ({
  revision: revision++,
  document: { version: 2, entries },
  health,
  error: health === 'stale' ? 'File unavailable' : null,
  path: '/fixture/status.json',
  modified_at: at,
  last_successful_read_at: at,
});
const calls = [];
mockWindows('main');
mockIPC((name, args) => {
  calls.push({ name, args });
  if (name === 'plugin:event|listen') {
    listeners.set(args.event, args.handler);
    return listeners.size;
  }
  if (name === 'get_settings') return settings;
  if (name === 'get_snapshot') return snapshot();
  if (name === 'get_recently_removed') return removed;
  if (name === 'restore_removed') {
    removed = removed.filter((x) => x.id !== args.id);
    return null;
  }
  if (name === 'preview_retention') return 2;
  if (name === 'get_skill_installation')
    return {
      installed: false,
      mcpRegistered: true,
      path: '/fixture/skill',
      targets: [],
      additionalTargets: [],
      additionalAgents: [],
      agents: [
        {
          id: 'codex',
          name: 'Codex',
          mcpRegistered: true,
          configurationValid: true,
          skillInstalled: false,
          available: true,
          configPath: '/fixture/config.toml',
          error: null,
        },
      ],
    };
  if (name === 'test_mcp_connection')
    return {
      status: 'connected',
      message: 'A new connection responded. Restart existing sessions after updating.',
    };
  return null;
});
window.fixture = {
  calls,
  set(healthValue, nextEntries) {
    health = healthValue;
    entries = nextEntries;
    window.__TAURI_INTERNALS__.runCallback(listeners.get('source-update'), { payload: snapshot() });
  },
  entries() {
    return [1, 2].map((i) => ({
      folder_name: 'project',
      session_id: `session-${i}`,
      branch: 'main',
      updated_at: at,
      points: [
        {
          id: `point-${i}`,
          text: `Point ${i}`,
          done: false,
          pinned: false,
          link: null,
          submitted_at: at,
        },
      ],
    }));
  },
  removed() {
    removed = [
      {
        id: 'removed-1',
        entry: this.entries()[0],
        point_index: 0,
        removed_at: at,
        reason: 'Time-based deletion',
      },
    ];
  },
};
const mode = new URLSearchParams(location.search).get('mode');
if (mode === 'retention')
  mount(RetentionSettings, {
    target: document.body,
    props: {
      settings,
      saving: false,
      onSaved: () => {},
      onError: console.error,
      onSortChange: () => {},
      onStatusSortChange: () => {},
    },
  });
else if (mode === 'integrations')
  mount(AgentIntegrations, { target: document.body, props: { onError: console.error } });
else mount(Viewer, { target: document.body });
