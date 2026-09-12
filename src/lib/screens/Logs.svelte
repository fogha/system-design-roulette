<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { api, onEvent, type ExecutionRun, type ExecutionLogLine } from '../ipc';
  import { app } from '../stores.svelte';
  import { RefreshCw, Radio, CircleCheck, CircleX, Loader, ScrollText } from 'lucide-svelte';

  let runs = $state<ExecutionRun[]>([]);
  let selected = $state<string | 'live'>('live');
  let lines = $state<ExecutionLogLine[]>([]);
  let loading = $state(false);
  let error = $state('');
  let follow = $state(true);
  let console_: HTMLDivElement | undefined = $state();

  async function loadRuns() {
    try { runs = await api.listExecutionRuns(); } catch (e) { error = String(e); }
  }
  async function loadLines() {
    loading = true;
    try {
      lines = selected === 'live' ? await api.getRecentExecutionLog(400) : await api.getExecutionLog(selected);
      error = '';
      if (follow) { await tick(); console_?.scrollTo({ top: console_.scrollHeight }); }
    } catch (e) { error = String(e); } finally { loading = false; }
  }
  onMount(() => {
    void loadRuns(); void loadLines();
    const subscriptions = [
      onEvent('gen:log', () => { if (selected === 'live' || runs[0]?.outcome === 'running') void loadLines(); }),
      onEvent('preparation:state', () => { void loadRuns(); void loadLines(); }),
    ];
    return () => { void Promise.all(subscriptions).then((offs) => offs.forEach((off) => off())); };
  });
  function pick(id: string | 'live') { selected = id; follow = id === 'live'; void loadLines(); }
  function clock(value: string) { const d = new Date(value); return Number.isNaN(d.getTime()) ? value : d.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit', second: '2-digit' }); }
  function day(value: string) { const d = new Date(value); return Number.isNaN(d.getTime()) ? '' : d.toLocaleDateString(undefined, { weekday: 'short', month: 'short', day: 'numeric' }); }
  function tone(line: string) {
    const l = line.toLowerCase();
    if (l.includes('failed') || l.includes('error') || l.includes('did not pass') || l.includes('refused')) return 'bad';
    if (l.includes('finished') || l.includes('ready') || l.includes('published') || l.includes('passed')) return 'ok';
    if (l.includes('correct') || l.includes('audit') || l.includes('retry')) return 'warn';
    return '';
  }
  const current = $derived(selected === 'live' ? null : runs.find((r) => r.run_id === selected) ?? null);
</script>

<div class="logs-page page-frame">
  <header>
    <div class="meta-label">EXECUTION LOG · WHAT THE TUTOR DID</div>
    <h1>Logs</h1>
    <p>Every line the runner reported while preparing a lesson, kept for thirty days so a slow or failed preparation can be read back.</p>
  </header>
  <div class="layout">
    <aside class="runs" aria-label="Preparation runs">
      <button class="run" class:on={selected === 'live'} onclick={() => pick('live')}>
        <span class="run-head"><Radio size={13} /> Live tail</span>
        <small>{app.preparingClass ? `Preparing ${app.preparingClass.label}…` : 'Latest lines from any run'}</small>
      </button>
      <div class="runs-head"><span class="mono">RUNS · {runs.length}</span><button class="icon" aria-label="Refresh runs" onclick={() => { void loadRuns(); void loadLines(); }}><RefreshCw size={12} /></button></div>
      <div class="run-list">
        {#each runs as run (run.run_id)}
          <button class="run" class:on={selected === run.run_id} onclick={() => pick(run.run_id)}>
            <span class="run-head">
              {#if run.outcome === 'ready'}<CircleCheck size={13} class="ok" />{:else if run.outcome === 'failed'}<CircleX size={13} class="bad" />{:else if run.outcome === 'running'}<Loader size={13} />{:else}<ScrollText size={13} />{/if}
              {run.label}
            </span>
            <small>{day(run.started_at)} · {clock(run.started_at)} → {clock(run.last_at)} · {run.lines} lines · <em class={`state-${run.outcome}`}>{run.outcome}</em></small>
          </button>
        {/each}
        {#if !runs.length}<p class="empty">No runs yet. The first lesson preparation appears here as it happens.</p>{/if}
      </div>
    </aside>
    <section class="console-wrap" aria-label="Log lines">
      <div class="console-head">
        <span class="mono">{selected === 'live' ? 'LIVE · LAST 400 LINES' : `RUN · ${current?.label ?? selected} · ${current?.outcome ?? ''}`}</span>
        <label class="follow"><input type="checkbox" bind:checked={follow} /> follow</label>
      </div>
      {#if current?.error}<p class="run-error"><strong>Outcome:</strong> {current.error}</p>{/if}
      <div class="console" bind:this={console_}>
        {#each lines as line (line.id)}
          <div class={`line ${tone(line.line)}`}><span class="at">{clock(line.at)}</span>{#if selected === 'live' && line.course_id}<span class="who">{line.course_id}</span>{/if}<span class="text">{line.line}</span></div>
        {/each}
        {#if !lines.length && !loading}<p class="empty">{selected === 'live' ? 'Nothing logged yet.' : 'This run has no lines.'}</p>{/if}
        {#if loading && !lines.length}<p class="empty">Reading…</p>{/if}
      </div>
      {#if error}<p class="run-error" role="alert">{error}</p>{/if}
    </section>
  </div>
</div>

<style>
  .logs-page { display: flex; flex-direction: column; flex: 1; min-height: 600px; gap: 14px; }
  header p { font-size: 13px; color: var(--muted); margin: 0; max-width: 60ch; } h1 { font-size: 28px; margin: 8px 0 6px; }
  .layout { display: grid; grid-template-columns: 300px minmax(0, 1fr); gap: 14px; flex: 1; min-height: 0; }
  .runs { display: flex; flex-direction: column; gap: 8px; min-height: 0; }
  .runs-head { display: flex; align-items: center; justify-content: space-between; padding: 6px 4px 0; } .runs-head .mono { font-size: 9px; letter-spacing: .7px; color: var(--faint); }
  .icon { display: grid; place-items: center; padding: 5px; border: 1px solid var(--node-border); border-radius: var(--radius-detail); background: var(--bg); color: var(--muted); cursor: pointer; }
  .run-list { display: flex; flex-direction: column; gap: 6px; overflow-y: auto; min-height: 0; }
  .run { display: grid; gap: 4px; text-align: left; padding: 10px 12px; border: 1px solid var(--node-border); border-radius: var(--radius-control); background: var(--surface); color: var(--fg); cursor: pointer; }
  .run.on { border-color: var(--violet); background: var(--surface-2); }
  .run-head { display: flex; align-items: center; gap: 7px; font-size: 12px; } .run small { font-size: 10px; color: var(--muted); line-height: 1.5; }
  .run em { font-style: normal; } .state-ready { color: var(--led-ok); } .state-failed { color: var(--led-err); } .state-running { color: var(--accent); }
  .run :global(.ok) { color: var(--led-ok); } .run :global(.bad) { color: var(--led-err); }
  .console-wrap { display: flex; flex-direction: column; min-height: 0; border: 1px solid var(--node-border); border-radius: var(--radius-panel); background: #0b0910; overflow: hidden; }
  .console-head { display: flex; align-items: center; justify-content: space-between; padding: 8px 12px; border-bottom: 1px solid var(--node-border); } .console-head .mono { font-size: 9px; letter-spacing: .7px; color: var(--faint); }
  .follow { display: flex; align-items: center; gap: 6px; font-size: 10px; color: var(--muted); }
  .console { flex: 1; min-height: 0; overflow: auto; padding: 10px 12px; font: 11px/1.7 var(--font-mono); color: var(--fg); }
  .line { display: grid; grid-template-columns: 70px auto minmax(0, 1fr); gap: 10px; white-space: pre-wrap; overflow-wrap: anywhere; }
  .line .at { color: var(--faint); } .line .who { color: var(--violet-fg); } .line .text { min-width: 0; }
  .line.ok .text { color: var(--led-ok); } .line.bad .text { color: var(--led-err); } .line.warn .text { color: var(--accent); }
  .run-error { margin: 0; padding: 8px 12px; font-size: 11px; color: var(--led-err); border-bottom: 1px solid var(--node-border); }
  .empty { color: var(--muted); font-size: 11px; padding: 10px 0; }
  @media (max-width: 860px) { .layout { grid-template-columns: 1fr; } .runs { max-height: 220px; } }
</style>
