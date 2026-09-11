<script lang="ts">
  import { tick, type Snippet } from 'svelte';
  import { app } from '../stores.svelte';
  import { DESTINATIONS } from './navigation';
  import ClusterBar from '../components/ClusterBar.svelte';
  import StatusLED from '../components/StatusLED.svelte';
  import PreparationStatus from '../features/classes/PreparationStatus.svelte';
  import mark from '../../../docs/logo-mark.svg';
  let { children }: { children: Snippet } = $props();
  const selected = $derived(app.screen === 'dashboard' ? 'progress' : app.destination);
  let content: HTMLElement;
  $effect(() => { selected; void tick().then(() => content?.scrollTo(0, 0)); });
</script>

<div class="desk blueprint">
  <a class="skip-link" href="#desk-content">Skip to content</a>
  <ClusterBar route={selected} status={app.locked ? 'focused session in progress' : 'all systems nominal'} tone={app.locked ? 'warn' : 'ok'} />
  <header class="command-bar">
    <button class="brand" onclick={() => app.navigate('today')} aria-label="Principia Desk home"><img src={mark} alt="" /><span>Principia Desk<small class="mono">PERSONAL LEARNING SYSTEM</small></span></button>
    <nav aria-label="Main navigation">
      {#each DESTINATIONS as destination, index}
        <button class:selected={selected === destination.id} aria-current={selected === destination.id ? 'page' : undefined} onclick={() => app.navigate(destination.id)}>
          <span class="route-index">{String(index + 1).padStart(2, '0')}</span>{destination.label}<StatusLED tone={selected === destination.id ? 'ok' : 'idle'} />
        </button>
      {/each}
    </nav>
  </header>
  {#if app.preparingClass}<PreparationStatus />{/if}
  <main id="desk-content" tabindex="-1" bind:this={content}>{@render children()}</main>
</div>

<style>
  .desk { display: flex; flex-direction: column; flex: 1; min-height: 0; }
  .command-bar { position: relative; z-index: 2; flex-shrink: 0; display: flex; align-items: center; justify-content: space-between; gap: 28px; margin: 18px 28px 0; padding-bottom: 18px; border-bottom: 1px dashed var(--node-border); }
  .brand { display: flex; align-items: center; gap: 11px; border: 0; background: none; color: var(--fg); text-align: left; font: 20px/1.2 var(--font-display); cursor: pointer; padding: 0; flex-shrink: 0; }
  .brand img { width: 36px; height: 36px; }
  .brand small { display: block; margin-top: 6px; font-size: 8px; color: var(--faint); letter-spacing: 1.3px; }
  nav { display: flex; flex-wrap: wrap; gap: 7px; }
  nav button { display: flex; align-items: center; gap: 9px; border: 1px solid var(--node-border); border-radius: var(--radius-control); background: var(--node-bg); color: var(--muted); font: 11px var(--font-mono); padding: 9px 11px; cursor: pointer; }
  nav button:hover { border-color: var(--muted); color: var(--fg); }
  nav button.selected { border-color: var(--violet); background: var(--surface-2); color: var(--violet-fg); }
  .route-index { color: var(--faint); font-size: 9px; }
  nav button.selected .route-index { color: var(--violet-fg); }
  main { min-height: 0; min-width: 0; overflow-y: auto; display: flex; flex-direction: column; flex: 1; }
  .skip-link { position: fixed; top: -60px; left: 12px; padding: 8px 12px; background: var(--surface); color: var(--fg); z-index: 90; }
  .skip-link:focus { top: 8px; }
  @media (max-width: 940px) { .command-bar { gap: 18px; } nav button { padding: 9px; gap: 6px; } .route-index { display: none; } }
  @media (max-width: 720px) { .command-bar { align-items: flex-start; flex-direction: column; margin: 14px 18px 0; gap: 16px; } nav { width: 100%; gap: 5px; } nav button { justify-content: center; flex: 1; font-size: 10px; padding: 9px 5px; } .brand { font-size: 19px; } }
  @media (max-width: 420px) { nav button { gap: 5px; } nav button :global(.led) { width: 5px; height: 5px; } }
</style>
