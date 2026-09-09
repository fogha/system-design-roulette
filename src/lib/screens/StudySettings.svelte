<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '../ipc';
  import { app } from '../stores.svelte';
  import NodeCard from '../components/NodeCard.svelte';
  import FlowStage from '../components/FlowStage.svelte';
  import { Zap, Cpu, Lock, ShieldAlert } from 'lucide-svelte';
  import AgentPicker from '../components/AgentPicker.svelte';
  import ModelPicker from '../components/ModelPicker.svelte';
  import EnforcementPicker from '../components/EnforcementPicker.svelte';
  let agent = $state('claude');
  let customBin = $state('');
  let model = $state('opus');
  let policy = $state('hard');
  let saving = $state('');
  let saved = $state('');
  onMount(() => { agent = app.state?.agent ?? 'claude'; customBin = app.state?.custom_agent_bin ?? ''; model = app.state?.model ?? 'opus'; policy = app.state?.kiosk_level ?? 'hard'; });
  async function save(section: 'provider' | 'model' | 'focus') {
    if (saving) return;
    saving = section; saved = '';
    try {
      if (section === 'provider') await api.setAgent(agent, customBin);
      if (section === 'model') await api.setModel(model);
      if (section === 'focus') await api.setKioskLevel(policy);
      await app.refresh(); saved = section;
    } catch (error) { app.error = String(error); }
    finally { saving = ''; }
  }
</script>

<div class="settings-page">
  <header><div class="meta-label">CONFIGURATION — TUTOR · FOCUS · RECOVERY</div><h1>Configure your desk</h1><p>Each service has its own controls. Class-specific tutor preferences live with the class.</p></header>
  <FlowStage number="01"><section aria-label="Default tutor"><NodeCard Icon={Zap} name="agent-backend" badge={app.state?.agent ?? 'claude'} badgeTone="violet">
    <div class="meta-label">DEFAULT TUTOR — DAILY STUDY</div>
    <AgentPicker bind:agent bind:customBin deepseekKeyConfigured={app.state?.deepseek_key_configured ?? false} onKeyChanged={() => app.refresh()} />
    <button class="cta mono-cta" disabled={!!saving} onclick={() => save('provider')}>{saving === 'provider' ? 'Saving…' : saved === 'provider' ? 'Provider saved' : 'Save provider'}</button>
  </NodeCard></section></FlowStage>
  {#if app.state?.agent === 'claude'}<FlowStage number="02"><section aria-label="Default model"><NodeCard Icon={Cpu} name="course-model" badge={app.state?.model ?? 'opus'} badgeTone="amber">
    <div class="meta-label">MODEL — WHO WRITES YOUR LESSONS</div><ModelPicker bind:value={model} /><button class="cta mono-cta" disabled={!!saving} onclick={() => save('model')}>{saving === 'model' ? 'Saving…' : saved === 'model' ? 'Model saved' : 'Save model'}</button>
  </NodeCard></section></FlowStage>{/if}
  <FlowStage number={app.state?.agent === 'claude' ? '03' : '02'}><section aria-label="Daily study focus"><NodeCard Icon={Lock} name="enforcement-service" badge={app.state?.kiosk_level ?? 'hard'} badgeTone="violet" accent="var(--violet)">
    <div class="meta-label">DAILY STUDY — FOCUS POLICY</div><EnforcementPicker bind:value={policy} /><button class="cta mono-cta" disabled={!!saving} onclick={() => save('focus')}>{saving === 'focus' ? 'Saving…' : saved === 'focus' ? 'Focus preference saved' : 'Save focus preference'}</button>
  </NodeCard></section></FlowStage>
  <FlowStage number={app.state?.agent === 'claude' ? '04' : '03'} last><section aria-label="Recovery"><NodeCard Icon={ShieldAlert} name="break-glass" badge={app.state?.enforcement_disarmed ? 'disarmed' : 'standby'} badgeTone="red">
    <div class="recovery"><span class="recovery-tag mono">RECOVERY</span><div><p>Your emergency phrase and <code>~/sdr-unlock</code> recovery file remain available during enforced study.</p><p class="mono">{app.state?.enforcement_disarmed ? 'Enforcement disarmed · recovery file present' : 'Recovery file absent · configured policy applies'}</p></div></div>
  </NodeCard></section></FlowStage>
</div>
<style>
  .settings-page { padding: 34px 24px 48px; width: min(780px, 100%); margin: 0 auto; }
  header { margin-bottom: 26px; }
  h1 { font-size: 28px; margin: 8px 0 6px; }
  p { font-size: 13px; color: var(--muted); margin: 0; }
  header p { max-width: 56ch; }
  button.cta { margin-top: 18px; }
  .recovery { display: flex; align-items: flex-start; gap: 14px; border: 1px dashed #793030; border-radius: 6px; padding: 12px; background: #1f1316; }
  .recovery-tag { border: 1px solid #793030; border-radius: 4px; font-size: 9px; color: var(--led-err); padding: 5px 7px; margin-top: 4px; }
  .recovery p + p { font-size: 10px; margin-top: 9px; }
  @media (max-width: 620px) { .settings-page { padding: 26px 16px; } .recovery { flex-direction: column; gap: 8px; } }
</style>
