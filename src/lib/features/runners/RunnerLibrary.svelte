<script lang="ts">
  import { untrack } from 'svelte';
  import { Terminal, KeyRound, HardDrive, Check } from 'lucide-svelte';
  import { api } from '$lib/ipc';
  import type { RunnerInfo, RunnerConfiguration } from '$lib/contracts/agents';
  import ModelLibrary from './ModelLibrary.svelte';
  import LocalModels from './LocalModels.svelte';
  import { openRunnerLink } from './links';
  let { runners, selected = $bindable('claude'), allowKeyEditing = true, onchanged }: {
    runners: RunnerInfo[]; selected?: string; allowKeyEditing?: boolean; onchanged: () => void | Promise<void>;
  } = $props();
  let route = $state('cli'); let draft = $state<RunnerConfiguration | null>(null);
  let loading = $state(false); let busy = $state(''); let message = $state(''); let error = $state('');
  let keyInput = $state(''); let keyOpen = $state(false); let freeOnly = $state(true); let catalogueRevision = $state(0);
  const drafts = new Map<string, RunnerConfiguration>();
  let sequence = 0;
  const picked = $derived(runners.find(r => r.provider === selected));
  const group = $derived(runners.filter(r => r.kind === route));
  const routes = [{ id: 'cli', label: 'CLI agents', Icon: Terminal }, { id: 'api', label: 'API providers', Icon: KeyRound }, { id: 'local', label: 'Local models', Icon: HardDrive }];
  const docs: Record<string,string> = { claude: 'https://code.claude.com/docs/en/setup', codex: 'https://developers.openai.com/codex/cli', cursor: 'https://cursor.com/docs/cli', gemini: 'https://github.com/google-gemini/gemini-cli' };
  async function load(provider: string) {
    const token = ++sequence; loading = true; message = ''; error = ''; keyInput = ''; keyOpen = false;
    try {
      const setup = drafts.get(provider) ?? await api.getRunnerConfiguration(provider);
      const free = await api.getOpenrouterFreeOnly();
      if (token === sequence) { draft = setup; freeOnly = free; }
    } catch (e) { if (token === sequence) error = String(e); }
    finally { if (token === sequence) loading = false; }
  }
  $effect(() => {
    const provider = selected;
    untrack(() => { const kind = runners.find(r => r.provider === provider)?.kind; if (kind) route = kind; if (draft) drafts.set(draft.runner.replace(/-(cli|api)$/, ''), draft); draft = null; void load(provider); });
    return () => { sequence++; };
  });
  function browse(kind: string) { const first = runners.find(r => r.kind === kind); if (first) selected = first.provider; }
  async function save() {
    if (!draft) return; busy = 'setup'; error = ''; message = '';
    try { draft = await api.saveRunnerConfiguration($state.snapshot(draft)); drafts.set(selected, draft); message = 'Runner setup saved. Select it in the active tutor box when you are ready.'; await onchanged(); }
    catch (e) { error = String(e); } finally { busy = ''; }
  }
  async function saveKey(clear = false) {
    if (!picked) return; busy = 'key'; error = ''; message = '';
    try { await api.setRunnerKey(picked.id, clear ? '' : keyInput); keyInput = ''; keyOpen = false; catalogueRevision++; await onchanged(); message = clear ? 'Saved key cleared.' : 'Provider key saved.'; }
    catch (e) { error = String(e); } finally { busy = ''; }
  }
  async function toggleFree() { try { await api.setOpenrouterFreeOnly(!freeOnly); freeOnly = !freeOnly; await onchanged(); } catch (e) { error = String(e); } }
</script>

<div class="runner-library">
  <div class="routes" aria-label="Runner configuration categories">{#each routes as item}<button type="button" class:active={route === item.id} aria-pressed={route === item.id} onclick={() => browse(item.id)}><item.Icon size={14} /><span>{item.label}</span><small>{runners.filter(r => r.kind === item.id).length}</small></button>{/each}</div>
  {#if route === 'local'}
    {#if draft}<LocalModels bind:models={draft.models} onchanged={onchanged} />{/if}
  {:else}
    <div class="workspace">
      <div class="providers" aria-label="Configure a runner">
        {#each group as runner (runner.id)}<button type="button" class:editing={selected === runner.provider} aria-pressed={selected === runner.provider} onclick={() => selected = runner.provider}><span class="dot" class:ready={runner.available}></span><span>{runner.label}</span></button>{/each}
        <button type="button" class="refresh" onclick={() => onchanged()}>Refresh detection</button>
      </div>
      <div class="editor">
        <header><strong>{picked?.label ?? selected}</strong><span class="status">{picked?.available ? route === 'api' ? 'KEY CONFIGURED' : 'DETECTED' : route === 'api' ? 'NEEDS KEY' : 'NOT FOUND'}</span></header>
        {#if route === 'cli'}
          <p>Uses the CLI’s existing account and authentication.</p>
          {#if picked && !picked.available && docs[selected]}<a href={docs[selected]} target="_blank" rel="noreferrer" onclick={e => openRunnerLink(e).catch(e => error = String(e))}>Installation instructions →</a>{/if}
          {#if selected === 'custom' && draft}<label class="command">Custom command<input aria-label="Saved custom runner command" bind:value={draft.custom_command} placeholder={"'/path/to/agent' --print {prompt}"} /></label><p>Use {'{model}'} for the selected model and {'{prompt}'} for the prompt. Without {'{prompt}'}, the prompt is appended as the final argument.</p>{/if}
        {:else}
          {#if allowKeyEditing}<div class="key-row">{#if keyOpen}<input type="password" aria-label={`${picked?.label} API key`} autocomplete="off" placeholder="Paste provider key" bind:value={keyInput} /><button type="button" class="small-action" disabled={!!busy || !keyInput.trim()} onclick={() => saveKey()}>Save key</button>{:else}<button type="button" class="small-action" onclick={() => keyOpen = true}>{picked?.available ? 'Replace key' : 'Add API key'}</button>{/if}{#if picked?.available}<button type="button" class="text-action" disabled={!!busy} onclick={() => saveKey(true)}>Clear saved key</button>{/if}</div><p>macOS keys use Keychain. Environment keys take priority.</p>{:else}<p>Manage shared API keys in Settings.</p>{/if}
          {#if selected === 'openrouter'}<button type="button" class="free-toggle" aria-pressed={freeOnly} onclick={toggleFree}><span>{#if freeOnly}<Check size={11} />{/if}</span>Free models only</button>{/if}
        {/if}
        {#if loading}<p role="status">Loading runner setup…</p>{:else if draft}<ModelLibrary runner={selected} bind:models={draft.models} freeOnly={selected === 'openrouter' && freeOnly} revision={catalogueRevision} />{/if}
      </div>
    </div>
  {/if}
  <div class="save-row"><button type="button" class="save mono" disabled={!!busy || !draft || loading} onclick={save}>{busy === 'setup' ? 'Saving…' : 'Save runner setup'}</button><span>Configuration stays separate from the active tutor.</span></div>
  {#if message}<p class="notice" role="status">{message}</p>{/if}{#if error}<p class="error" role="alert">{error}</p>{/if}
</div>

<style>
  .runner-library { display: grid; gap: 16px; }
  .routes { display: flex; gap: 5px; border-bottom: 1px solid var(--node-border); padding-bottom: 10px; }
  .routes button { display: flex; align-items: center; gap: 7px; flex: 1; padding: 9px 7px; color: var(--muted); background: var(--bg); border: 1px solid var(--node-border); border-radius: var(--radius-control); font-size: 11px; cursor: pointer; } .routes .active { color: var(--accent); border-color: var(--accent); } .routes small { margin-left: auto; font: 9px var(--font-mono); opacity: .6; }
  .workspace { display: grid; grid-template-columns: 152px minmax(0,1fr); gap: 17px; }
  .providers { display: grid; align-content: start; gap: 4px; } .providers button { display: flex; align-items: center; gap: 8px; border: 1px solid transparent; padding: 9px 7px; border-radius: var(--radius-control); background: transparent; color: var(--muted); text-align: left; font-size: 11px; cursor: pointer; } .providers .editing { color: var(--text); background: var(--surface-2); border-color: var(--node-border); }
  .dot { width: 5px; height: 5px; border-radius: 50%; background: var(--faint); flex-shrink: 0; } .dot.ready { background: var(--led-ok); } .providers button.refresh { font-size: 9px; margin-top: 10px; padding-left: 0; color: var(--accent); }
  .editor { min-width: 0; display: grid; gap: 11px; align-content: start; border-left: 1px dashed var(--node-border); padding-left: 17px; }
  header { display: flex; justify-content: space-between; align-items: center; gap: 8px; } strong { font-size: 13px; font-weight: 500; } .status { font: 8px var(--font-mono); color: var(--muted); }
  p { font-size: 10px; line-height: 1.6; color: var(--muted); margin: 0; } a { color: var(--accent); font-size: 11px; }
  .key-row { display: flex; align-items: center; gap: 7px; flex-wrap: wrap; } input { min-width: 0; flex: 1; border: 1px solid var(--node-border); padding: 8px; color: var(--text); background: var(--bg); font-size: 11px; } .command { display: grid; gap: 7px; font: 10px var(--font-mono); color: var(--muted); }
  .small-action { background: var(--bg); border: 1px solid var(--node-border); color: var(--text); padding: 6px 8px; font-size: 10px; cursor: pointer; } .text-action { border: 0; background: transparent; color: var(--muted); font-size: 10px; cursor: pointer; }
  .free-toggle { justify-self: start; display: flex; align-items: center; gap: 7px; background: transparent; border: 0; color: var(--text); font-size: 11px; padding: 0; cursor: pointer; } .free-toggle span { border-radius: var(--radius-detail); display: grid; place-items: center; width: 16px; height: 16px; border: 1px solid var(--node-border); color: var(--accent); }
  .save-row { border-top: 1px dashed var(--node-border); padding-top: 13px; display: flex; align-items: center; gap: 12px; } .save-row > span { font-size: 10px; color: var(--muted); line-height: 1.5; } .save { padding: 9px 11px; border: 1px solid var(--accent); background: var(--bg); color: var(--accent); font-size: 10px; cursor: pointer; white-space: nowrap; } button:disabled { opacity: .4; cursor: default; }
  .notice { color: var(--led-ok); } .error { color: var(--led-err); overflow-wrap: anywhere; }
  @media(max-width:620px) { .workspace { grid-template-columns: 1fr; } .providers { grid-template-columns: repeat(2,1fr); } .providers button.refresh { margin-top: 0; } .editor { border-left: 0; padding-left: 0; border-top: 1px dashed var(--node-border); padding-top: 12px; } .routes small { display: none; } .save-row { flex-wrap: wrap; } }
</style>
