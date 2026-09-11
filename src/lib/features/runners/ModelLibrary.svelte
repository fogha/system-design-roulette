<script lang="ts">
  import { untrack } from 'svelte';
  import { Plus, Check, X, Search } from 'lucide-svelte';
  import { api } from '$lib/ipc';
  import type { ModelCatalog } from '$lib/contracts/agents';
  import { modelPage } from './model-list';
  let { runner, models = $bindable([]), freeOnly = false, revision = 0 }: { runner: string; models?: string[]; freeOnly?: boolean; revision?: number } = $props();
  let catalogue = $state<ModelCatalog>({ models: [], source: '', error: null });
  let query = $state(''); let page = $state(0); let manual = $state(''); let manualOpen = $state(false);
  let loading = $state(false); let error = $state(''); let sequence = 0;
  const results = $derived(modelPage(catalogue.models, query, page, freeOnly));
  async function load(refresh: boolean) {
    const token = ++sequence; loading = true;
    try { const result = await api.getRunnerModels(runner, refresh); if (token === sequence) catalogue = result; }
    catch (e) { if (token === sequence) error = String(e); }
    finally { if (token === sequence) loading = false; }
  }
  $effect(() => { void runner; void revision; untrack(() => { query = ''; page = 0; void load(false); }); return () => { sequence++; }; });
  function toggle(id: string) { models = models.includes(id) ? models.filter(m => m !== id) : [...models, id]; }
  function addManual() {
    const id = manual.trim();
    if (!id || id.length > 160 || /\s/.test(id)) { error = 'Enter a model ID without spaces.'; return; }
    if (!models.includes(id)) models = [...models, id]; manual = ''; error = '';
  }
</script>

<div class="model-library">
  <div class="line"><span class="eyebrow">MODEL SHORTLIST <b>{models.length}</b>{#if loading}<i class="loading" role="status">· loading catalogue…</i>{/if}</span><button type="button" class="text-action" onclick={() => manualOpen = !manualOpen}>Add by ID</button></div>
  <div class="chips" aria-label="Models in this runner’s shortlist">
    {#each models as id (id)}<button type="button" class="chip" aria-label={`Remove ${id} from shortlist`} onclick={() => toggle(id)}><span>{id}</span><X size={11} /></button>{/each}
    {#if !models.length}<p>No saved models yet. Add models from the catalogue below.</p>{/if}
  </div>
  {#if manualOpen}<div class="manual"><input aria-label="Model ID to add" placeholder="Provider model ID" bind:value={manual} onkeydown={e => { if (e.key === 'Enter') { e.preventDefault(); addManual(); } }} /><button type="button" class="small-action" onclick={addManual}>Add</button></div>{/if}
  <div class="search-row"><div class="search field-group"><Search size={13} /><input aria-label="Search model catalogue" placeholder="Search models by name or ID…" value={query} oninput={e => { query = e.currentTarget.value; page = 0; }} /></div><button type="button" class="small-action" disabled={loading} onclick={() => load(true)}>{loading ? 'Loading…' : 'Refresh'}</button></div>
  <div class="catalogue" aria-label="Model catalogue" aria-busy={loading}>
    {#if loading && !catalogue.models.length}
      {#each [0, 1, 2, 3, 4] as row (row)}
        <div class="model-row skeleton" aria-hidden="true"><span class="model-copy"><span class="bar wide"></span><span class="bar"></span></span></div>
      {/each}
    {/if}
    {#each results.items as item (item.id)}
      <button type="button" class="model-row" class:added={models.includes(item.id)} aria-pressed={models.includes(item.id)} onclick={() => toggle(item.id)}>
        <span class="model-copy"><strong>{item.label}</strong><small>{item.id}</small></span>
        <span class="price">{item.free ? 'FREE' : item.input_usd_per_million === null ? '' : `$${item.input_usd_per_million.toFixed(2)} / $${item.output_usd_per_million?.toFixed(2) ?? '?'}`}</span>
        {#if models.includes(item.id)}<Check size={13} />{:else}<Plus size={13} />{/if}
      </button>
    {/each}
    {#if !results.count && !loading}<p class="empty">{query ? 'No models match your search.' : 'No catalogue is available. Add a model by ID, or configure a provider key first.'}</p>{/if}
  </div>
  <div class="pagination"><span>{loading && !catalogue.models.length ? 'reading catalogue…' : `${results.count} models`}{results.count ? ` · ${results.current * 5 + 1}–${Math.min((results.current + 1) * 5, results.count)}` : ''}</span><div><button type="button" class="small-action" aria-label="Previous model page" disabled={results.current === 0} onclick={() => page = results.current - 1}>←</button><span>{results.current + 1} / {results.pages}</span><button type="button" class="small-action" aria-label="Next model page" disabled={results.current + 1 >= results.pages} onclick={() => page = results.current + 1}>→</button></div></div>
  {#if runner === 'openrouter'}<p>Prices show input / output per million tokens.</p>{/if}
  {#if catalogue.error || error}<p class="error" role="status">{error || catalogue.error}</p>{/if}
</div>

<style>
  .model-library { display: grid; gap: 10px; min-width: 0; }
  .line,.pagination,.pagination > div { display: flex; justify-content: space-between; align-items: center; gap: 9px; }
  .eyebrow,.pagination,.price { color: var(--muted); font: 9px/1.5 var(--font-mono); } .eyebrow { letter-spacing: .7px; } b { color: var(--accent); margin-left: 5px; }
  .chips { display: flex; flex-wrap: wrap; gap: 5px; max-height: 96px; overflow-y: auto; }
  .chip { display: flex; align-items: center; gap: 7px; max-width: 100%; padding: 4px 7px; border: 1px solid var(--node-border); border-radius: var(--radius-control); color: var(--text); background: var(--bg); font: 10px/1.5 var(--font-mono); cursor: pointer; } .chip span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .search-row,.manual { display: flex; gap: 7px; } .search:focus-within { border-color: var(--accent); } .search input { outline: none; } .search { border-radius: var(--radius-control); overflow: hidden; display: flex; align-items: center; gap: 7px; flex: 1; min-width: 0; border: 1px solid var(--node-border); padding-left: 9px; background: var(--bg); color: var(--muted); }
  input { width: 100%; min-width: 0; padding: 8px; background: var(--bg); color: var(--text); border: 1px solid var(--node-border); font-size: 11px; } .search input { border: 0; }
  .catalogue { border-top: 1px solid var(--node-border); }
  .model-row { display: flex; align-items: center; gap: 9px; text-align: left; padding: 9px 3px; min-height: 49px; width: 100%; background: transparent; border: 0; border-bottom: 1px solid var(--node-border); color: var(--muted); cursor: pointer; } .model-row:hover,.model-row.added { color: var(--accent); } .model-copy { flex: 1; min-width: 0; } strong { color: var(--text); font-size: 11px; font-weight: 500; display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } small { font-size: 9px; display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; } .model-row :global(svg) { flex-shrink: 0; }
  .small-action { padding: 6px 8px; color: var(--text); background: var(--bg); border: 1px solid var(--node-border); font-size: 10px; cursor: pointer; } button:disabled { opacity: .4; cursor: default; }
  .text-action { color: var(--accent); background: transparent; border: 0; font-size: 10px; cursor: pointer; }
  .loading { font-style: normal; color: var(--accent); margin-left: 6px; }
  .skeleton { pointer-events: none; } .skeleton .model-copy { display: grid; gap: 7px; }
  .bar { display: block; height: 9px; width: 42%; border-radius: var(--radius-detail); background: linear-gradient(90deg, var(--surface) 25%, var(--surface-2) 37%, var(--surface) 63%); background-size: 320% 100%; animation: catalogue-shimmer 1.3s ease-in-out infinite; }
  .bar.wide { width: 68%; height: 11px; }
  @keyframes catalogue-shimmer { from { background-position: 100% 0; } to { background-position: 0 0; } }
  @media (prefers-reduced-motion: reduce) { .bar { animation: none; } }
  p { margin: 0; color: var(--muted); font-size: 10px; line-height: 1.6; } .empty { padding: 15px 0; } .error { color: var(--led-err); overflow-wrap: anywhere; }
</style>
