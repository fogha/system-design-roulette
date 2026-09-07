<script lang="ts">
  import type { CurriculumMapView, CurriculumPhase } from '../ipc';
  import { CheckCircle2, Circle, Clock3, Link2, X } from 'lucide-svelte';

  let {
    map,
    onclose,
  }: {
    map: CurriculumMapView;
    onclose: () => void;
  } = $props();

  const phases: { id: CurriculumPhase; label: string }[] = [
    { id: 'foundations', label: 'Foundations' },
    { id: 'mechanisms', label: 'Mechanisms' },
    { id: 'production', label: 'Production' },
    { id: 'synthesis', label: 'Synthesis' },
    { id: 'elective', label: 'Electives' },
  ];

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
      <span class="eyebrow mono">30-SESSION CURRICULUM · {map.completed_sessions} COMPLETE</span>
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
                    · <Link2 size={10} /> {concept.prerequisites.join(', ')}
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
    border: 1px solid rgba(62, 207, 142, 0.22);
    background: rgba(5, 13, 24, 0.96);
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
    color: var(--text-primary, #eef4ff);
    font-size: 18px;
  }
  header p {
    margin: 0;
    max-width: 920px;
    color: var(--text-secondary, #99a8bd);
    line-height: 1.55;
    font-size: 12px;
  }
  .eyebrow,
  .meta,
  .phase-head span,
  .you-are-here {
    font-size: 9px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }
  .eyebrow,
  .you-are-here {
    color: #3ecf8e;
  }
  .close {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border: 1px solid rgba(139, 159, 184, 0.25);
    background: transparent;
    color: #9aacbf;
  }
  .phase-grid {
    display: grid;
    grid-template-columns: repeat(5, minmax(210px, 1fr));
    gap: 10px;
    overflow-x: auto;
    padding-bottom: 6px;
  }
  .phase {
    border: 1px solid rgba(139, 159, 184, 0.16);
    background: rgba(10, 21, 36, 0.72);
    padding: 10px;
  }
  .phase.current {
    border-color: rgba(62, 207, 142, 0.52);
  }
  .phase-head {
    display: flex;
    justify-content: space-between;
    color: #dce7f4;
    font-size: 12px;
  }
  .phase-head span {
    color: #7890a9;
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
    border-left: 2px solid rgba(139, 159, 184, 0.22);
    background: rgba(4, 11, 20, 0.58);
  }
  li.complete {
    border-left-color: #3ecf8e;
  }
  li.elective {
    border-left-style: dashed;
  }
  .state {
    color: #73869c;
    padding-top: 1px;
  }
  li.complete .state {
    color: #3ecf8e;
  }
  li strong {
    display: block;
    color: #d6e1ed;
    font-size: 10px;
    line-height: 1.35;
  }
  li p {
    color: #8799ae;
    font-size: 9px;
    line-height: 1.45;
    margin: 4px 0 6px;
  }
  .meta {
    color: #657a91;
    display: inline-flex;
    align-items: center;
    gap: 3px;
  }
</style>
