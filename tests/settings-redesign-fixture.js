import { mount } from 'svelte';
import { mockIPC, mockWindows } from '@tauri-apps/api/mocks';
import App from '../src/App.svelte';
import '../src/styles.css';
mockWindows('main');
const settings = {
  theme: 'light',
  primaryColor: '#c98c5a',
  textScale: 1,
  pinned: true,
  projectSortOrder: 'project-directory-asc',
  statusSortOrder: 'submitted-at-desc',
  statusGrouping: 'project-directory',
  autoClearAfterMinutes: null,
  recentlyRemovedEnabled: false,
  maxStatusItemsPerProject: null,
  undoLimit: 3,
};
const installation = {
  installed: false,
  mcpRegistered: false,
  path: '/shared',
  targets: [
    { id: 'codex', name: 'Codex', path: '/codex/skill', installed: false, upToDate: false },
    { id: 'gemini', name: 'Gemini', path: '/gemini/skill', installed: false, upToDate: false },
  ],
  additionalTargets: [],
  agents: [
    {
      id: 'codex',
      name: 'Codex',
      skillInstalled: false,
      mcpRegistered: false,
      configurationValid: false,
      available: true,
      configPath: '/codex/config',
      error: null,
    },
  ],
  additionalAgents: [],
};
if (new URLSearchParams(location.search).has('modified')) {
  installation.targets[0].installed = true;
  installation.targets[0].updateStatus = 'modified';
  installation.agents[0].mcpRegistered = true;
  installation.agents[0].configurationValid = true;
}
window.calls = [];
window.installArgs = [];
mockIPC((name, args) => {
  window.calls.push(name);
  if (name === 'get_settings') return settings;
  if (name === 'get_skill_installation') return structuredClone(installation);
  if (name === 'install_skill_for_target') {
    window.installArgs.push(args);
    const target = installation.targets.find((item) => item.id === args.target);
    target.installed = true;
    target.upToDate = true;
    target.updateStatus = 'current';
    return structuredClone(installation);
  }
  if (name === 'install_mcp_for_harness') {
    installation.agents[0].mcpRegistered = true;
    installation.agents[0].configurationValid = true;
    return structuredClone(installation);
  }
  if (name === 'test_mcp_connection')
    return window.failConnection
      ? { status: 'error', message: 'Unavailable' }
      : { status: 'connected', message: 'Ready' };
  if (name === 'plugin:event|listen') return 1;
  return null;
});
mount(App, { target: document.body });
