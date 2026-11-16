import { mount } from 'svelte';
import { mockIPC, mockWindows } from '@tauri-apps/api/mocks';
import App from '../src/App.svelte';
import '../src/styles.css';

mockWindows('main');
mockIPC((command) => {
  if (command === 'get_settings')
    return {
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
  if (command === 'get_snapshot')
    return {
      revision: 1,
      document: { version: 2, entries: [] },
      health: 'waiting',
      path: null,
      error: null,
      modified_at: null,
    };
  if (command === 'get_skill_installation')
    return {
      installed: false,
      mcpRegistered: false,
      path: '/fixture/skills',
      targets: [],
      additionalTargets: [],
      agents: [],
      additionalAgents: [],
    };
  if (command === 'plugin:event|listen') return 1;
  return null;
});
mount(App, { target: document.body });
