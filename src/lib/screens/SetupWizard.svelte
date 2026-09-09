<script lang="ts">
  import { api } from '../ipc';
  import { app } from '../stores.svelte';
  import ClusterBar from '../components/ClusterBar.svelte';
  import NodeCard from '../components/NodeCard.svelte';
  import TimePicker from '../components/TimePicker.svelte';
  import EnforcementPicker from '../components/EnforcementPicker.svelte';
  import RunnerSetup from '../features/runners/RunnerSetup.svelte';
  import { Clock, Lock, Rocket, X } from 'lucide-svelte';

  let time = $state('19:00');
  let kioskLevel = $state('hard');
  let model = $state('opus');
  let agent = $state('claude');
  let customBin = $state('');
  let phrase = $state('I am choosing to skip my training today and I accept the broken streak');
  let phrase2 = $state('');
  let submitting = $state(false);
  let error = $state('');

  async function finish() {
    error = '';
    if (phrase.trim().length < 40) {
      error = 'escape phrase must be at least 40 characters';
      return;
    }
    if (phrase.trim() !== phrase2.trim()) {
      error = 'phrases do not match';
      return;
    }
    const [h, m] = time.split(':').map(Number);
    submitting = true;
    try {
      if (agent === 'custom') customBin = (await api.getRunnerConfiguration(agent)).custom_command;
      await api.completeSetup(h, m, phrase.trim(), kioskLevel, model, agent, customBin);
      await app.refresh();
    } catch (e) {
      error = String(e);
    } finally {
      submitting = false;
    }
  }
</script>

<div class="boot blueprint">
  <ClusterBar route="bootstrap" status="awaiting deploy" tone="warn" />
  <div class="boot-body">
    <header class="boot-head">
      <h1>Bootstrap your training cluster</h1>
      <p class="sub">
        Configure the shared teacher credentials and primary enforcement service, then use
        Classroom to enable, schedule, and immediately start each subject.
      </p>
    </header>

    <div class="flow">
      <!-- 01 · scheduler -->
      <section class="stage">
        <span class="step mono">01</span>
        <NodeCard Icon={Clock} name="cron-scheduler" badge=":launchd" badgeTone="amber">
          {#snippet children()}
            <div class="meta-label">FIRE_AT — daily trigger</div>
            <div class="sched">
              <TimePicker bind:value={time} />
              <ul class="sched-info mono">
                <li><span class="k">trigger</span> daily · local time</li>
                <li><span class="k">asleep</span> fires on next wake</li>
                <li><span class="k">powered off</span> fires at next login</li>
              </ul>
            </div>
          {/snippet}
        </NodeCard>
      </section>

      <div class="pipe" aria-hidden="true"></div>

      <!-- 02 · agent -->
      <section class="stage">
        <span class="step mono">02</span>
        <RunnerSetup bind:agent bind:model bind:customBin initiallyExpanded onKeyChanged={() => app.refresh()} />
      </section>

        <div class="pipe" aria-hidden="true"></div>

      <!-- enforcement -->
      <section class="stage">
        <span class="step mono">03</span>
        <NodeCard
          Icon={Lock}
          name="enforcement-service"
          badge={kioskLevel === 'hard' ? 'kiosk · no mercy' : kioskLevel === 'firm' ? 'kiosk · level 1000' : 'advisory'}
          badgeTone={kioskLevel === 'hard' ? 'red' : kioskLevel === 'firm' ? 'violet' : 'teal'}
          accent="var(--violet)"
        >
          {#snippet children()}
            <div class="meta-label">SLO — daily session completion · pick your strictness</div>
            <EnforcementPicker bind:value={kioskLevel} />
          {/snippet}
        </NodeCard>
      </section>

      <div class="pipe" aria-hidden="true"></div>

      <!-- escape phrase -->
      <section class="stage">
        <span class="step mono">04</span>
        <div class="break-glass">
          <div class="bg-tag">BREAK<br />GLASS</div>
          <div class="bg-fields">
            <div class="bg-label">ESCAPE_PHRASE — circuit breaker · trips streak to 0 · min 40 chars</div>
            <input class="bg-input mono" type="text" bind:value={phrase} />
            <input
              class="bg-input mono"
              type="text"
              placeholder="type it again to confirm"
              bind:value={phrase2}
            />
          </div>
        </div>
      </section>
    </div>

    {#if error}<p class="error mono"><X size={12} /> {error}</p>{/if}

    <div class="deploy-row">
      <button class="cta mono-cta" onclick={finish} disabled={submitting}>
        {#if !submitting}<Rocket size={14} />{/if}{submitting ? '… deploying' : 'deploy to prod'}
      </button>
      <span class="hint">
        writes launchd plist · pre-generates day-1 course<br />
        first session: today at {time} (or now, if {time} already passed)
      </span>
    </div>
  </div>
</div>

<style>
  .boot {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
    animation: fade-in 0.35s ease;
  }
  .boot-body {
    width: min(680px, 92vw);
    margin: 0 auto;
    padding: 34px 24px 64px;
  }
  .boot-head {
    margin-bottom: 26px;
  }
  h1 {
    font-size: 28px;
    margin-bottom: 6px;
  }
  .sub {
    color: var(--muted);
    font-size: 13px;
    margin: 0;
    max-width: 52ch;
  }

  /* vertical service flow */
  .flow {
    display: flex;
    flex-direction: column;
  }
  .stage {
    position: relative;
    padding-left: 38px;
  }
  /* step number rail */
  .step {
    position: absolute;
    left: 0;
    top: 10px;
    width: 24px;
    text-align: center;
    font-size: 11px;
    color: var(--faint);
    border: 1px solid var(--node-border);
    border-radius: var(--radius-control);
    padding: 3px 0;
    background: var(--bg);
  }
  /* dashed connector between stages, aligned over the step rail */
  .pipe {
    width: 0;
    height: 22px;
    margin-left: 11px;
    border-left: 1.5px dashed var(--violet);
    opacity: 0.7;
  }

  .sched {
    display: flex;
    align-items: center;
    gap: 20px;
    flex-wrap: wrap;
    margin-top: 8px;
  }
  .sched-info {
    list-style: none;
    margin: 0;
    padding: 0 0 0 18px;
    border-left: 1px dashed var(--node-divider);
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 10.5px;
    color: var(--faint);
    line-height: 1.4;
  }
  .sched-info .k {
    display: inline-block;
    min-width: 84px;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }
  .hint {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--faint);
    line-height: 1.6;
  }
  .break-glass {
    background: #1f1316;
    border: 1px dashed #793030;
    border-radius: var(--radius-panel);
    padding: 12px 14px;
    display: flex;
    gap: 14px;
    align-items: flex-start;
  }
  .bg-tag {
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--led-err);
    border: 1px solid #793030;
    border-radius: var(--radius-detail);
    padding: 6px 8px;
    text-align: center;
    line-height: 1.5;
    margin-top: 14px;
  }
  .bg-fields {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .bg-label {
    font-family: var(--font-mono);
    font-size: 10px;
    color: #a05050;
    letter-spacing: 0.5px;
  }
  .bg-input {
    font-family: var(--font-mono);
    font-size: 12px;
    color: #d8b0a8;
    background: var(--bg);
    border: 1px solid #3a2228;
    border-radius: var(--radius-control);
    padding: 8px 12px;
  }
  .bg-input:focus {
    border-color: #793030;
  }
  .error {
    color: var(--bad-fg);
    font-size: 12px;
    margin: 14px 0 0;
  }
  .deploy-row {
    margin-top: 22px;
    display: flex;
    align-items: center;
    gap: 16px;
  }
</style>
