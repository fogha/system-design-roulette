<script lang="ts">
  import { api } from '../ipc';
  import { app } from '../stores.svelte';
  import ClusterBar from '../components/ClusterBar.svelte';
  import RunnerSetup from '../features/runners/RunnerSetup.svelte';
  import SearchSetup from '../features/runners/SearchSetup.svelte';
  import RecoveryGuide from '../features/recovery/RecoveryGuide.svelte';
  import type { SearchSettingsView } from '../ipc';
  import { ArrowLeft, ArrowRight, Check, Rocket, X } from 'lucide-svelte';

  let model = $state('opus');
  let agent = $state('claude');
  let customBin = $state('');
  let phrase = $state('I am choosing to skip my training today and I accept the broken streak');
  let phrase2 = $state('');
  let submitting = $state(false);
  let error = $state('');
  let step = $state(0);

  const STEPS = [
    { title: 'Tutor', heading: 'Choose the tutor that writes your lessons', blurb: 'Pick a runner and the model it should use. You can keep a shortlist of models per provider and change any of this later, per class.' },
    { title: 'Search', heading: 'Give the tutor a way to look things up', blurb: 'Optional. A tutor on Ollama, OpenRouter or a bare API cannot browse; with a search engine set, the desk finds and fetches documentation itself and hands the tutor only pages it retrieved. Leave it off and lessons use the pages the curriculum names.' },
    { title: 'Recovery', heading: 'Set your break-glass phrase, and learn the way out', blurb: 'An enforced session holds the machine until the lesson is done. This phrase ends one early: it is deliberately long so it cannot be typed on reflex, and using it breaks your streak. Below it, the recovery console: a key combination that works even if the desk goes blank.' },
    { title: 'Deploy', heading: 'Review and deploy', blurb: 'This writes your preferences and opens the desk. Next you choose a class, set its starting point and add study times.' },
  ];
  const RECOVERY = 2;

  /** The search choice, read for the review; it is saved as it is made. */
  let search = $state<SearchSettingsView | null>(null);
  $effect(() => {
    if (step !== STEPS.length - 1) return;
    api.getSearchSettings().then((view) => (search = view)).catch(() => (search = null));
  });
  const SEARCH_LABELS: Record<string, string> = { none: 'Off', searxng: 'SearXNG', brave: 'Brave Search', tavily: 'Tavily' };

  const phraseReady = $derived(phrase.trim().length >= 40 && phrase.trim() === phrase2.trim());
  const phraseProblem = $derived(
    phrase.trim().length < 40
      ? `escape phrase must be at least 40 characters · ${phrase.trim().length}/40`
      : phrase.trim() !== phrase2.trim()
        ? 'phrases do not match'
        : '',
  );

  function canLeave(index: number) {
    return index === RECOVERY ? phraseReady : true;
  }
  function goto(next: number) {
    if (next > step && !canLeave(step)) {
      error = phraseProblem;
      return;
    }
    error = '';
    step = Math.max(0, Math.min(STEPS.length - 1, next));
  }

  async function finish() {
    error = '';
    if (!phraseReady) {
      step = RECOVERY;
      error = phraseProblem;
      return;
    }
    submitting = true;
    try {
      if (agent === 'custom') customBin = (await api.getRunnerConfiguration(agent)).custom_command;
      await api.completeSetup(phrase.trim(), 'advisory', model, agent, customBin);
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
      <p class="sub">Four short steps: your tutor, its search, your way out, then deploy.</p>
    </header>

    <ol class="steps" aria-label="Setup steps">
      {#each STEPS as item, index (item.title)}
        <li class:done={index < step} class:current={index === step}>
          <button
            type="button"
            aria-current={index === step ? 'step' : undefined}
            disabled={index > step}
            onclick={() => goto(index)}
          >
            <span class="chip mono">{#if index < step}<Check size={12} />{:else}{index + 1}{/if}</span>
            <span class="name">{item.title}</span>
          </button>
        </li>
      {/each}
    </ol>

    <section class="panel" aria-label={STEPS[step].title}>
      <h2>{STEPS[step].heading}</h2>
      <p class="sub">{STEPS[step].blurb}</p>

      {#if step === 0}
        <RunnerSetup bind:agent bind:model bind:customBin initiallyExpanded onKeyChanged={() => app.refresh()} />
      {:else if step === 1}
        <SearchSetup />
      {:else if step === RECOVERY}
        <div class="break-glass" class:ready={phraseReady}>
          <div class="bg-tag mono"><span class="bg-tag-line"></span>BREAK<br />GLASS<span class="bg-tag-line"></span></div>
          <div class="bg-fields">
            <label class="bg-label mono" for="escape-phrase">ESCAPE_PHRASE · circuit breaker · trips the streak to 0</label>
            <input id="escape-phrase" class="bg-input mono" type="text" bind:value={phrase} />
            <div class="bg-meter" role="progressbar" aria-label="Phrase length" aria-valuemin="0" aria-valuemax="40" aria-valuenow={Math.min(40, phrase.trim().length)}>
              <span class="bg-meter-fill" style={`width: ${Math.min(100, (phrase.trim().length / 40) * 100)}%`}></span>
              <span class="bg-meter-text mono">{phrase.trim().length} / 40 characters{phrase.trim().length >= 40 ? ' · long enough' : ' · keep going'}</span>
            </div>
            <input
              class="bg-input mono"
              type="text"
              aria-label="Repeat the escape phrase"
              placeholder="type it again to confirm"
              bind:value={phrase2}
            />
            <p class="bg-state mono" role="status">
              {phraseReady ? '✓ phrase confirmed' : phraseProblem}
            </p>
          </div>
        </div>
        <div class="recovery-guide">
          <div class="rg-head"><span class="meta-label">THE WAY OUT</span><h3>If the desk ever goes blank</h3></div>
          <RecoveryGuide {phrase} autoplay />
        </div>
      {:else}
        <dl class="review">
          <div><dt class="mono">TUTOR</dt><dd>{agent === 'custom' ? `Custom CLI · ${customBin || 'command set in the library'}` : agent}</dd></div>
          <div><dt class="mono">MODEL</dt><dd>{model || 'runner default'}</dd></div>
          <div><dt class="mono">SEARCH</dt><dd>{search ? `${SEARCH_LABELS[search.provider] ?? search.provider}${search.provider === 'none' ? '' : search.available ? ' · working' : ' · setup needed'}` : '…'}</dd></div>
          <div><dt class="mono">ESCAPE_PHRASE</dt><dd>{phrase.trim().length} characters · confirmed</dd></div>
          <div><dt class="mono">ENFORCEMENT</dt><dd>Advisory to start. Each class carries its own policy in its Settings tab.</dd></div>
        </dl>
      {/if}
    </section>

    {#if error}<p class="error mono"><X size={12} /> {error}</p>{/if}

    <div class="nav-row">
      <button class="ghost mono-ghost" disabled={step === 0 || submitting} onclick={() => goto(step - 1)}>
        <ArrowLeft size={13} /> Back
      </button>
      <span class="hint mono">Step {step + 1} of {STEPS.length}</span>
      {#if step < STEPS.length - 1}
        <button class="cta mono-cta" disabled={!canLeave(step)} onclick={() => goto(step + 1)}>
          Continue <ArrowRight size={13} />
        </button>
      {:else}
        <button class="cta mono-cta" onclick={finish} disabled={submitting}>
          {#if !submitting}<Rocket size={14} />{/if}{submitting ? '… deploying' : 'deploy to prod'}
        </button>
      {/if}
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
    width: min(1040px, 94vw);
    margin: 0 auto;
    padding: 34px 24px 64px;
  }
  .boot-head {
    margin-bottom: 22px;
  }
  h1 {
    font-size: 28px;
    margin-bottom: 6px;
  }
  h2 {
    font-size: 17px;
    margin: 0 0 6px;
  }
  .sub {
    color: var(--muted);
    font-size: 13px;
    margin: 0;
    max-width: 56ch;
  }

  /* step rail */
  .steps {
    display: flex;
    align-items: center;
    gap: 10px;
    list-style: none;
    margin: 0 0 18px;
    padding: 0;
  }
  .steps li {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  .steps li + li::before {
    content: '';
    width: 28px;
    border-top: 1.5px dashed var(--violet);
    opacity: 0.6;
  }
  .steps button {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 9px 5px 5px;
    border: 1px solid transparent;
    border-radius: var(--radius-control);
    background: transparent;
    color: var(--faint);
    font-size: 12px;
    cursor: pointer;
  }
  .steps button:disabled {
    cursor: default;
  }
  .steps .current button {
    border-color: var(--node-border);
    background: var(--surface);
    color: var(--fg);
  }
  .steps .done button {
    color: var(--muted);
  }
  .chip {
    display: grid;
    place-items: center;
    width: 24px;
    height: 24px;
    border: 1px solid var(--node-border);
    border-radius: var(--radius-control);
    background: var(--bg);
    font-size: 11px;
  }
  .steps .current .chip {
    border-color: var(--accent);
    color: var(--accent);
  }
  .steps .done .chip {
    border-color: var(--led-ok);
    color: var(--led-ok);
  }

  .panel {
    display: grid;
    gap: 14px;
    align-content: start;
    min-height: 260px;
  }
  .panel .sub {
    margin-bottom: 4px;
  }

  .break-glass {
    background: #1f1316;
    border: 1px dashed #793030;
    border-radius: var(--radius-panel);
    padding: 14px 16px;
    display: flex;
    gap: 16px;
    align-items: flex-start;
    transition: border-color 0.3s, background 0.3s;
  }
  .break-glass.ready { border-color: color-mix(in srgb, var(--ok-fg) 55%, #793030); background: #17191a; }
  .bg-tag {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    font-size: 10px;
    letter-spacing: 1.5px;
    color: var(--led-err);
    border: 1px solid #793030;
    border-radius: var(--radius-detail);
    padding: 8px 10px;
    text-align: center;
    line-height: 1.5;
    margin-top: 6px;
    background: repeating-linear-gradient(-45deg, transparent 0 6px, rgba(121, 48, 48, 0.18) 6px 8px);
  }
  .bg-tag-line { width: 100%; height: 1px; background: #793030; }
  .break-glass.ready .bg-tag { color: var(--ok-fg); border-color: color-mix(in srgb, var(--ok-fg) 50%, #793030); }
  .break-glass.ready .bg-tag-line { background: color-mix(in srgb, var(--ok-fg) 50%, #793030); }
  .bg-meter { position: relative; height: 18px; border-radius: 4px; background: var(--bg); overflow: hidden; border: 1px solid #3a2228; }
  .bg-meter-fill { position: absolute; inset: 0 auto 0 0; background: linear-gradient(90deg, #793030, color-mix(in srgb, var(--ok-fg) 70%, #793030)); transition: width 0.2s; }
  .bg-meter-text { position: relative; display: block; padding: 0 8px; font-size: 9.5px; line-height: 18px; color: #d8b0a8; letter-spacing: 0.4px; }
  .recovery-guide { display: flex; flex-direction: column; gap: 14px; margin-top: 18px; padding: 16px; border: 1px solid var(--node-border); border-radius: var(--radius-panel); background: var(--node-bg); }
  .rg-head h3 { margin: 4px 0 0; font-size: 15px; }
  .bg-fields {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .bg-label {
    font-size: 10px;
    color: #a05050;
    letter-spacing: 0.5px;
  }
  .bg-input {
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
  .bg-state {
    font-size: 10px;
    color: #a05050;
    margin: 2px 0 0;
  }

  .review {
    display: grid;
    gap: 1px;
    margin: 0;
    border: 1px solid var(--node-border);
    border-radius: var(--radius-panel);
    overflow: hidden;
    background: var(--node-border);
  }
  .review > div {
    display: grid;
    grid-template-columns: 140px 1fr;
    gap: 12px;
    padding: 11px 13px;
    background: var(--surface);
  }
  .review dt {
    font-size: 10px;
    color: var(--faint);
    letter-spacing: 0.6px;
  }
  .review dd {
    margin: 0;
    font-size: 12px;
    color: var(--fg);
    overflow-wrap: anywhere;
  }

  .error {
    color: var(--bad-fg);
    font-size: 12px;
    margin: 14px 0 0;
  }
  .nav-row {
    margin-top: 22px;
    display: flex;
    align-items: center;
    gap: 16px;
  }
  .nav-row .cta {
    margin-left: auto;
  }
  .hint {
    font-size: 10px;
    color: var(--faint);
  }

  @media (max-width: 560px) {
    .steps li + li::before {
      width: 12px;
    }
    .steps .name {
      display: none;
    }
    .review > div {
      grid-template-columns: 1fr;
      gap: 4px;
    }
  }
</style>
