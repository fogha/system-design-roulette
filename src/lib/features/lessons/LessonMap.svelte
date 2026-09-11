<script lang="ts">
  import type { SessionPlan } from '../../ipc';
  import { readingMinutes, type LessonSection } from './sections';
  import { iconSvg } from './icons';
  import { Check } from 'lucide-svelte';

  let {
    sections,
    current,
    plan,
    onjump,
  }: {
    sections: LessonSection[];
    /** Index of the section the reader is in. */
    current: number;
    plan: SessionPlan;
    onjump: (section: LessonSection) => void;
  } = $props();

  const hinted = $derived(readingMinutes(sections));
  const readMinutes = $derived(hinted > 0 ? hinted : plan.learn_minutes);
  const total = $derived(readMinutes + plan.practice_minutes + plan.check_minutes);
</script>

{#if sections.length}
  <nav class="lesson-map" aria-label="Lesson sections">
    <div class="map-head">
      <span class="eyebrow mono">THIS SESSION · ABOUT {total} MIN</span>
      <ul class="plan mono" aria-label="How the time divides">
        <li><span class="dot read"></span>read ~{readMinutes} min</li>
        <li><span class="dot practise"></span>practise {plan.practice_minutes} min</li>
        <li><span class="dot check"></span>check {plan.check_minutes} min</li>
      </ul>
    </div>
    <ol class="parts">
      {#each sections as section (section.id)}
        <li>
          <button
            type="button"
            class="part"
            class:current={section.index === current}
            class:done={section.index < current}
            onclick={() => onjump(section)}
            aria-current={section.index === current ? 'location' : undefined}
            title={section.purpose || section.title}
          >
            <span class="part-icon" aria-hidden="true">{@html iconSvg(section.icon, 14)}</span>
            <span class="part-text">
              <span class="part-title">{section.title}</span>
              {#if section.hint}<span class="part-minutes mono">~{section.hint.minutes} min</span>{/if}
            </span>
            {#if section.index < current}<span class="part-done" aria-label="read"><Check size={11} /></span>{/if}
          </button>
        </li>
      {/each}
    </ol>
  </nav>
{/if}

<style>
  .lesson-map {
    margin: 0 0 26px;
    padding: 14px 16px 12px;
    border: 1px solid var(--node-border);
    border-radius: var(--radius-panel);
    background: color-mix(in srgb, var(--surface) 70%, transparent);
  }
  .map-head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 8px 16px;
    margin-bottom: 10px;
  }
  .eyebrow { color: var(--accent); font-size: 9px; letter-spacing: 1.4px; }
  .plan {
    display: flex;
    gap: 14px;
    margin: 0;
    padding: 0;
    list-style: none;
    font-size: 9.5px;
    letter-spacing: 0.5px;
    color: var(--muted);
  }
  .plan li { display: inline-flex; align-items: center; gap: 6px; }
  .dot { width: 7px; height: 7px; border-radius: 50%; background: var(--accent); }
  .dot.practise { background: var(--ok-fg); }
  .dot.check { background: var(--violet-fg, var(--muted)); }
  .parts {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(168px, 1fr));
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .part {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 9px;
    min-height: 40px;
    padding: 6px 9px;
    border: 1px solid transparent;
    border-radius: var(--radius-control);
    background: transparent;
    color: var(--muted);
    text-align: left;
    cursor: pointer;
    font: inherit;
  }
  .part:hover { background: var(--surface); color: var(--fg); }
  .part:focus-visible { outline: 2px solid var(--accent); outline-offset: 1px; }
  .part.current {
    border-color: color-mix(in srgb, var(--accent) 55%, var(--node-border));
    background: color-mix(in srgb, var(--accent) 9%, var(--surface));
    color: var(--fg);
  }
  .part.done { color: var(--faint); }
  .part.done .part-icon { color: var(--faint); }
  .part-icon {
    flex: none;
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: var(--radius-detail);
    background: var(--surface-2);
    color: var(--accent);
  }
  .part-text { display: flex; flex-direction: column; gap: 1px; min-width: 0; }
  .part-title { font-size: 11.5px; line-height: 1.25; }
  .part-minutes { font-size: 8.5px; letter-spacing: 0.6px; color: var(--faint); }
  .part-done { margin-left: auto; color: var(--ok-fg); display: inline-flex; }
</style>
