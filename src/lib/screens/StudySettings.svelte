<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '../ipc';
  import { app } from '../stores.svelte';
  import NodeCard from '../components/NodeCard.svelte';
  import FlowStage from '../components/FlowStage.svelte';
  import { ShieldAlert } from 'lucide-svelte';
  import RunnerSetup from '../features/runners/RunnerSetup.svelte';
  import RunnerFallback from '../features/runners/RunnerFallback.svelte';
  import SearchSetup from '../features/runners/SearchSetup.svelte';
  let agent = $state('claude');
  let customBin = $state('');
  let model = $state('opus');
  onMount(() => { agent = app.state?.agent ?? 'claude'; customBin = app.state?.custom_agent_bin ?? ''; model = app.state?.model ?? 'opus'; });
  async function saveTutor() { await api.selectRunner(agent, model, customBin); await app.refresh(); }

</script>

<div class="settings-page page-frame">
  <header><div class="meta-label">CONFIGURATION — TUTOR · SEARCH · RECOVERY</div><h1>Configure your desk</h1><p>Each service has its own controls. Class-specific tutor preferences live with the class.</p></header>
  <FlowStage number="01"><section aria-label="Tutor configuration">
    <RunnerSetup bind:agent bind:model bind:customBin onUse={saveTutor} onKeyChanged={() => app.refresh()} />
    <RunnerFallback />
  </section></FlowStage>
  <FlowStage number="02"><section aria-label="Web search"><SearchSetup /></section></FlowStage>
  <FlowStage number="03" last><section aria-label="Recovery"><NodeCard Icon={ShieldAlert} name="break-glass" badge={app.state?.enforcement_disarmed ? 'disarmed' : 'standby'} badgeTone="red">
    <div class="recovery"><span class="recovery-tag mono">RECOVERY</span><div><p>Your emergency phrase and <code>principia-unlock</code> release token remain available during enforced study.</p><p class="mono">{app.state?.enforcement_disarmed ? 'Enforcement disarmed · recovery file present' : 'Recovery file absent'}</p></div></div>
  </NodeCard></section></FlowStage>
</div>
<style>
  .settings-page { padding-bottom: 48px; }
  header { margin-bottom: 26px; }
  h1 { font-size: 28px; margin: 8px 0 6px; }
  p { font-size: 13px; color: var(--muted); margin: 0; }
  header p { max-width: 56ch; }
  .recovery { display: flex; align-items: flex-start; gap: 14px; border: 1px dashed #793030; border-radius: var(--radius-control); padding: 12px; background: #1f1316; }
  .recovery-tag { border: 1px solid #793030; border-radius: var(--radius-detail); font-size: 9px; color: var(--led-err); padding: 5px 7px; margin-top: 4px; }
  .recovery p + p { font-size: 10px; margin-top: 9px; }
  @media (max-width: 620px) { .recovery { flex-direction: column; gap: 8px; } }
</style>
