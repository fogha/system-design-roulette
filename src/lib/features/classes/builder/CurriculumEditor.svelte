<script lang="ts">
  /**
   * The course as the learner edits it: the header that tells the tutor what
   * the course is and how to teach it, the four stages in the course's own
   * words, and the topics grouped by stage. Validation runs as they type;
   * what the desk objects to is listed beside the rail and on each card.
   */
  import type { CourseDraft, DraftIssue, DraftTopic } from '../../../ipc';
  import { blankTopic, STAGES, validateDraft } from '../../../custom-preview';
  import TopicCard from './TopicCard.svelte';
  import { Plus, X, CircleAlert, CircleCheck } from 'lucide-svelte';

  let {
    draft = $bindable(),
    nativeIssues = [],
    onchange,
  }: {
    draft: CourseDraft;
    /** What the desk said after the last save; the local mirror runs between saves. */
    nativeIssues?: DraftIssue[];
    onchange: () => void;
  } = $props();

  const issues = $derived(validateDraft(draft));
  const headerIssues = $derived(issues.filter((issue) => !issue.at.startsWith('topics/')));
  const topicIssues = (slug: string) => issues.filter((issue) => issue.at === `topics/${slug}`);
  const byStage = $derived(
    [...STAGES.map((stage) => ({ ...stage, label: draft.entry_points.find((e) => e.id === stage.id)?.label ?? stage.label })), { id: 'elective', label: 'Electives' }].map((stage) => ({
      ...stage,
      topics: draft.topics.map((topic, index) => ({ topic, index })).filter(({ topic }) => topic.curriculum.phase === stage.id),
    })),
  );
  let openSlug = $state<string | null>(null);
  let hostInput = $state('');

  function addHost() {
    const host = hostInput.trim().replace(/^https?:\/\//, '').replace(/\/.*$/, '').toLowerCase();
    if (host && !draft.source_hosts.includes(host)) draft.source_hosts = [...draft.source_hosts, host];
    hostInput = '';
    onchange();
  }
  function dropHost(host: string) { draft.source_hosts = draft.source_hosts.filter((h) => h !== host); onchange(); }
  function addTopic(phase: DraftTopic['curriculum']['phase']) {
    const topic = blankTopic(phase);
    topic.slug = `topic-${draft.topics.length + 1}`;
    draft.topics = [...draft.topics, topic];
    openSlug = topic.slug;
    onchange();
  }
  function removeTopic(index: number) {
    const gone = draft.topics[index]?.slug;
    draft.topics = draft.topics.filter((_, i) => i !== index).map((topic) => ({ ...topic, prereqs: topic.prereqs.filter((p) => p !== gone) }));
    onchange();
  }
  function moveTopic(index: number, direction: -1 | 1) {
    const target = index + direction;
    if (target < 0 || target >= draft.topics.length) return;
    const topics = [...draft.topics];
    [topics[index], topics[target]] = [topics[target], topics[index]];
    draft.topics = topics;
    onchange();
  }
  function jump(at: string) {
    const slug = at.replace(/^topics\//, '');
    openSlug = slug;
    document.getElementById(`topic-${slug}`)?.scrollIntoView({ block: 'center', behavior: 'smooth' });
  }
</script>

<div class="editor">
  <aside class="rail" aria-label="What still needs work">
    {#if issues.length}
      <span class="rail-label mono"><CircleAlert size={11} /> {issues.length} {issues.length === 1 ? 'THING' : 'THINGS'} TO FIX</span>
      <ul>
        {#each issues.slice(0, 24) as issue, i (i)}
          <li><button type="button" onclick={() => (issue.at.startsWith('topics/') ? jump(issue.at) : document.getElementById(`field-${issue.at}`)?.scrollIntoView({ block: 'center', behavior: 'smooth' }))}><span class="mono">{issue.at.replace('topics/', '')}</span>{issue.message}</button></li>
        {/each}
        {#if issues.length > 24}<li class="more mono">and {issues.length - 24} more</li>{/if}
      </ul>
    {:else}
      <span class="rail-label mono ok"><CircleCheck size={11} /> READY TO PUBLISH</span>
      <p>Every check passes. {draft.topics.length} topics across four stages.</p>
    {/if}
    {#if nativeIssues.length && !issues.length}
      <p class="native"><CircleAlert size={11} /> The desk still objects: {nativeIssues[0].message}</p>
    {/if}
    <dl class="counts mono">
      <div><dt>topics</dt><dd>{draft.topics.length}</dd></div>
      <div><dt>core</dt><dd>{draft.topics.filter((t) => t.curriculum.core).length}</dd></div>
      <div><dt>sources</dt><dd>{draft.topics.reduce((n, t) => n + t.curriculum.primary_sources.filter(Boolean).length, 0)}</dd></div>
      <div><dt>hosts</dt><dd>{draft.source_hosts.length}</dd></div>
    </dl>
  </aside>

  <div class="sheet">
    <section class="block" aria-label="The course">
      <span class="eyebrow mono">THE COURSE</span>
      <div class="grid">
        <label class="field span-2" id="field-label"><span>Class name <small>as it reads in the list</small></span><input value={draft.label} oninput={(e) => { draft.label = e.currentTarget.value; onchange(); }} /></label>
        <label class="field" id="field-short_code"><span>Short code <small>1–4 letters</small></span><input class="mono" maxlength="4" value={draft.short_code} oninput={(e) => { draft.short_code = e.currentTarget.value.toUpperCase(); onchange(); }} /></label>
        <label class="field" id="field-native_label"><span>Field <small>two or three words, lowercase</small></span><input value={draft.native_label} oninput={(e) => { draft.native_label = e.currentTarget.value; onchange(); }} placeholder="systems programming" /></label>
        <label class="field span-2" id="field-title"><span>Full title</span><input value={draft.title} oninput={(e) => { draft.title = e.currentTarget.value; onchange(); }} /></label>
        <label class="field span-2" id="field-summary"><span>Summary <small>one sentence</small></span><input value={draft.summary} oninput={(e) => { draft.summary = e.currentTarget.value; onchange(); }} /></label>
        <label class="field span-2" id="field-outcome"><span>Outcome <small>what you will have made and can defend at the end</small></span><textarea rows="3" value={draft.outcome} oninput={(e) => { draft.outcome = e.currentTarget.value; onchange(); }}></textarea></label>
        <label class="field span-2" id="field-context"><span>How it should be taught <small>the tutor reads this before every lesson</small></span><textarea rows="3" value={draft.context} oninput={(e) => { draft.context = e.currentTarget.value; onchange(); }}></textarea></label>
        <label class="field span-2" id="field-environment"><span>Working environment</span><input value={draft.environment} oninput={(e) => { draft.environment = e.currentTarget.value; onchange(); }} placeholder="A terminal, a text editor and …" /></label>
      </div>
      <div class="field" id="field-source_hosts">
        <span>Documentation hosts <small>every lesson is taught from and cites these</small></span>
        <div class="hosts">
          {#each draft.source_hosts as host (host)}<span class="host mono">{host}<button type="button" aria-label={`Remove ${host}`} onclick={() => dropHost(host)}><X size={11} /></button></span>{/each}
          <input class="mono host-input" placeholder="docs.example.org" bind:value={hostInput} onkeydown={(e) => { if (e.key === 'Enter' || e.key === ',') { e.preventDefault(); addHost(); } }} onblur={addHost} />
        </div>
      </div>
      {#if headerIssues.length}<ul class="issues">{#each headerIssues as issue (issue.at + issue.message)}<li><CircleAlert size={11} /> <span class="mono">{issue.at}</span> {issue.message}</li>{/each}</ul>{/if}
    </section>

    <section class="block" aria-label="Stages" id="field-entry_points">
      <span class="eyebrow mono">THE FOUR STAGES <small>in this course's own words</small></span>
      <div class="stages">
        {#each draft.entry_points as entry, i (entry.id)}
          <label class="field"><span class="mono">{String(i + 1).padStart(2, '0')} · {entry.id}</span><input value={entry.label} oninput={(e) => { draft.entry_points[i].label = e.currentTarget.value; onchange(); }} /></label>
        {/each}
      </div>
    </section>

    {#each byStage as stage (stage.id)}
      <section class="block stage" aria-label={stage.label}>
        <div class="stage-head">
          <span class="eyebrow mono">{stage.id === 'elective' ? 'ELECTIVES' : `STAGE · ${stage.label.toUpperCase()}`} <b>{stage.topics.length}</b></span>
          <button type="button" class="ghost mono-ghost small" onclick={() => addTopic(stage.id as DraftTopic['curriculum']['phase'])}><Plus size={12} />Add topic</button>
        </div>
        {#if stage.topics.length}
          <div class="topics">
            {#each stage.topics as { topic, index } (index)}
              <TopicCard
                bind:topic={draft.topics[index]}
                {index}
                stages={draft.entry_points}
                others={draft.topics}
                issues={topicIssues(topic.slug)}
                open={openSlug === topic.slug}
                onremove={() => removeTopic(index)}
                onmove={(direction) => moveTopic(index, direction)}
                onchange={() => { openSlug = draft.topics[index]?.slug ?? openSlug; onchange(); }}
              />
            {/each}
          </div>
        {:else}
          <p class="empty">{stage.id === 'elective' ? 'No electives. Optional.' : 'No topics in this stage yet; it needs at least one core topic.'}</p>
        {/if}
      </section>
    {/each}
  </div>
</div>

<style>
  .editor { display: grid; grid-template-columns: 220px minmax(0, 1fr); gap: 18px; align-items: start; }
  .rail { position: sticky; top: 8px; display: flex; flex-direction: column; gap: 10px; padding: 12px; border: 1px solid var(--node-border); border-radius: var(--radius-panel); background: var(--node-bg); }
  .rail-label { display: inline-flex; align-items: center; gap: 6px; font-size: 9px; letter-spacing: 1.2px; color: var(--warn-fg); } .rail-label.ok { color: var(--ok-fg); }
  .rail ul { display: flex; flex-direction: column; gap: 3px; margin: 0; padding: 0; list-style: none; max-height: 40vh; overflow-y: auto; }
  .rail li button { display: flex; flex-direction: column; gap: 1px; width: 100%; padding: 6px 8px; border: 1px solid transparent; border-radius: var(--radius-detail); background: transparent; color: var(--muted); font: 10.5px/1.4 var(--font-body); text-align: left; cursor: pointer; }
  .rail li button:hover { background: var(--surface); border-color: var(--node-border); color: var(--fg); }
  .rail li button .mono { font-size: 8.5px; letter-spacing: 0.5px; color: var(--accent); }
  .rail .more { font-size: 9px; color: var(--faint); padding: 4px 8px; }
  .rail p { margin: 0; font-size: 11px; line-height: 1.5; color: var(--muted); }
  .native { display: flex; gap: 6px; color: var(--warn-fg) !important; }
  .counts { display: grid; grid-template-columns: 1fr 1fr; gap: 6px; margin: 4px 0 0; padding-top: 10px; border-top: 1px dashed var(--node-divider); }
  .counts div { display: flex; justify-content: space-between; font-size: 9.5px; color: var(--muted); } .counts dd { margin: 0; color: var(--fg); }
  .sheet { display: flex; flex-direction: column; gap: 14px; min-width: 0; }
  .block { display: flex; flex-direction: column; gap: 12px; padding: 16px; border: 1px solid var(--node-border); border-radius: var(--radius-panel); background: var(--node-bg); }
  .eyebrow { font-size: 9px; letter-spacing: 1.3px; color: var(--accent); } .eyebrow small { margin-left: 8px; color: var(--faint); letter-spacing: 0.4px; } .eyebrow b { margin-left: 8px; color: var(--muted); }
  .grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 12px 16px; } .span-2 { grid-column: 1 / -1; }
  .field { display: flex; flex-direction: column; gap: 6px; min-width: 0; } .field > span { font-size: 11px; color: var(--muted); } .field > span small { margin-left: 6px; color: var(--faint); font-size: 10px; }
  input, textarea { width: 100%; padding: 9px 11px; border: 1px solid var(--node-border); border-radius: var(--radius-control); background: var(--bg); color: var(--fg); font: 12.5px/1.5 var(--font-body); }
  input.mono { font-family: var(--font-mono); font-size: 11.5px; } textarea { resize: vertical; }
  input:focus, textarea:focus { outline: none; border-color: var(--accent); }
  .hosts { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; padding: 6px; border: 1px solid var(--node-border); border-radius: var(--radius-control); background: var(--bg); }
  .host { display: inline-flex; align-items: center; gap: 5px; padding: 4px 6px 4px 9px; border: 1px solid var(--node-border); border-radius: 6px; background: var(--surface); font-size: 10.5px; color: var(--fg); }
  .host button { display: grid; place-items: center; width: 16px; height: 16px; border: 0; border-radius: 3px; background: transparent; color: var(--faint); cursor: pointer; } .host button:hover { color: var(--led-err); }
  .host-input { flex: 1; min-width: 160px; border: 0; background: transparent; padding: 4px 6px; }
  .issues { display: flex; flex-direction: column; gap: 4px; margin: 0; padding: 10px 12px; list-style: none; border-left: 2px solid var(--warn-fg); background: var(--surface); }
  .issues li { display: flex; align-items: center; gap: 7px; font-size: 11px; color: var(--warn-fg); } .issues li .mono { color: var(--muted); }
  .stages { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 10px; }
  .stages .field > span { font-size: 9px; letter-spacing: 0.6px; color: var(--faint); }
  .stage-head { display: flex; align-items: center; justify-content: space-between; gap: 10px; }
  .topics { display: flex; flex-direction: column; gap: 8px; }
  .empty { margin: 0; font-size: 11px; color: var(--faint); }
  @media (max-width: 900px) { .editor { grid-template-columns: 1fr; } .rail { position: static; } .stages { grid-template-columns: repeat(2, minmax(0, 1fr)); } }
  @media (max-width: 640px) { .grid { grid-template-columns: 1fr; } }
</style>
