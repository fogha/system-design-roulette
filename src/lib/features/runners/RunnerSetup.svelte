<script lang="ts">
  import { onMount, untrack, tick } from 'svelte';
  import { Library, Zap, Settings2 } from 'lucide-svelte';
  import { api } from '$lib/ipc';
  import type { RunnerInfo, HealthCheck } from '$lib/contracts/agents';
  import NodeCard from '$lib/components/NodeCard.svelte';
  import Dropdown from '$lib/components/Dropdown.svelte';
  import ModelPicker from '$lib/components/ModelPicker.svelte';
  import RunnerLibrary from './RunnerLibrary.svelte';
  let { agent = $bindable('claude'), model = $bindable('opus'), customBin = $bindable(''), allowKeyEditing = true, onUse, onKeyChanged, onchange, initiallyExpanded = false, selectionHint = 'Applied when you save this setup.' }: {
    agent?: string; model?: string; customBin?: string; allowKeyEditing?: boolean; onUse?: () => Promise<void>; onKeyChanged?: () => void | Promise<void>; onchange?: () => void; initiallyExpanded?: boolean; selectionHint?: string;
  } = $props();
  let runners = $state<RunnerInfo[]>([]); let loading = $state(true); let expanded = $state(false); let editing = $state('claude');
  let libraryHeader: HTMLDivElement | undefined;
  let applied = $state({ agent: '', model: '', command: '' });
  const pending = $derived(!!onUse && !!applied.agent && (agent !== applied.agent || model !== applied.model || (agent === 'custom' && customBin !== applied.command)));
  let error = $state(''); let busy = $state(''); let result = $state<HealthCheck | null>(null); let saved = $state(false); let revision = $state(0);
  const picked = $derived(runners.find(r => r.provider === agent));
  const options = $derived([...runners].sort((a,b) => Number(b.available) - Number(a.available)).map(r => ({ value: r.provider, label: r.label, description: `${r.kind === 'local' ? 'Local' : r.kind.toUpperCase()} · ${r.available ? 'configured' : 'setup needed'}` })));
  $effect(() => { void agent; void model; void customBin; untrack(() => { result = null; saved = false; }); });
  async function refresh() { runners = await api.listAgentRunners(); revision++; }
  onMount(() => { expanded = initiallyExpanded; void (async () => { try { await refresh(); editing = agent; applied = { agent, model, command: customBin }; } catch (e) { error = String(e); } finally { loading = false; } })(); });
  async function configurationChanged() { await refresh(); if (editing === 'custom' && agent === 'custom') { customBin = (await api.getRunnerConfiguration('custom')).custom_command; onchange?.(); } await onKeyChanged?.(); }
  async function configure() { editing = agent; expanded = true; await tick(); libraryHeader?.scrollIntoView({ block: 'start', behavior: 'smooth' }); }
  async function choose(provider: string) {
    busy = 'selection'; error = '';
    try {
      const setup = await api.getRunnerConfiguration(provider);
      const runner = runners.find(r => r.provider === provider);
      agent = provider;
      model = runner?.saved_model && setup.models.includes(runner.saved_model) ? runner.saved_model : setup.models[0] || runner?.default_model || 'default';
      if (provider === 'custom') customBin = setup.custom_command;
      onchange?.();
    } catch (e) { error = String(e); } finally { busy = ''; }
  }
  async function commandForSelection() {
    if (agent !== 'custom') return customBin;
    return customBin || (await api.getRunnerConfiguration(agent)).custom_command;
  }
  async function test() {
    busy = 'test'; error = ''; result = null;
    const selection = [agent,model];
    try { const checked = await api.testAgentConnection(agent, await commandForSelection(), model); if (selection[0] === agent && selection[1] === model) result = checked; }
    catch (e) { error = String(e); } finally { busy = ''; }
  }
  async function use() {
    busy = 'save'; error = '';
    try { customBin = await commandForSelection(); await onUse?.(); applied = { agent, model, command: customBin }; await refresh(); saved = true; }
    catch (e) { error = String(e); } finally { busy = ''; }
  }
</script>

<div class="runner-workspace">
  <NodeCard Icon={Library} name="runner-library" badge="configuration" badgeTone="muted">
    <div class="library-summary" bind:this={libraryHeader}><div><strong>Runners &amp; models</strong><p>Set up providers and keep a shortlist of models for each.</p><span class="counts mono">5 CLI · 7 API · Ollama local</span></div><button type="button" class="manage mono" aria-expanded={expanded} onclick={() => expanded = !expanded}><Settings2 size={13} />{expanded ? 'Close library' : 'Configure'}</button></div>
    <div class="library-content" hidden={!expanded}>
      {#if loading}<p class="hint" role="status">Loading runner library…</p>{:else}<RunnerLibrary {runners} bind:selected={editing} {allowKeyEditing} onchanged={configurationChanged} />{/if}
    </div>
  </NodeCard>
  <NodeCard Icon={Zap} name="active-tutor" badge={picked?.label ?? agent} badgeTone="violet">
    <div class="active-head"><strong>Active tutor</strong><p>Choose the runner and saved model to use for study.</p></div>
    <div class="selection"><Dropdown label="Runner" value={agent} {options} disabled={loading || !!busy} onchange={choose} />{#key agent}<ModelPicker {agent} bind:value={model} {revision} disabled={!!busy} onconfigure={configure} {onchange} />{/key}</div>
    {#if agent === 'custom'}<p class="hint">Uses the command saved in the runner library.</p>{/if}
    {#if picked && !picked.available}<p class="hint">{picked.detail}</p>{/if}
    {#if pending}<p class="pending mono" role="status">Selection not applied yet</p>{/if}
    <div class="actions">{#if onUse}<button type="button" class="action mono" disabled={!!busy || loading} onclick={use}>{busy === 'save' ? 'Applying…' : saved ? 'Active tutor saved ✓' : 'Use for study'}</button>{:else}<span class="hint">{selectionHint}</span>{/if}<button type="button" class="secondary mono" disabled={!!busy || loading} onclick={test}>{busy === 'test' ? 'Testing…' : 'Test connection'}</button></div>
    {#if result}<div class="test-result" class:ok={result.ok} role="status"><span class="mono">{result.runner} · {result.model} · {(result.duration_ms/1000).toFixed(1)}s</span><p>{result.detail}</p></div>{/if}
    {#if error}<p class="error" role="alert">{error}</p>{/if}
  </NodeCard>
</div>

<style>
  .runner-workspace { display: grid; gap: 16px; }
  .pending { color: var(--accent); font-size: 9px; margin-top: 12px; }
  .library-summary { scroll-margin-top: 160px; display: flex; align-items: center; justify-content: space-between; gap: 14px; } strong { display: block; font-size: 14px; font-weight: 500; } p { margin: 5px 0 0; font-size: 11px; color: var(--muted); line-height: 1.6; }
  .counts { display: block; margin-top: 9px; color: var(--faint); font-size: 9px; }
  .manage { display: flex; align-items: center; gap: 7px; padding: 9px 11px; border: 1px solid var(--node-border); background: var(--bg); color: var(--text); font-size: 10px; cursor: pointer; white-space: nowrap; }
  .library-content { padding-top: 20px; } .library-content[hidden] { display: none; }
  .active-head { margin-bottom: 16px; } .selection { display: grid; grid-template-columns: minmax(0,1fr) minmax(0,1fr); gap: 14px; align-items: start; }
  .hint { color: var(--muted); font-size: 10px; line-height: 1.6; } .actions { display: flex; gap: 9px; align-items: center; flex-wrap: wrap; margin-top: 16px; }
  .action { background: var(--accent); color: var(--bg); border: 1px solid var(--accent); padding: 10px 13px; font-size: 10px; cursor: pointer; }
  .secondary { padding: 9px 11px; background: var(--bg); border: 1px solid var(--node-border); color: var(--text); font-size: 10px; cursor: pointer; } button:disabled { opacity: .4; cursor: default; }
  .test-result { border: 1px solid var(--node-border); margin-top: 13px; padding: 11px; font-size: 10px; } .test-result.ok { border-color: var(--led-ok); } .test-result p { overflow-wrap: anywhere; } .error { color: var(--led-err); overflow-wrap: anywhere; }
  @media(max-width:620px) { .selection { grid-template-columns: 1fr; } .library-summary { align-items: flex-start; } }
</style>
