<script lang="ts">
  /** Stable frame for every class lesson: cluster bar, identity, stage rail,
   *  save state, time budget, reading preferences and the return action. */
  import type { Snippet } from 'svelte';
  import type { LessonStage } from '../../ipc';
  import type { StageLink } from './types';
  import ClusterBar from '../../components/ClusterBar.svelte';
  import { ArrowLeft, Minus, Plus } from 'lucide-svelte';


  let {
    route,
    status,
    code,
    eyebrow,
    title,
    subtitle,
    stages,
    stage,
    saveMessage,
    saveError = false,
    minutes,
    locked = false,
    returnLabel = 'pause class',
    onreturn,
    onstage,
    scroller = $bindable<HTMLElement | undefined>(undefined),
    onscroll,
    actions,
    children,
  }: {
    route: string;
    status: string;
    code: string;
    eyebrow: string;
    title: string;
    subtitle: string;
    stages: StageLink[];
    stage: LessonStage;
    saveMessage: string;
    saveError?: boolean;
    minutes: number;
    locked?: boolean;
    returnLabel?: string;
    onreturn: () => void;
    onstage: (stage: LessonStage) => void;
    scroller?: HTMLElement | undefined;
    onscroll?: () => void;
    actions?: Snippet;
    children: Snippet;
  } = $props();

  const stored = typeof localStorage !== 'undefined' ? Number(localStorage.getItem('sdr-font-size')) : 0;
  let fontSize = $state(stored || 15);
  function bump(delta: number) {
    fontSize = Math.min(21, Math.max(13, fontSize + delta));
    if (typeof localStorage !== 'undefined') localStorage.setItem('sdr-font-size', String(fontSize));
  }
</script>

<div class="lesson-shell blueprint">
  <ClusterBar {route} {status} tone="ok" />
  <header class="lesson-head">
    <button
      class="return-button mono"
      type="button"
      onclick={onreturn}
      disabled={locked}
      title={locked ? 'Focused session: finish the check, or use the escape hatch.' : undefined}
    >
      <ArrowLeft size={14} /> {locked ? 'focused · finish the check' : returnLabel}
    </button>
    <div class="identity">
      <span class="class-code mono">{code}</span>
      <div class="identity-text">
        <span class="eyebrow mono">{eyebrow}</span>
        <h1>{title}</h1>
        <p>{subtitle}</p>
      </div>
    </div>
    <div class="lesson-actions">
      <div class="reading-prefs mono" role="group" aria-label="Reading size">
        <button type="button" onclick={() => bump(-1)} disabled={fontSize <= 13} aria-label="Smaller text"><Minus size={11} /></button>
        <span>{fontSize}px</span>
        <button type="button" onclick={() => bump(1)} disabled={fontSize >= 21} aria-label="Larger text"><Plus size={11} /></button>
      </div>
      {#if actions}{@render actions()}{/if}
    </div>
    <nav class="stage-rail" aria-label="Lesson stages">
      {#each stages as link, index (link.id)}
        <button
          type="button"
          class="stage mono"
          class:current={stage === link.id}
          class:done={stages.findIndex((s) => s.id === stage) > index}
          disabled={!link.available}
          aria-current={stage === link.id ? 'step' : undefined}
          onclick={() => onstage(link.id)}
        >
          <span class="stage-index">{String(index + 1).padStart(2, '0')}</span>
          {link.label}
        </button>
      {/each}
      <span class="status-line mono" class:err={saveError} role="status">{saveMessage} · about {minutes} min</span>
    </nav>
  </header>
  <main class="reading-layout" bind:this={scroller} {onscroll} style={`--reading-font: ${fontSize}px`}>
    {@render children()}
  </main>
</div>

<style>
  .lesson-shell { min-height: 100vh; display: flex; flex: 1; flex-direction: column; background: var(--bg); overflow: hidden; }
  .lesson-head {
    padding: 12px 22px 0;
    border-bottom: 1px solid var(--node-border);
    background: var(--node-bg);
    display: grid;
    grid-template-columns: 150px minmax(0, 1fr) auto;
    align-items: center;
    gap: 14px 18px;
  }
  .return-button {
    justify-self: start;
    border: 1px solid var(--node-border);
    border-radius: var(--radius-control);
    background: transparent;
    color: var(--muted);
    min-height: 34px;
    padding: 6px 9px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    cursor: pointer;
    font-size: 10px;
  }
  .return-button:disabled { cursor: not-allowed; opacity: 0.8; }
  .identity { display: flex; align-items: center; gap: 13px; min-width: 0; }
  .identity-text { min-width: 0; }
  .class-code {
    width: 42px;
    height: 42px;
    display: grid;
    place-items: center;
    border: 1px solid var(--accent);
    border-radius: var(--radius-control);
    color: var(--accent);
    flex: none;
  }
  .eyebrow { color: var(--accent); font-size: 9px; letter-spacing: 1.4px; }
  h1 { font-size: 19px; line-height: 1.2; margin: 4px 0; overflow: hidden; display: -webkit-box; -webkit-line-clamp: 2; line-clamp: 2; -webkit-box-orient: vertical; }
  .identity p { color: var(--muted); font-size: 10px; margin: 0; }
  .lesson-actions { justify-self: end; display: flex; align-items: center; gap: 9px; }
  .reading-prefs { display: inline-flex; align-items: center; gap: 4px; color: var(--faint); font-size: 9px; }
  .reading-prefs button {
    width: 24px; height: 24px; display: grid; place-items: center; border: 1px solid var(--node-border);
    border-radius: var(--radius-detail); background: transparent; color: var(--muted); cursor: pointer;
  }
  .reading-prefs button:disabled { opacity: 0.4; cursor: default; }
  .stage-rail { grid-column: 1 / -1; display: flex; align-items: center; gap: 6px; padding: 4px 0 10px; flex-wrap: wrap; }
  .stage {
    display: inline-flex; align-items: center; gap: 6px; min-height: 26px; padding: 4px 10px;
    border: 1px solid var(--node-border); border-radius: var(--radius-detail); background: transparent;
    color: var(--muted); font-size: 9px; letter-spacing: 1px; text-transform: uppercase; cursor: pointer;
  }
  .stage-index { color: var(--faint); }
  .stage.done { color: var(--faint); }
  .stage.current { border-color: var(--accent); color: var(--accent); }
  .stage.current .stage-index { color: var(--accent); }
  .stage:disabled { opacity: 0.45; cursor: default; }
  .status-line { margin-left: auto; color: var(--faint); font-size: 9px; }
  .status-line.err { color: var(--red); }
  .reading-layout { position: relative; flex: 1; min-height: 0; overflow-y: auto; }
  .return-button:focus-visible, .stage:focus-visible, .reading-prefs button:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
  @media (max-width: 860px) {
    .lesson-head { grid-template-columns: 1fr; }
    .lesson-actions { justify-self: start; flex-wrap: wrap; }
    .status-line { margin-left: 0; width: 100%; }
  }
</style>
