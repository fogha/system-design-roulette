<script lang="ts">
  import { api } from '$lib/ipc';

  /** Primary generation-provider selector. API credentials stay in the Rust
   *  process (Keychain-backed for deepseek) and are never stored in
   *  frontend state — this component only ever sees whether a key is
   *  configured, never the key's value once saved. */
  let {
    agent = $bindable('claude'),
    customBin = $bindable(''),
    deepseekKeyConfigured = false,
    allowKeyEditing = true,
    onKeyChanged,
  }: {
    agent?: string;
    customBin?: string;
    deepseekKeyConfigured?: boolean;
    allowKeyEditing?: boolean;
    onKeyChanged?: () => void | Promise<void>;
  } = $props();

  let keyInput = $state('');
  let keySaving = $state(false);
  let keySaved = $state(false);
  let keyError = $state('');
  // Local, mutable mirror of the prop: needs to flip immediately after a
  // save/clear without waiting for the parent to refetch app state.
  let keyConfigured = $state(false);

  $effect(() => {
    keyConfigured = deepseekKeyConfigured;
  });

  async function saveKey() {
    keySaving = true;
    keyError = '';
    try {
      await api.setDeepseekApiKey(keyInput);
      keyConfigured = keyInput.trim().length > 0;
      keyInput = '';
      await onKeyChanged?.();
      keySaved = true;
      setTimeout(() => (keySaved = false), 1600);
    } catch (e) {
      keyError = e instanceof Error ? e.message : String(e);
    } finally {
      keySaving = false;
    }
  }

  async function clearKey() {
    keySaving = true;
    keyError = '';
    try {
      await api.setDeepseekApiKey('');
      keyConfigured = false;
      await onKeyChanged?.();
    } catch (e) {
      keyError = e instanceof Error ? e.message : String(e);
    } finally {
      keySaving = false;
    }
  }

  const AGENTS = [
    {
      id: 'claude',
      name: 'CLAUDE',
      tag: 'claude code cli',
      desc: 'Full support: model choice (opus/sonnet/haiku), web research for course resources, live agent log, JSON repair. Needs the claude CLI installed and authenticated.',
    },
    {
      id: 'codex',
      name: 'CODEX',
      tag: 'openai codex cli',
      desc: 'Runs `codex exec` and reads the final message. Uses whatever model your codex CLI is configured with. Needs codex installed and a valid ~/.codex/config.toml. Claude (if present) is the JSON-repair fallback.',
    },
    {
      id: 'cursor',
      name: 'CURSOR',
      tag: 'cursor-agent cli',
      desc: 'Runs `cursor-agent -p --output-format text`. Needs cursor-agent installed and authenticated (`cursor-agent login` or CURSOR_API_KEY).',
    },
    {
      id: 'gemini',
      name: 'GEMINI',
      tag: 'google gemini cli',
      desc: 'Runs `gemini -p`. Needs the gemini CLI installed and authenticated.',
    },
    {
      id: 'deepseek',
      name: 'DEEPSEEK',
      tag: 'hosted api · v4',
      desc: 'Calls the official DeepSeek API directly. One app-wide key powers the primary teacher and every classroom subject; it is stored in the macOS Keychain or read from DEEPSEEK_API_KEY. Defaults to deepseek-v4-flash; DEEPSEEK_MODEL overrides that.',
    },
    {
      id: 'custom',
      name: 'CUSTOM',
      tag: 'any other cli',
      desc: 'Any CLI that prints the answer to stdout. Use {prompt} in the command to place the prompt (lets you add flags); without it the prompt is appended as the final argument.',
    },
  ];
</script>

<div class="apicker">
  {#each AGENTS as a}
    <button class="agt mono" class:active={agent === a.id} onclick={() => (agent = a.id)}>
      <span class="agt-name">{a.name}</span>
      <span class="agt-tag">{a.tag}</span>
    </button>
  {/each}
</div>
{#each AGENTS.filter((a) => a.id === agent) as a}
  <p class="agt-desc">{a.desc}</p>
{/each}
{#if agent === 'custom'}
  <input
    class="agt-bin mono"
    type="text"
    placeholder={'e.g. /usr/local/bin/mycli --print {prompt}'}
    bind:value={customBin}
  />
{/if}
{#if agent === 'deepseek'}
  <div class="key-row">
    <span class="key-status mono" class:ok={keyConfigured}>
      {keyConfigured ? 'GLOBAL API KEY: CONFIGURED' : 'GLOBAL API KEY: NOT SET'}
    </span>
    {#if allowKeyEditing}
      <input
        class="agt-bin mono key-input"
        type="password"
        placeholder="sk-… (one key shared by every class)"
        bind:value={keyInput}
        autocomplete="off"
      />
      <button
        class="key-btn mono"
        disabled={keySaving || !keyInput.trim()}
        onclick={saveKey}
      >
        {keySaved ? 'saved ✓' : keySaving ? 'saving…' : 'save global key'}
      </button>
      {#if keyConfigured}
        <button class="key-btn key-clear mono" disabled={keySaving} onclick={clearKey}>
          clear
        </button>
      {/if}
    {:else}
      <span class="key-scope">
        Every DeepSeek classroom teacher uses the same key from global app settings.
      </span>
    {/if}
  </div>
  {#if keyError}
    <p class="key-error mono">{keyError}</p>
  {/if}
{/if}

<style>
  .apicker {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 8px;
    margin-top: 8px;
  }
  .agt {
    display: flex;
    flex-direction: column;
    gap: 3px;
    align-items: flex-start;
    background: var(--bg);
    border: 1px solid var(--node-border);
    border-radius: 7px;
    padding: 8px 11px;
    cursor: pointer;
    text-align: left;
  }
  .agt:hover {
    border-color: var(--muted);
  }
  .agt.active {
    border-color: var(--violet);
    background: var(--surface-2);
  }
  .agt-name {
    font-size: 11px;
    letter-spacing: 1.5px;
    color: var(--muted);
  }
  .agt.active .agt-name {
    color: var(--violet-fg);
  }
  .agt-tag {
    font-size: 8.5px;
    color: var(--faint);
    letter-spacing: 0.5px;
  }
  .agt-desc {
    font-size: 12.5px;
    color: var(--muted);
    line-height: 1.55;
    margin: 10px 0 2px;
  }
  .agt-bin {
    margin-top: 8px;
    font-size: 12px;
    background: var(--bg);
    border: 1px solid var(--node-border);
    border-radius: 6px;
    padding: 8px 12px;
  }
  .key-row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin-top: 8px;
  }
  .key-status {
    font-size: 9.5px;
    letter-spacing: 0.6px;
    padding: 4px 8px;
    border-radius: 5px;
    background: var(--bad-bg, rgba(255, 80, 80, 0.12));
    color: var(--bad-fg, #d66);
    white-space: nowrap;
  }
  .key-status.ok {
    background: var(--good-bg, rgba(80, 200, 120, 0.12));
    color: var(--good-fg, #2a9d5c);
  }
  .key-input {
    flex: 1;
    min-width: 180px;
    margin-top: 0;
  }
  .key-scope {
    color: var(--muted);
    font-size: 10px;
    line-height: 1.45;
  }
  .key-btn {
    font-size: 11px;
    padding: 8px 14px;
    background: var(--bg);
    border: 1px solid var(--node-border);
    border-radius: 6px;
    cursor: pointer;
    white-space: nowrap;
  }
  .key-btn:hover:not(:disabled) {
    border-color: var(--violet);
  }
  .key-btn:disabled {
    opacity: 0.55;
    cursor: default;
  }
  .key-clear {
    color: var(--bad-fg, #d66);
  }
  .key-error {
    font-size: 11px;
    color: var(--bad-fg, #d66);
    margin: 6px 0 0;
  }
</style>
