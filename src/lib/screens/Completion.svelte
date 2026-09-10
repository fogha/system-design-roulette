<script lang="ts">
  import { api } from '../ipc';
  import { app } from '../stores.svelte';
  // Capture once: async work must keep the session that opened this screen.
  const sessionId = app.session?.session_id ?? '';
  import ClusterBar from '../components/ClusterBar.svelte';
  import NodeCard from '../components/NodeCard.svelte';
  import { Check, ArrowUpRight, Languages } from 'lucide-svelte';

  const session = $derived(app.session);
  const skipped = $derived(session?.status === 'skipped');
  let resourcesOpened = $state(false);

  async function openResources() {
    const n = await api.openResources(sessionId).catch(() => 0);
    resourcesOpened = n > 0;
  }

</script>

<div class="done blueprint">
  <ClusterBar
    route={skipped ? 'postmortem' : 'deploy/complete'}
    status={skipped ? 'circuit breaker tripped' : 'session shipped'}
    tone={skipped ? 'err' : 'ok'}
  />
  <div class="done-body">
    {#if skipped}
      <div class="meta-label">POSTMORTEM — streak reset to 0</div>
      <h1>Circuit breaker tripped.</h1>
      <p class="sub">
        This saved session is marked skipped. Your class schedules remain available.
      </p>
      <div class="actions">
        <button class="ghost mono-ghost" onclick={() => (app.screen = 'dashboard')}>cluster overview</button>
        <button class="ghost mono-ghost" onclick={() => app.openClass()}>
          <Languages size={12} /> classes
        </button>
      </div>
    {:else}
      <div class="meta-label">DEPLOY COMPLETE — session shipped</div>
      <h1>Session complete.</h1>
      <div class="stats">
        <NodeCard name="error-budget" badge="quiz" badgeTone="muted">
          {#snippet children()}
            <div class="num mono">
              {session?.quiz_score != null ? Math.round(session.quiz_score * 100) + '%' : '—'}
            </div>
          {/snippet}
        </NodeCard>
        <NodeCard name="uptime" badge="streak" badgeTone="teal">
          {#snippet children()}
            <div class="num mono">{session?.streak ?? 0}d</div>
          {/snippet}
        </NodeCard>
      </div>
      <div class="actions">
        <button class="cta mono-cta" onclick={openResources}>
          {#if resourcesOpened}<Check size={13} />{:else}<ArrowUpRight size={13} />{/if}{resourcesOpened ? 'egress queue flushed' : 'open reading list in browser'}
        </button>
        <button class="ghost mono-ghost" onclick={() => (app.screen = 'dashboard')}>cluster overview</button>
        <button class="ghost mono-ghost" onclick={() => app.openClass()}>
          <Languages size={12} /> classes
        </button>
      </div>

    {/if}
  </div>
</div>

<style>
  .done {
    flex: 1;
    display: flex;
    flex-direction: column;
    animation: fade-in 0.35s ease;
  }
  .done-body {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 24px;
  }
  h1 {
    font-size: 36px;
    margin: 10px 0 22px;
  }
  .sub {
    color: var(--muted);
  }
  .stats {
    display: flex;
    gap: 14px;
    margin-bottom: 22px;
    min-width: 360px;
  }
  .stats :global(.node) {
    flex: 1;
  }
  .num {
    font-size: 26px;
    color: var(--fg);
    text-align: center;
  }
  .actions {
    display: flex;
    gap: 12px;
    align-items: center;
  }
</style>
