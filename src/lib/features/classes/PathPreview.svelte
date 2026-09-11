<script lang="ts">
  import { tick } from 'svelte';
  import type { PathRecommendation, PathTopic } from '../../contracts/placement';
  import { Compass, Route, Flag, ArrowLeft, ArrowRight, Layers } from 'lucide-svelte';
  let { path, onclose, onfoundations, onaccept, accepting = false, error = '', acceptedRevision, embedded = false }: { path: PathRecommendation; onclose: () => void; onfoundations: () => void; onaccept?: () => void; accepting?: boolean; error?: string; acceptedRevision?: number; embedded?: boolean } = $props();
  $effect(() => { path.id; if (embedded) return; void tick().then(() => document.getElementById('desk-content')?.scrollTo(0, 0)); });

  const passed = $derived(path.criteria.filter((c) => c.verdict === 'passed').length);
  const sampled = $derived(path.criteria.length);
  const ROUTE_SOURCE: Record<string, string> = {
    diagnostic: 'a short check you took',
    manual: 'the stage you chose yourself',
    foundations: 'your choice to begin at the beginning',
  };
  /** The concrete reasons this route looks the way it does, strongest first. */
  const reasons = $derived([
    sampled ? `You demonstrated ${passed} of ${sampled} sampled criteria, so lessons begin after the material those samples cover.` : '',
    path.earlier_topics.length ? `${path.earlier_topics.length} earlier ${path.earlier_topics.length === 1 ? 'topic is' : 'topics are'} treated as already covered. Nothing is deleted: you can pull any of them back in.` : '',
    path.refreshers.length ? `${path.refreshers.length} prerequisite ${path.refreshers.length === 1 ? 'refresher is' : 'refreshers are'} queued before the work that depends on them.` : 'No prerequisite gaps were found in what the check sampled.',
  ].filter(Boolean));

  /** Long lists page instead of scrolling: six cards at a time. */
  const SIZE = 6;
  let refresherPage = $state(0);
  let earlierPage = $state(0);
  function pageOf(items: PathTopic[], page: number) {
    const pages = Math.max(1, Math.ceil(items.length / SIZE));
    const current = Math.max(0, Math.min(page, pages - 1));
    return { items: items.slice(current * SIZE, current * SIZE + SIZE), pages, current };
  }
  const refreshers = $derived(pageOf(path.refreshers, refresherPage));
  const earlier = $derived(pageOf(path.earlier_topics, earlierPage));
</script>

<section class="path" class:embedded aria-label="Suggested learning path">
  {#if !embedded || !acceptedRevision}<button class="ghost mono-ghost" onclick={onclose} disabled={accepting}><ArrowLeft size={13} /> {acceptedRevision ? 'Back to classes' : 'Change starting point'}</button>{/if}

  <header>
    <p class="eyebrow mono">PERSONAL PATH · {acceptedRevision ? `ACCEPTED REVISION ${acceptedRevision}` : 'PREVIEW'} · FROM {(ROUTE_SOURCE[path.route] ?? path.route).toUpperCase()}</p>
    <h2>Your lessons start at <em>{path.entry_label}</em></h2>
    <p class="lead">{path.explanation}</p>
  </header>

  <section class="why" aria-label="Why this route">
    <h3>Why here</h3>
    <ol>{#each reasons as reason (reason)}<li>{reason}</li>{/each}</ol>
  </section>

  <div class="cards">
    <article class="card start">
      <p class="card-head mono"><Compass size={13} /> START HERE</p>
      <h4>{path.entry_label}</h4>
      <p>{path.earlier_topics.length ? `Everything before this point is set aside, and the ${path.refreshers.length} refresher${path.refreshers.length === 1 ? '' : 's'} below come first.` : 'This is where the first lesson begins.'}</p>
      {#if sampled}<p class="meter-line"><strong>{passed}/{sampled}</strong> sampled criteria demonstrated</p>
        <div class="meter" role="img" aria-label={`${passed} of ${sampled} sampled criteria demonstrated`}><i style:width={`${(passed / sampled) * 100}%`}></i></div>
        <p class="fine">Entry samples only. They are not lesson completion and not mastery.</p>{/if}
    </article>

    <article class="card">
      <p class="card-head mono"><Route size={13} /> REFRESHERS · {path.refreshers.length}</p>
      {#if path.refreshers.length}
        <p class="fine">Taken before the work that depends on them, because a sample for each was not demonstrated.</p>
        <ul class="topic-grid">{#each refreshers.items as item (item.id)}<li><strong>{item.label}</strong><span>{item.reason}</span></li>{/each}</ul>
        {#if refreshers.pages > 1}<div class="pager"><button class="ghost mono-ghost" aria-label="Previous refreshers" disabled={refreshers.current === 0} onclick={() => refresherPage = refreshers.current - 1}><ArrowLeft size={12} /></button><span class="mono">{refreshers.current + 1} / {refreshers.pages}</span><button class="ghost mono-ghost" aria-label="More refreshers" disabled={refreshers.current + 1 >= refreshers.pages} onclick={() => refresherPage = refreshers.current + 1}><ArrowRight size={12} /></button></div>{/if}
      {:else}<p class="fine">None queued. Skills the check did not sample stay unknown rather than assumed.</p>{/if}
    </article>

    <article class="card">
      <p class="card-head mono"><Layers size={13} /> SET ASIDE · {path.earlier_topics.length}</p>
      {#if path.earlier_topics.length}
        <p class="fine">Available whenever you want them. Pull one back from the Curriculum tab.</p>
        <ul class="topic-grid">{#each earlier.items as item (item.id)}<li><strong>{item.label}</strong><span>{item.reason}</span></li>{/each}</ul>
        {#if earlier.pages > 1}<div class="pager"><button class="ghost mono-ghost" aria-label="Previous earlier topics" disabled={earlier.current === 0} onclick={() => earlierPage = earlier.current - 1}><ArrowLeft size={12} /></button><span class="mono">{earlier.current + 1} / {earlier.pages}</span><button class="ghost mono-ghost" aria-label="More earlier topics" disabled={earlier.current + 1 >= earlier.pages} onclick={() => earlierPage = earlier.current + 1}><ArrowRight size={12} /></button></div>{/if}
      {:else}<p class="fine">Nothing is set aside: this route starts at the beginning.</p>{/if}
    </article>

    <article class="card">
      <p class="card-head mono"><Flag size={13} /> STILL REQUIRED</p>
      <p>{path.required_outcome}</p>
      <p class="fine">The final assessment stays required whatever route you take. A starting choice or a short check never completes it.</p>
    </article>
  </div>

  {#if path.unknown_areas.length}
    <details class="unmeasured">
      <summary>What this route has not measured · {path.unknown_areas.length}</summary>
      <p class="fine">Unknown, not failed. Lessons cover these as you reach them, and a unit challenge can check any of them early.</p>
      <ul>{#each path.unknown_areas as area (area)}<li>{area}</li>{/each}</ul>
    </details>
  {/if}

  <footer>
    <p>{acceptedRevision ? 'This accepted path guides future lessons. Saved lessons and their original results remain in your history.' : 'Accepting saves this path. A saved study time is required to activate the class. Existing work and schedules are preserved.'}</p>
    {#if error}<p class="activation-error" role="alert">{error}</p>{/if}
    <div class="actions">
      <button class="ghost mono-ghost" onclick={onclose} disabled={accepting}>{acceptedRevision ? (embedded ? 'Overview' : 'Back to classes') : 'Adjust setup'}</button>
      <button class="ghost mono-ghost" onclick={onfoundations} disabled={accepting}>{acceptedRevision ? 'Revise starting point' : 'Include foundations'}</button>
      {#if onaccept}<button class="cta mono-cta" onclick={onaccept} disabled={accepting}>{accepting ? 'Saving path…' : 'Accept learning path'}</button>{/if}
    </div>
  </footer>
</section>

<style>
  .activation-error { color: var(--led-err); }
  .path { width: min(980px, 100%); margin: auto; padding: 26px 28px 48px; }
  header { margin: 20px 0 16px; } h2 { font: 30px var(--font-display); margin: 8px 0; } h2 em { font-style: normal; color: var(--accent); }
  .eyebrow { font-size: 10px; color: var(--faint); letter-spacing: .7px; }
  .lead { font-size: 13px; line-height: 1.65; color: var(--muted); max-width: 70ch; }

  .why { border-left: 2px solid var(--violet); padding: 2px 0 2px 14px; margin-bottom: 22px; }
  .why h3 { font-size: 12px; letter-spacing: .6px; text-transform: uppercase; color: var(--violet-fg); margin: 0 0 8px; }
  .why ol { margin: 0; padding-left: 18px; } .why li { font-size: 13px; line-height: 1.7; margin: 4px 0; }

  .cards { display: grid; grid-template-columns: repeat(auto-fit, minmax(260px, 1fr)); gap: 14px; }
  .card { border: 1px solid var(--node-border); border-radius: var(--radius-panel); background: var(--surface); padding: 14px 16px; display: flex; flex-direction: column; gap: 8px; }
  .card.start { border-color: var(--accent); }
  .card-head { display: flex; align-items: center; gap: 7px; font-size: 10px; letter-spacing: .7px; color: var(--faint); margin: 0; }
  .card h4 { font: 20px var(--font-display); margin: 0; }
  .card p { font-size: 12px; line-height: 1.6; margin: 0; }
  .fine { color: var(--muted); font-size: 11px !important; }
  .meter-line { color: var(--muted); } .meter-line strong { color: var(--fg); font-size: 15px; }
  .meter { height: 4px; border-radius: 2px; background: var(--bg); overflow: hidden; } .meter i { display: block; height: 100%; background: var(--accent); }

  .topic-grid { list-style: none; margin: 0; padding: 0; display: grid; gap: 8px; }
  .topic-grid li { border-top: 1px solid var(--node-divider); padding-top: 8px; }
  .topic-grid strong { display: block; font-size: 12px; font-weight: 500; }
  .topic-grid span { display: block; color: var(--muted); font-size: 11px; line-height: 1.55; margin-top: 2px; }
  .pager { display: flex; align-items: center; justify-content: flex-end; gap: 8px; margin-top: auto; padding-top: 6px; }
  .pager span { font-size: 10px; color: var(--muted); }

  .unmeasured { margin-top: 18px; border: 1px dashed var(--node-border); border-radius: var(--radius-panel); padding: 12px 16px; }
  .unmeasured summary { cursor: pointer; font-size: 12px; color: var(--violet-fg); }
  .unmeasured ul { margin: 8px 0 0; padding-left: 18px; } .unmeasured li { font-size: 12px; line-height: 1.6; color: var(--muted); }

  footer { margin-top: 22px; color: var(--muted); } footer p { font: 11px/1.7 var(--font-mono); border-left: 2px solid var(--violet); padding-left: 12px; }
  .actions { display: flex; flex-wrap: wrap; gap: 10px; justify-content: flex-end; margin-top: 14px; }
  @media(max-width:620px) { .path { padding: 20px 16px; } }
  .path.embedded { width: 100%; max-width: 1000px; padding: 0; } .embedded header { margin-top: 0; }
</style>
