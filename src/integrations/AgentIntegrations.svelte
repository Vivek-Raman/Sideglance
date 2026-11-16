<script lang="ts">
  import { onMount } from 'svelte';
  import { command } from '../shared/commands';
  import type { SkillInstallation } from '../shared/types';
  import SkillTargetRow from './SkillTargetRow.svelte';
  import McpAgentRow from './McpAgentRow.svelte';
  export let onError: (error: unknown) => void;
  let skill: SkillInstallation | null = null;
  let installingSkill: string | null = null;
  let installingMcp: string | null = null;
  let testingMcp: string | null = null;
  let testResults: Record<string, string> = {};
  let disposed = false;
  async function testConnection(harness: string) {
    if (testingMcp || installingMcp) return;
    testingMcp = harness;
    try {
      const result = await command('test_mcp_connection', { harness });
      if (!disposed)
        testResults = {
          ...testResults,
          [harness]: `${result.status === 'connected' ? 'Connected' : 'Connection needs attention'}: ${result.message}`,
        };
    } catch (error) {
      if (!disposed) onError(error);
    } finally {
      if (!disposed) testingMcp = null;
    }
  }

  async function installSkill(target: string) {
    installingSkill = target;
    try {
      const value = await command('install_skill_for_target', { target });
      if (!disposed) skill = value;
    } catch (error) {
      if (!disposed) onError(error);
    } finally {
      installingSkill = null;
    }
  }
  async function toggleMcp(harness: string, registered: boolean) {
    installingMcp = harness;
    try {
      const value = await command(
        registered ? 'uninstall_mcp_for_harness' : 'install_mcp_for_harness',
        {
          harness,
        },
      );
      if (!disposed) {
        skill = value;
        testResults = {
          ...testResults,
          [harness]: 'Configuration updated. Restart existing agent sessions to load it.',
        };
      }
    } catch (error) {
      if (!disposed) onError(error);
    } finally {
      installingMcp = null;
    }
  }
  onMount(() => {
    void command('get_skill_installation')
      .then((value) => {
        if (!disposed) skill = value;
      })
      .catch((error) => {
        if (!disposed) onError(error);
      });
    return () => {
      disposed = true;
    };
  });
</script>

<section class="integration">
  <div>
    <h2>Agent integration</h2>
    <p>
      Install the bundled skill into the locations used by each agent, then configure MCP separately
      when needed. After installing or updating, restart existing agent sessions. Test connection
      opens a new read-only connection; it does not reload those sessions.
    </p>
  </div>
  {#if skill}<div class="skill-targets" aria-label="Skill installation targets">
      {#each skill.targets as target}<SkillTargetRow
          {target}
          busy={installingSkill !== null}
          onInstall={installSkill}
        />{/each}
    </div>
    {#if skill.additionalTargets.length > 0}<details class="additional">
        <summary>Additional skill directories ({skill.additionalTargets.length})</summary>
        <div class="skill-targets">
          {#each skill.additionalTargets as target}<SkillTargetRow
              {target}
              busy={installingSkill !== null}
              onInstall={installSkill}
            />{/each}
        </div>
      </details>{/if}
    <div class="agent-status" aria-label="Agent integration status">
      {#each skill.agents as agent}<McpAgentRow
          {agent}
          busy={installingMcp !== null || testingMcp !== null}
          onTest={(id) => void testConnection(id)}
          onRepair={(id) => void toggleMcp(id, false)}
          testResult={testResults[agent.id] ?? ''}
          onToggle={(id, registered) => void toggleMcp(id, registered)}
        />{/each}
    </div>
    {#if skill.additionalAgents.length > 0}<details class="additional">
        <summary>Additional MCP configs ({skill.additionalAgents.length})</summary>
        <div class="agent-status">
          {#each skill.additionalAgents as agent}<McpAgentRow
              {agent}
              busy={installingMcp !== null || testingMcp !== null}
              onTest={(id) => void testConnection(id)}
              onRepair={(id) => void toggleMcp(id, false)}
              testResult={testResults[agent.id] ?? ''}
              onToggle={(id, registered) => void toggleMcp(id, registered)}
            />{/each}
        </div>
      </details>{/if}
    {#if skill.agents.length === 0}<p class="status">
        No supported agent harnesses were found.
      </p>{/if}
  {/if}
</section>

<style>
  .integration {
    display: grid;
    gap: 1rem;
    margin: 0 0 24px;
    padding: 1.1rem;
    border: 1px solid var(--border);
    border-radius: 0.65rem;
  }
  .integration h2 {
    margin: 0;
    font-size: 1rem;
  }
  .integration p {
    color: var(--muted);
    margin: 0.4rem 0 0;
  }
  .skill-targets,
  .agent-status {
    border: 1px solid var(--border);
    border-radius: 0.4rem;
    overflow: hidden;
  }
  .additional {
    display: grid;
    gap: 0.45rem;
  }
  .skill-targets :global(.skill-target + .skill-target),
  .agent-status :global(.agent + .agent) {
    border-top: 1px solid var(--border);
  }
  .status {
    color: var(--muted);
  }
</style>
