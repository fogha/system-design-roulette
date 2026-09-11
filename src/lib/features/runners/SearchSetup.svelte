<script lang="ts">
  import { onMount } from 'svelte';
  import { Globe, Search } from 'lucide-svelte';
  import { api, type SearchProvider, type SearchResult, type SearchSettingsView } from '$lib/ipc';
  import NodeCard from '$lib/components/NodeCard.svelte';

  const PROVIDERS: { id: SearchProvider; label: string; hint: string }[] = [
    { id: 'searxng', label: 'SearXNG', hint: 'Open source, runs on your machine, no key. Asks the public engines for you. Remote Ledger installs one on port 8899; the desk can share it.' },
    { id: 'brave', label: 'Brave Search', hint: '2,000 queries a month free. One key, nothing to run.' },
    { id: 'tavily', label: 'Tavily', hint: 'Returns cleaned page text as well as links. One key, nothing to run.' },
    { id: 'none', label: 'Off', hint: 'No web search. Lessons use only the pages the curriculum names, and their mirrors.' },
  ];

  let settings = $state<SearchSettingsView | null>(null);
  let busy = $state('');
  let error = $state('');
  let url = $state('');
  let key = $state('');
  let query = $state('bash redirection operators');
  let results = $state<SearchResult[] | null>(null);
  const provider = $derived(settings?.provider ?? 'none');
  const keySet = $derived(provider === 'brave' ? !!settings?.brave_key_set : provider === 'tavily' ? !!settings?.tavily_key_set : false);

  onMount(() => { void load(); });

  async function load() {
    busy = 'load';
    try { settings = await api.getSearchSettings(); url = settings.searxng_url; }
    catch (e) { error = String(e); }
    finally { busy = ''; }
  }
  async function choose(next: SearchProvider) {
    if (busy) return;
    busy = 'provider'; error = ''; results = null;
    try { settings = await api.setSearchSettings(next, url || settings?.default_searxng_url || ''); url = settings.searxng_url; }
    catch (e) { error = String(e); }
    finally { busy = ''; }
  }
  async function saveUrl() {
    busy = 'url'; error = '';
    try { settings = await api.setSearchSettings(provider, url); url = settings.searxng_url; }
    catch (e) { error = String(e); }
    finally { busy = ''; }
  }
  async function saveKey() {
    busy = 'key'; error = '';
    try { settings = await api.setSearchKey(provider, key); key = ''; }
    catch (e) { error = String(e); }
    finally { busy = ''; }
  }
  async function test() {
    busy = 'test'; error = ''; results = null;
    try { results = await api.testSearch(query); }
    catch (e) { error = String(e); }
    finally { busy = ''; }
  }
</script>

<NodeCard Icon={Globe} name="web-search" badge={settings ? (settings.available ? 'working' : provider === 'none' ? 'off' : 'setup needed') : '…'} badgeTone={settings?.available ? 'teal' : provider === 'none' ? 'muted' : 'amber'}>
  <div class="head"><strong>Web search for the tutor</strong><p>A tutor on Ollama, OpenRouter or a bare API cannot look anything up. With a search engine configured, the desk searches and fetches the documentation itself and hands the tutor only pages it actually retrieved, still held to each subject's source allowlist.</p></div>
  <div class="chips" role="radiogroup" aria-label="Search provider">
    {#each PROVIDERS as option (option.id)}
      <button type="button" class="chip mono" class:on={provider === option.id} role="radio" aria-checked={provider === option.id} disabled={!!busy} title={option.hint} onclick={() => choose(option.id)}>{option.label}</button>
    {/each}
  </div>
  <p class="hint">{PROVIDERS.find((option) => option.id === provider)?.hint}</p>

  {#if provider === 'searxng'}
    <label class="field"><span class="mono">SEARXNG ADDRESS</span><div class="row"><input type="url" bind:value={url} placeholder={settings?.default_searxng_url} spellcheck="false" /><button type="button" class="secondary mono" disabled={!!busy} onclick={saveUrl}>{busy === 'url' ? 'Saving…' : 'Save'}</button></div></label>
    <p class="hint">JSON output must be on: <code>json</code> under <code>search.formats</code> in its settings.yml, or the API answers 403.</p>
  {:else if provider === 'brave' || provider === 'tavily'}
    <label class="field"><span class="mono">{provider === 'brave' ? 'BRAVE SEARCH' : 'TAVILY'} API KEY · {keySet ? 'set' : 'not set'}</span><div class="row"><input type="password" bind:value={key} placeholder={keySet ? 'Paste a new key to replace it, or leave blank to clear' : 'Paste the key'} autocomplete="off" spellcheck="false" /><button type="button" class="secondary mono" disabled={!!busy} onclick={saveKey}>{busy === 'key' ? 'Saving…' : key ? 'Save key' : keySet ? 'Clear key' : 'Save key'}</button></div></label>
    <p class="hint">Kept in the system keychain, never in the profile database or its exports.</p>
  {/if}

  {#if settings}
    <div class="status" class:ok={settings.available} role="status">{settings.why}</div>
  {/if}

  {#if provider !== 'none'}
    <div class="test">
      <label class="field"><span class="mono">TRY A SEARCH</span><div class="row"><input type="text" bind:value={query} spellcheck="false" onkeydown={(event) => { if (event.key === 'Enter') void test(); }} /><button type="button" class="secondary mono" disabled={!!busy || !settings?.available} onclick={test}><Search size={11} /> {busy === 'test' ? 'Searching…' : 'Search'}</button></div></label>
      {#if results}
        {#if results.length === 0}<p class="hint">No results came back for that query.</p>{:else}
          <ol class="results">
            {#each results as result (result.url)}
              <li><strong>{result.title || result.url}</strong><span class="mono url">{result.url}</span>{#if result.snippet}<p>{result.snippet}</p>{/if}</li>
            {/each}
          </ol>
        {/if}
      {/if}
    </div>
  {/if}
  {#if error}<p class="error" role="alert">{error}</p>{/if}
</NodeCard>

<style>
  .head { margin-bottom: 14px; } .head strong { display: block; font-size: 14px; font-weight: 500; } .head p, .hint { margin: 5px 0 0; font-size: 11px; color: var(--muted); line-height: 1.6; }
  .chips { display: flex; flex-wrap: wrap; gap: 6px; margin-top: 4px; }
  .chip { padding: 8px 12px; border: 1px solid var(--node-border); background: var(--bg); color: var(--muted); font-size: 10px; letter-spacing: .5px; cursor: pointer; }
  .chip.on { border-color: var(--violet); color: var(--violet-fg); background: var(--violet-bg); }
  .chip:disabled { opacity: .5; cursor: default; }
  .field { display: block; margin-top: 14px; } .field > span { display: block; font-size: 9px; letter-spacing: 1px; color: var(--faint); margin-bottom: 6px; }
  .row { display: flex; gap: 8px; } .row input { flex: 1; min-width: 0; font-size: 12px; padding: 8px 10px; }
  .secondary { display: inline-flex; align-items: center; gap: 6px; padding: 8px 11px; background: var(--bg); border: 1px solid var(--node-border); color: var(--text); font-size: 10px; cursor: pointer; white-space: nowrap; } .secondary:disabled { opacity: .4; cursor: default; }
  code { font-family: var(--font-mono); font-size: 10px; color: var(--fg); }
  .status { margin-top: 14px; padding: 9px 11px; border: 1px solid var(--node-border); font-size: 11px; color: var(--muted); } .status.ok { border-color: var(--led-ok); color: var(--fg); }
  .test { margin-top: 6px; }
  .results { margin: 12px 0 0; padding: 0 0 0 18px; display: flex; flex-direction: column; gap: 10px; } .results li { font-size: 12px; } .results strong { font-weight: 500; display: block; } .results p { margin: 3px 0 0; font-size: 11px; color: var(--muted); line-height: 1.5; } .url { display: block; font-size: 9.5px; color: var(--accent); overflow-wrap: anywhere; margin-top: 2px; }
  .error { color: var(--led-err); font-size: 11px; margin-top: 10px; overflow-wrap: anywhere; }
</style>
