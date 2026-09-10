<script lang="ts">
  import { tick } from 'svelte';
  import type { PathRecommendation } from '../../contracts/placement';
  import NodeCard from '../../components/NodeCard.svelte';
  import FlowStage from '../../components/FlowStage.svelte';
  import { Compass, Route, Flag, ArrowLeft } from 'lucide-svelte';
  let { path, onclose, onfoundations, onaccept, accepting = false, error = '', acceptedRevision, embedded = false }: { path: PathRecommendation; onclose: () => void; onfoundations: () => void; onaccept?: () => void; accepting?: boolean; error?: string; acceptedRevision?: number; embedded?: boolean } = $props();
  $effect(() => { path.id; if (embedded) return; void tick().then(() => document.getElementById('desk-content')?.scrollTo(0, 0)); });
</script>
<section class="path" class:embedded aria-label="Suggested learning path">
  {#if !embedded || !acceptedRevision}<button class="ghost mono-ghost" onclick={onclose} disabled={accepting}><ArrowLeft size={13} /> {acceptedRevision ? 'Back to classes' : 'Change starting point'}</button>{/if}
  <header><p class="eyebrow mono">PERSONAL PATH · {acceptedRevision ? `ACCEPTED REVISION ${acceptedRevision}` : 'PREVIEW'}</p><h2>Begin at {path.entry_label}</h2><p>{path.explanation}</p></header>
  <FlowStage number="01"><NodeCard Icon={Compass} name="starting-point" badge={path.route} badgeTone="violet"><p class="start">{path.entry_label}</p>
    {#if path.criteria.length}<p>{path.criteria.filter((c) => c.verdict === 'passed').length} of {path.criteria.length} sampled criteria demonstrated. These are entry samples, separate from lesson completion and mastery.</p>{/if}
    <p class="muted"><strong>Still unassessed:</strong> {path.unknown_areas.join('; ')}</p>
  </NodeCard></FlowStage>
  <FlowStage number="02"><NodeCard Icon={Route} name="prerequisite-bridges" badge={String(path.refreshers.length)} badgeTone="amber">
    {#if path.refreshers.length}<p>Check these prerequisites before work that depends on them.</p><ul>{#each path.refreshers as item}<li><strong>{item.label}</strong><span>{item.reason}</span></li>{/each}</ul>{:else}<p>No earlier prerequisite gaps were identified by this route. Unsampled skills remain unknown.</p>{/if}
    {#if path.earlier_topics.length}<details><summary>Earlier material available to revisit · {path.earlier_topics.length}</summary><ul>{#each path.earlier_topics as item}<li><strong>{item.label}</strong><span>{item.reason}</span></li>{/each}</ul></details>{/if}
  </NodeCard></FlowStage>
  <FlowStage number="03" last><NodeCard Icon={Flag} name="required-outcome" badge="retained" badgeTone="teal"><p>{path.required_outcome}</p><p class="muted">The course's final assessment or capstone remains required. A starting preference or short diagnostic does not complete the goal.</p></NodeCard></FlowStage>
  <footer><p>{acceptedRevision ? 'This accepted path guides future lessons. Saved lessons and their original results remain in your history.' : 'Accepting saves this path. A saved study time is required to activate the class. Existing work and schedules are preserved.'}</p>{#if error}<p class="activation-error" role="alert">{error}</p>{/if}<div class="actions"><button class="ghost mono-ghost" onclick={onclose} disabled={accepting}>{acceptedRevision ? (embedded ? 'Overview' : 'Back to classes') : 'Adjust setup'}</button><button class="ghost mono-ghost" onclick={onfoundations} disabled={accepting}>{acceptedRevision ? 'Revise starting point' : 'Include foundations'}</button>{#if onaccept}<button class="cta mono-cta" onclick={onaccept} disabled={accepting}>{accepting ? 'Saving path…' : 'Accept learning path'}</button>{/if}</div></footer>
</section>
<style>
  .activation-error { color: var(--led-err); }
  .path { width: min(820px, 100%); margin: auto; padding: 26px 28px 48px; }
  header { margin: 24px 0; } h2 { font: 30px var(--font-display); margin: 8px 0; }
  header p, p, li { font-size: 13px; line-height: 1.65; }
  header p, .muted, li span { color: var(--muted); }
  .eyebrow { font-size: 10px; color: var(--faint); letter-spacing: .7px; }
  .start { font: 22px var(--font-display); margin-top: 0; }
  ul { padding-left: 18px; } li { margin: 12px 0; } li span { display: block; font-size: 12px; }
  summary { cursor: pointer; font-size: 12px; color: var(--violet-fg); }
  footer { margin-top: 22px; color: var(--muted); } footer p { font: 11px/1.7 var(--font-mono); border-left: 2px solid var(--violet); padding-left: 12px; }
  .actions { display: flex; flex-wrap: wrap; gap: 10px; justify-content: flex-end; }
  @media(max-width:620px) { .path { padding: 20px 16px; } }
  .path.embedded { width: 100%; max-width: 1000px; padding: 0; } .embedded header { margin-top: 0; }
</style>
