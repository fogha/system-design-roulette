<script lang="ts">
  import { courseDefinition } from '../catalog';
  import type { CurriculumMapView, CurriculumPhase } from '../ipc';
  import { CheckCircle2, Circle, Clock3, Link2, X } from 'lucide-svelte';

  let {
    map,
    onclose,
  }: {
    map: CurriculumMapView;
    onclose: () => void;
  } = $props();

  const phases = $derived([
    ...(courseDefinition(map.focus)?.entry_points ?? []).map((entry) => ({
      id: entry.id as CurriculumPhase, label: entry.label,
    })),
    { id: 'elective' as CurriculumPhase, label: 'Electives' },
  ].filter((phase) => map.concepts.some((concept) => concept.phase === phase.id)));
  const coreCount = $derived(map.concepts.filter((concept) => concept.core).length);
  function prerequisiteTitles(slugs: string[]) {
    return slugs.map((slug) => map.concepts.find((concept) => concept.slug === slug)?.title ?? slug).join('; ');
  }

  function conceptsFor(phase: CurriculumPhase) {
    return map.concepts.filter((concept) => concept.phase === phase);
  }

  function complete(state: string) {
    return state === 'mastered' || state === 'maintenance';
  }
</script>

<section class="curriculum-map" aria-labelledby="curriculum-map-title">
  <header>
    <div>
      <span class="eyebrow mono">{coreCount} CORE TOPICS · {map.completed_sessions} SESSIONS COMPLETED</span>
      <h3 id="curriculum-map-title">{map.label} learning map</h3>
      <p>{map.month_outcome}</p>
    </div>
    <button type="button" class="close" onclick={onclose} aria-label="Close curriculum map">
      <X size={15} />
    </button>
  </header>

  <div class="phase-grid">
    {#each phases as phase}
      {@const concepts = conceptsFor(phase.id)}
      <section class:current={map.current_phase === phase.id} class="phase">
        <div class="phase-head">
          <strong>{phase.label}</strong>
          <span class="mono">
            {concepts.filter((concept) => complete(concept.mastery_state)).length}/{concepts.length}
          </span>
        </div>
        {#if map.current_phase === phase.id}<small class="you-are-here">current phase</small>{/if}
        <ol>
          {#each concepts as concept}
            <li class:complete={complete(concept.mastery_state)} class:elective={!concept.core}>
              <span class="state">
                {#if complete(concept.mastery_state)}
                  <CheckCircle2 size={13} />
                {:else if concept.mastery_state === 'unseen'}
                  <Circle size={13} />
                {:else}
                  <Clock3 size={13} />
                {/if}
              </span>
              <div>
                <strong>{concept.title}</strong>
                <p>{concept.learner_outcome}</p>
                <span class="meta mono">
                  {concept.mastery_state}
                  {#if concept.prerequisites.length}
                    · <Link2 size={10} /> {prerequisiteTitles(concept.prerequisites)}
                  {/if}
                </span>
              </div>
            </li>
          {/each}
        </ol>
      </section>
    {/each}
  </div>
</section>

<style>
  .curriculum-map {
    grid-column: 1 / -1;
    border: 1px solid var(--border);
    background: var(--surface);
    padding: 16px;
  }
  header {
    display: flex;
    justify-content: space-between;
    gap: 18px;
    align-items: flex-start;
    margin-bottom: 16px;
  }
  h3 {
    margin: 4px 0 6px;
    color: var(--fg);
    font-size: 18px;
  }
  header p {
    margin: 0;
    max-width: 920px;
    color: var(--muted);
    line-height: 1.55;
    font-size: 14px;
  }
  .eyebrow,
  .meta,
  .phase-head span,
  .you-are-here {
    font-size: 11px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }
  .eyebrow,
  .you-are-here {
    color: var(--ok-fg);
  }
  .close {
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
    border: 1px solid var(--border);
    background: transparent;
    color: var(--muted);
  }
  .close:focus-visible { outline: 2px solid var(--accent); outline-offset: 3px; }
  .phase-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(100%, 250px), 1fr));
    gap: 10px;

    padding-bottom: 6px;
  }
  .phase {
    border: 1px solid var(--border);
    background: var(--surface-2);
    padding: 10px;
  }
  .phase.current {
    border-color: var(--ok-fg);
  }
  .phase-head {
    display: flex;
    justify-content: space-between;
    color: var(--fg);
    font-size: 14px;
  }
  .phase-head span {
    color: var(--muted);
  }
  .you-are-here {
    display: block;
    margin-top: 3px;
  }
  ol {
    list-style: none;
    padding: 0;
    margin: 10px 0 0;
    display: grid;
    gap: 7px;
  }
  li {
    display: grid;
    grid-template-columns: 16px 1fr;
    gap: 6px;
    padding: 8px;
    border-left: 2px solid var(--border);
    background: var(--surface);
  }
  li.complete {
    border-left-color: var(--ok-fg);
  }
  li.elective {
    border-left-style: dashed;
  }
  .state {
    color: var(--muted);
    padding-top: 1px;
  }
  li.complete .state {
    color: var(--ok-fg);
  }
  li strong {
    display: block;
    color: var(--fg);
    font-size: 14px;
    line-height: 1.35;
  }
  li p {
    color: var(--muted);
    font-size: 11px;
    line-height: 1.45;
    margin: 4px 0 6px;
  }
  .meta {
    color: var(--muted);
    display: block;
    text-transform: none;
    letter-spacing: normal;
    font-family: var(--font-body);
    font-size: 12px;
    line-height: 1.5;
  }
</style>
