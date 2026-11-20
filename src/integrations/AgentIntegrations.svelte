<script lang="ts">
  import { onMount } from 'svelte';
  import { command } from '../shared/commands';
  import type { AgentInstallation, SkillInstallation, SkillTarget } from '../shared/types';
  import SkillTargetRow from './SkillTargetRow.svelte';
  export let onError: (error: unknown) => void;
  let skill: SkillInstallation | null = null;
  let busy = '';
  let progress = '';
  let results: Record<string, string> = {};
  let errors: Record<string, string> = {};
  let disposed = false;
  let loadError = '';
  let failedTests: Record<string, boolean> = {};
  const message = (error: unknown) => (error instanceof Error ? error.message : String(error));
  function targetFor(id: string) {
    return [...(skill?.targets ?? []), ...(skill?.additionalTargets ?? [])].find(
      (target) => target.id === id,
    );
  }
  function agentFor(id: string) {
    return [...(skill?.agents ?? []), ...(skill?.additionalAgents ?? [])].find(
      (agent) => agent.id === id,
    );
  }
  function action(target?: SkillTarget, agent?: AgentInstallation, failed = false) {
    if (failed || agent?.error || (agent?.mcpRegistered && !agent.configurationValid))
      return 'Fix setup';
    if ((target && !target.installed) || (agent && !agent.mcpRegistered)) return 'Set up';
    if (target && !target.upToDate) return 'Update';
    return '';
  }
  async function setup(id: string, force: 'skill' | 'mcp' | null = null) {
    if (busy) return;
    busy = id;
    errors = { ...errors, [id]: '' };
    results = { ...results, [id]: '' };
    try {
      const target = targetFor(id);
      if (target && (force === 'skill' || !target.installed || !target.upToDate)) {
        progress = 'Installing skill…';
        const value = await command('install_skill_for_target', { target: id });
        if (disposed) return;
        skill = value;
      }
      const agent = agentFor(id);
      if (
        agent &&
        (force === 'mcp' || !agent.mcpRegistered || !agent.configurationValid || agent.error)
      ) {
        progress = 'Configuring MCP…';
        const value = await command('install_mcp_for_harness', { harness: id });
        if (disposed) return;
        skill = value;
      }
      if (agentFor(id)) {
        progress = 'Testing connection…';
        const result = await command('test_mcp_connection', { harness: id });
        if (disposed) return;
        failedTests = { ...failedTests, [id]: result.status !== 'connected' };
        results = {
          ...results,
          [id]: `${result.status === 'connected' ? 'Connection test passed' : 'Connection needs attention'}: ${result.message} Restart existing agent sessions to load changes.`,
        };
      } else {
        failedTests = { ...failedTests, [id]: false };
        results = {
          ...results,
          [id]: 'Skill installed. Restart existing agent sessions to load changes. MCP setup is not supported for this agent.',
        };
      }
    } catch (error) {
      if (!disposed) failedTests = { ...failedTests, [id]: true };
      if (!disposed)
        errors = {
          ...errors,
          [id]: `${message(error)}. Completed steps remain in place; retry setup.`,
        };
    } finally {
      if (!disposed) {
        busy = '';
        progress = '';
      }
    }
  }
  async function test(id: string) {
    if (busy) return;
    busy = id;
    progress = 'Testing connection…';
    errors = { ...errors, [id]: '' };
    try {
      const result = await command('test_mcp_connection', { harness: id });
      if (!disposed) failedTests = { ...failedTests, [id]: result.status !== 'connected' };
      if (!disposed)
        results = {
          ...results,
          [id]: `${result.status === 'connected' ? 'Connection test passed' : 'Connection needs attention'}: ${result.message}`,
        };
    } catch (error) {
      if (!disposed) failedTests = { ...failedTests, [id]: true };
      if (!disposed) errors = { ...errors, [id]: message(error) };
    } finally {
      if (!disposed) {
        busy = '';
        progress = '';
      }
    }
  }
  async function remove(id: string) {
    if (busy) return;
    busy = id;
    progress = 'Removing MCP…';
    errors = { ...errors, [id]: '' };
    try {
      const value = await command('uninstall_mcp_for_harness', { harness: id });
      if (!disposed) {
        skill = value;
        results = {
          ...results,
          [id]: 'MCP removed. Restart existing agent sessions to load changes.',
        };
      }
    } catch (error) {
      if (!disposed) errors = { ...errors, [id]: message(error) };
    } finally {
      if (!disposed) {
        busy = '';
        progress = '';
      }
    }
  }
  async function installExtra(id: string) {
    if (busy) return;
    busy = id;
    try {
      const value = await command('install_skill_for_target', { target: id });
      if (!disposed) skill = value;
    } catch (error) {
      if (!disposed) onError(error);
    } finally {
      if (!disposed) busy = '';
    }
  }
  async function load() {
    loadError = '';
    try {
      const value = await command('get_skill_installation');
      if (!disposed) skill = value;
    } catch (error) {
      if (!disposed) loadError = message(error);
    }
  }
  onMount(() => {
    void load();
    return () => {
      disposed = true;
    };
  });
  $: ids = skill
    ? [
        ...new Set([
          ...skill.agents.map((agent) => agent.id),
          ...skill.targets.filter((target) => target.id !== 'agents').map((target) => target.id),
        ]),
      ]
    : [];
  $: extraIds = skill
    ? [
        ...new Set([
          ...skill.additionalAgents.map((agent) => agent.id),
          ...skill.additionalTargets.map((target) => target.id),
        ]),
      ]
    : [];
</script>

{#snippet rows(ids: string[], installation: SkillInstallation)}
  {#each ids as id}
    {@const target = [...installation.targets, ...installation.additionalTargets].find(
      (item) => item.id === id,
    )}
    {@const agent = [...installation.agents, ...installation.additionalAgents].find(
      (item) => item.id === id,
    )}
    {@const mainAction = action(target, agent, !!failedTests[id])}
    <div class="agent-row">
      <div class="row-heading">
        <div>
          <strong>{agent?.name ?? target?.name}</strong>
          <p>
            {!target || !agent
              ? 'Additional setup required'
              : mainAction
                ? mainAction === 'Update'
                  ? 'Skill update available'
                  : mainAction === 'Fix setup'
                    ? 'Setup needs attention'
                    : 'Setup needed'
                : 'Configured'}
          </p>
        </div>
        {#if mainAction}<button disabled={!!busy} onclick={() => void setup(id)}
            >{busy === id ? progress : mainAction}</button
          >{/if}
      </div>
      {#if busy === id}<p role="status">{progress}</p>{/if}
      {#if errors[id]}<p class="error" role="alert">{errors[id]}</p>{/if}
      {#if results[id]}<p role="status">{results[id]}</p>{/if}
      <details>
        <summary>Details</summary>
        {#if target}<p>
            Skill: {target.installed
              ? target.upToDate
                ? 'Installed'
                : 'Update available'
              : 'Not installed'}<br /><small>{target.path}</small>
          </p>
          {#if target.installed}<button disabled={!!busy} onclick={() => void setup(id, 'skill')}
              >Reinstall skill</button
            >{/if}{:else}<p>Skill installation is not supported.</p>{/if}
        {#if agent}<p>
            MCP: {agent.configurationValid
              ? 'Configuration valid'
              : agent.mcpRegistered
                ? 'Needs repair'
                : 'Not configured'}<br /><small>{agent.configPath}</small>
          </p>
          {#if agent.error}<p class="error">{agent.error}</p>{/if}
          {#if agent.mcpRegistered}<div class="actions">
              <button disabled={!!busy} onclick={() => void setup(id, 'mcp')}>Repair MCP</button>
              <button disabled={!!busy} onclick={() => void test(id)}>Test connection</button
              ><button disabled={!!busy} onclick={() => void remove(id)}>Remove MCP</button>
            </div>{/if}
        {:else}<p>MCP setup is not supported.</p>{/if}
      </details>
    </div>
  {/each}
{/snippet}

<section class="integration">
  <p>
    Set up the available components for each agent. Restart existing agent sessions after changes. A
    connection test does not confirm that an agent has reported status.
  </p>
  {#if skill}
    {@render rows(ids, skill)}
    {#if extraIds.length}<details class="additional">
        <summary>Additional agent directories ({extraIds.length})</summary>{@render rows(
          extraIds,
          skill,
        )}
      </details>{/if}
    {#if skill.targets.find((target) => target.id === 'agents')}
      <details class="additional">
        <summary>Shared skill directory</summary>
        <SkillTargetRow
          target={skill.targets.find((target) => target.id === 'agents')!}
          busy={!!busy}
          onInstall={installExtra}
        />
        <p>Restart existing agent sessions after installing or updating the skill.</p>
      </details>
    {/if}
  {:else if loadError}<p class="error" role="alert">{loadError}</p>
    <button onclick={() => void load()}>Retry loading integrations</button>{:else}<p role="status">
      Loading integrations…
    </p>{/if}
</section>

<style>
  .integration {
    display: grid;
    gap: 0;
  }
  p {
    color: var(--muted);
    margin: 0.35rem 0;
    line-height: 1.45;
  }
  .integration > p {
    margin: 0 0 1rem;
  }
  .agent-row {
    padding: 0.9rem 0;
    border-bottom: 1px solid var(--border);
  }
  .row-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
  }
  .row-heading p,
  details,
  .agent-row > p {
    font-size: 0.85rem;
  }
  details {
    margin-top: 0.5rem;
  }
  summary {
    cursor: pointer;
    color: var(--accent-text, var(--text));
  }
  small {
    overflow-wrap: anywhere;
  }
  .actions {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
  }
  .additional {
    padding: 1rem 0;
    border-bottom: 1px solid var(--border);
  }
  .error {
    color: #c43d35;
    overflow-wrap: anywhere;
  }
  button {
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
    opacity: 0.65;
    cursor: wait;
  }
  button:focus-visible,
  summary:focus-visible {
    outline: 2px solid var(--focus-ring, var(--accent));
    outline-offset: 2px;
  }
</style>
