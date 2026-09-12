<script lang="ts">
  /**
   * The class builder: a learner's own class from a brief to a published
   * course, on a five-step rail. Brief (what you want to be able to do),
   * Draft (the tutor writes it, you write it, or a file), Review (edit
   * until it reads right), Verify (the tutor reads it back; the sources
   * are fetched), Enroll (publish and set a starting point).
   */
  import { onMount, untrack } from 'svelte';
  import { api, type CourseBrief, type CourseDraft, type CustomCourseView } from '../../../ipc';
  import type { RunnerInfo } from '../../../contracts/agents';
  import { app } from '../../../stores.svelte';
  import { validateDraft } from '../../../custom-preview';
  import Dropdown from '../../../components/Dropdown.svelte';
  import ModelPicker from '../../../components/ModelPicker.svelte';
  import CurriculumEditor from './CurriculumEditor.svelte';
  import { ArrowLeft, ArrowRight, Bot, Check, FileJson, FileUp, PenLine, Rocket, ShieldCheck, Sparkles, Trash2, Globe, CircleAlert, CircleCheck, CircleDashed, X, Save, ListChecks } from 'lucide-svelte';

  let { id = null, onclose, onpublished }: { id?: string | null; onclose: () => void; onpublished: (id: string) => void } = $props();

  const STEPS = [
    { key: 'brief', title: 'Brief', Icon: Sparkles, ahead: 'what you want' },
    { key: 'draft', title: 'Draft', Icon: Bot, ahead: 'tutor, hand or file' },
    { key: 'review', title: 'Review', Icon: PenLine, ahead: 'edit the topics' },
    { key: 'verify', title: 'Verify', Icon: ShieldCheck, ahead: 'read back, fetch' },
    { key: 'enroll', title: 'Enroll', Icon: Rocket, ahead: 'publish' },
  ] as const;
  type StepKey = (typeof STEPS)[number]['key'];
  let step = $state<StepKey>('brief');
  const stepIndex = $derived(STEPS.findIndex((s) => s.key === step));

  let view = $state<CustomCourseView | null>(null);
  let brief = $state<CourseBrief>({ title: '', outcome: '', background: '', trusted_hosts: [], agent: app.state?.agent ?? 'claude', model: app.state?.model ?? 'opus', custom_agent_bin: app.state?.custom_agent_bin ?? '' });
  let draft = $state<CourseDraft | null>(null);
  let hostInput = $state('');
  let runners = $state<RunnerInfo[]>([]);
  let busy = $state<'' | 'create' | 'draft' | 'review' | 'sources' | 'bank' | 'publish' | 'save' | 'import' | 'delete' | 'export'>('');
  let error = $state('');
  let saved = $state<'idle' | 'saving' | 'saved'>('idle');
  let fileInput = $state<HTMLInputElement | undefined>(undefined);
  let dismissed = $state<number[]>([]);
  let logTail = $derived(app.genLog.slice(-6));

  const runnerOptions = $derived([...runners].sort((a, b) => Number(b.available) - Number(a.available)).map((r) => ({ value: r.provider, label: r.label, description: `${r.kind === 'local' ? 'Local' : r.kind.toUpperCase()} · ${r.available ? 'configured' : 'setup needed'}` })));
  const briefReady = $derived(brief.title.trim().length > 0 && brief.outcome.trim().split(/\s+/).filter(Boolean).length >= 5);
  const issues = $derived(draft ? validateDraft(draft) : []);
  const drafted = $derived(!!draft && (draft.topics.length > 1 || draft.topics[0]?.slug !== 'first-topic' || !!draft.summary));
  const unreachable = $derived((view?.sources ?? []).filter((s) => s.state !== 'reachable'));
  const highFindings = $derived((view?.review ?? []).filter((f) => f.severity === 'high').length);
  const bankQuestions = $derived(view?.bank?.questions ?? []);
  const voided = $derived(bankQuestions.filter((q) => q.voided));
  const bankStale = $derived.by(() => { const d = draft; return !!d && bankQuestions.some((q) => !d.topics.some((t) => `${d.id}-${t.slug}` === q.competency)); });

  onMount(() => {
    api.listAgentRunners().then((list) => (runners = list)).catch(() => {});
    if (id) void load(id);
  });

  async function load(courseId: string) {
    try {
      const loaded = await api.getCustomCourse(courseId);
      adopt(loaded);
      step = untrack(() => (drafted ? 'review' : 'draft'));
    } catch (cause) { error = String(cause); }
  }
  function adopt(next: CustomCourseView) {
    view = next;
    brief = { ...next.brief };
    draft = structuredClone(next.draft);
  }

  function addHost() {
    const host = hostInput.trim().replace(/^https?:\/\//, '').replace(/\/.*$/, '').toLowerCase();
    if (host && !brief.trusted_hosts.includes(host)) brief.trusted_hosts = [...brief.trusted_hosts, host];
    hostInput = '';
  }

  /** Leaving the brief creates the class, or saves the brief of an existing one. */
  async function leaveBrief() {
    if (!briefReady || busy) return;
    busy = 'create'; error = '';
    try {
      addHost();
      if (!view) adopt(await api.createCustomCourse($state.snapshot(brief), 'manual'));
      else adopt(await api.saveCustomCourseBrief(view.id, $state.snapshot(brief)));
      step = 'draft';
    } catch (cause) { error = String(cause); } finally { busy = ''; }
  }

  async function askTutor() {
    if (!view || busy) return;
    busy = 'draft'; error = '';
    try { adopt(await api.draftCustomCourse(view.id)); step = 'review'; }
    catch (cause) { error = String(cause); } finally { busy = ''; }
  }
  function writeByHand() { step = 'review'; }
  async function importFile(event: Event) {
    const file = (event.currentTarget as HTMLInputElement).files?.[0];
    if (!file || busy) return;
    busy = 'import'; error = '';
    try {
      const text = await file.text();
      const imported = await api.importCustomCourse(text);
      // The import is its own class; a blank one made for it is dropped.
      if (view && !drafted && view.status === 'draft') { try { await api.deleteCustomCourseDraft(view.id); } catch { /* keep going */ } }
      adopt(imported);
      step = 'review';
      await app.refresh();
    } catch (cause) { error = String(cause); } finally { busy = ''; if (fileInput) fileInput.value = ''; }
  }

  /** The draft is saved a moment after it stops changing. */
  let saveTimer: ReturnType<typeof setTimeout> | null = null;
  function changed() {
    saved = 'saving';
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => void save(), 700);
  }
  async function save() {
    if (!view || !draft) return;
    try {
      const next = await api.saveCustomCourseDraft(view.id, $state.snapshot(draft));
      view = next;
      saved = 'saved';
    } catch (cause) { error = String(cause); saved = 'idle'; }
  }
  async function flush() { if (saveTimer) { clearTimeout(saveTimer); saveTimer = null; } await save(); }

  async function review() {
    if (!view || busy) return;
    await flush();
    busy = 'review'; error = ''; dismissed = [];
    try { adopt(await api.reviewCustomCourse(view.id)); } catch (cause) { error = String(cause); } finally { busy = ''; }
  }
  async function verifySources() {
    if (!view || busy) return;
    await flush();
    busy = 'sources'; error = '';
    try { adopt(await api.verifyCustomCourseSources(view.id)); } catch (cause) { error = String(cause); } finally { busy = ''; }
  }
  async function writeBank() {
    if (!view || busy) return;
    await flush();
    busy = 'bank'; error = '';
    try { adopt(await api.writeCustomCourseBank(view.id)); } catch (cause) { error = String(cause); } finally { busy = ''; }
  }
  async function publish() {
    if (!view || busy) return;
    await flush();
    busy = 'publish'; error = '';
    try {
      const published = await api.publishCustomCourse(view.id);
      adopt(published);
      await app.refresh();
      app.notify(`${published.draft.label} is published as version ${published.version}.`);
      onpublished(published.id);
    } catch (cause) { error = String(cause); } finally { busy = ''; }
  }
  async function exportFile() {
    if (!view || busy) return;
    await flush();
    busy = 'export'; error = '';
    try {
      const result = await api.exportCustomCourse(view.id);
      app.notify(`Class file written: ${result.file_name}`, { label: 'Show file', run: () => void api.revealExport(result.path) });
    } catch (cause) { error = String(cause); } finally { busy = ''; }
  }
  async function discard() {
    if (!view || busy) return;
    if (!confirm(`Delete the draft "${view.draft.label || view.brief.title}"? This cannot be undone.`)) return;
    busy = 'delete'; error = '';
    try { await api.deleteCustomCourseDraft(view.id); await app.refresh(); onclose(); }
    catch (cause) { error = String(cause); } finally { busy = ''; }
  }
  function jumpTo(topic: string) {
    step = 'review';
    setTimeout(() => document.getElementById(`topic-${topic}`)?.scrollIntoView({ block: 'center', behavior: 'smooth' }), 60);
  }
  function go(next: StepKey) {
    if (next === 'brief') { step = 'brief'; return; }
    if (!view) return;
    if ((next === 'verify' || next === 'enroll') && !drafted) return;
    step = next;
  }
</script>

<div class="builder">
  <header class="top">
    <div>
      <span class="meta-label">{view?.status === 'published' ? `YOUR CLASS · VERSION ${view.version}` : 'NEW CLASS'}</span>
      <h2>{view?.draft.label || brief.title || 'A class of your own'}</h2>
    </div>
    <div class="top-actions">
      {#if view && draft}<span class="save-state mono" role="status">{saved === 'saving' ? 'saving…' : saved === 'saved' ? 'draft saved' : ''}</span>{/if}
      {#if view && view.status === 'draft'}<button type="button" class="ghost mono-ghost small danger" onclick={discard} disabled={!!busy}><Trash2 size={12} />Delete draft</button>{/if}
      <button type="button" class="ghost mono-ghost small" onclick={onclose} disabled={busy === 'publish'}><X size={12} />Close</button>
    </div>
  </header>

  <ol class="steps" aria-label="Builder steps">
    {#each STEPS as item, index (item.key)}
      {@const Icon = item.Icon}
      <li class:done={index < stepIndex} class:current={index === stepIndex} class:ahead={index > stepIndex}>
        <button type="button" disabled={index > stepIndex && !(view && drafted)} aria-current={index === stepIndex ? 'step' : undefined} onclick={() => go(item.key)}>
          <span class="step-tile">{#if index < stepIndex}<Check size={14} strokeWidth={2.4} />{:else}<Icon size={15} />{/if}</span>
          <span class="step-text"><span class="step-no mono">{index < stepIndex ? 'done' : index === stepIndex ? 'now' : `0${index + 1}`}</span><span class="step-name">{item.title}</span><span class="step-sub">{item.ahead}</span></span>
        </button>
      </li>
    {/each}
  </ol>

  {#if error}<p class="error" role="alert"><CircleAlert size={12} /> {error}</p>{/if}

  {#if step === 'brief'}
    <section class="panel" aria-label="Brief">
      <div class="panel-head"><h3>What do you want to be able to do?</h3><p>The tutor designs the whole course from this, so say it the way you would to a friend who teaches. Everything here can be changed later.</p></div>
      <div class="brief-grid">
        <label class="field span-2"><span>Title</span><input bind:value={brief.title} placeholder="Rust for command-line tools" /></label>
        <label class="field span-2"><span>The outcome <small>what you will have made or be able to do at the end</small></span><textarea rows="3" bind:value={brief.outcome} placeholder="Build and ship a small CLI with clean error handling, tests and a release binary."></textarea></label>
        <label class="field span-2"><span>What you already know <small>optional; it decides where foundations start</small></span><textarea rows="2" bind:value={brief.background} placeholder="Comfortable in Python; never touched a systems language."></textarea></label>
        <div class="field span-2">
          <span>Documentation you trust <small>optional hostnames; the tutor adds the obvious ones</small></span>
          <div class="hosts">
            {#each brief.trusted_hosts as host (host)}<span class="host mono">{host}<button type="button" aria-label={`Remove ${host}`} onclick={() => (brief.trusted_hosts = brief.trusted_hosts.filter((h) => h !== host))}><X size={11} /></button></span>{/each}
            <input class="mono host-input" placeholder="doc.rust-lang.org" bind:value={hostInput} onkeydown={(e) => { if (e.key === 'Enter' || e.key === ',') { e.preventDefault(); addHost(); } }} onblur={addHost} />
          </div>
        </div>
        <div class="field"><span class="mono pick-label">TUTOR <small>drafts the course and teaches it</small></span><Dropdown label="Tutor" hideLabel value={brief.agent} options={runnerOptions} onchange={(value) => { brief.agent = value; brief.model = runners.find((r) => r.provider === value)?.saved_model ?? runners.find((r) => r.provider === value)?.default_model ?? brief.model; }} /></div>
        <div class="field"><span class="mono pick-label">MODEL</span>{#key brief.agent}<ModelPicker agent={brief.agent} bind:value={brief.model} hideLabel />{/key}</div>
      </div>
      <footer class="nav"><span class="hint mono">{briefReady ? 'ready' : 'a title and a sentence of outcome are needed'}</span><button type="button" class="cta mono-cta" disabled={!briefReady || !!busy} onclick={leaveBrief}>{busy === 'create' ? 'Saving…' : 'Continue'} <ArrowRight size={13} /></button></footer>
    </section>

  {:else if step === 'draft'}
    <section class="panel" aria-label="Draft">
      <div class="panel-head"><h3>Who writes the first draft?</h3><p>All three lead to the same editor. Nothing is published until the last step.</p></div>
      <div class="ways">
        <button type="button" class="way" class:busy={busy === 'draft'} disabled={!!busy} onclick={askTutor}>
          <span class="way-tile"><Bot size={20} /></span>
          <strong>Ask the tutor</strong>
          <small>{brief.agent} · {brief.model || 'runner default'} designs the stages and 12 to 36 topics with sources, held to the desk's checks.</small>
          {#if busy === 'draft'}<span class="working mono">drafting… this takes a minute or two</span>{/if}
        </button>
        <button type="button" class="way" disabled={!!busy} onclick={writeByHand}>
          <span class="way-tile"><PenLine size={20} /></span>
          <strong>Write it yourself</strong>
          <small>Start from an empty course with one topic. The editor says what each topic still needs.</small>
        </button>
        <button type="button" class="way" disabled={!!busy} onclick={() => fileInput?.click()}>
          <span class="way-tile"><FileUp size={20} /></span>
          <strong>Import a file</strong>
          <small>A <span class="mono">.principia-class.json</span> exported from this or another desk. It opens in the editor as a new class.</small>
          {#if busy === 'import'}<span class="working mono">reading…</span>{/if}
        </button>
        <input bind:this={fileInput} type="file" accept=".json,application/json" hidden onchange={importFile} />
      </div>
      {#if busy === 'draft' && logTail.length}
        <pre class="feed mono" aria-live="polite">{logTail.join('\n')}</pre>
      {/if}
      {#if drafted}<footer class="nav"><span class="hint mono">a draft exists · {draft?.topics.length} topics</span><button type="button" class="ghost mono-ghost" onclick={() => (step = 'review')}>Open the draft <ArrowRight size={12} /></button></footer>{/if}
      <footer class="nav back"><button type="button" class="ghost mono-ghost" onclick={() => (step = 'brief')} disabled={!!busy}><ArrowLeft size={12} />Back to the brief</button></footer>
    </section>

  {:else if step === 'review' && draft}
    <section class="panel bare" aria-label="Review">
      <div class="panel-head"><h3>Edit until it reads right</h3><p>The rail lists what the desk would refuse to publish. Cards open to every field the tutor teaches from.</p></div>
      <CurriculumEditor bind:draft nativeIssues={view?.issues ?? []} onchange={changed} />
      <footer class="nav">
        <button type="button" class="ghost mono-ghost" onclick={() => (step = 'draft')}><ArrowLeft size={12} />Back</button>
        <span class="hint mono">{issues.length ? `${issues.length} to fix before publishing` : 'ready'}</span>
        <button type="button" class="ghost mono-ghost" onclick={exportFile} disabled={!!busy}><FileJson size={12} />Export class file</button>
        <button type="button" class="cta mono-cta" onclick={async () => { await flush(); step = 'verify'; }}>Continue <ArrowRight size={13} /></button>
      </footer>
    </section>

  {:else if step === 'verify' && draft}
    <section class="panel" aria-label="Verify">
      <div class="panel-head"><h3>Two checks before you enroll</h3><p>Both are optional and both can be run again after edits. Neither changes the draft on its own.</p></div>
      <div class="checks">
        <div class="check-card">
          <div class="check-head"><span class="check-tile"><Bot size={16} /></span><div><strong>The tutor reads it back</strong><small>Looks for outcomes that cannot be observed, missing or wrong prerequisites, topics that are one, stages that jump, sources that do not support their topic.</small></div><button type="button" class="ghost mono-ghost" onclick={review} disabled={!!busy}>{busy === 'review' ? 'Reading…' : view?.review.length ? 'Read it again' : 'Ask for a review'}</button></div>
          {#if busy === 'review' && logTail.length}<pre class="feed mono">{logTail.join('\n')}</pre>{/if}
          {#if view?.review.length}
            <ol class="findings">
              {#each view.review as finding, i (i)}
                {#if !dismissed.includes(i)}
                  <li class={finding.severity}>
                    <span class="sev mono">{finding.severity}</span>
                    <div class="finding-body">
                      <p>{finding.message}</p>
                      {#if finding.fix}<p class="fix"><b>Proposed:</b> {finding.fix}</p>{/if}
                      <div class="finding-actions">{#if finding.topic}<button type="button" class="text" onclick={() => jumpTo(finding.topic)}>Open {finding.topic} <ArrowRight size={11} /></button>{/if}<button type="button" class="text quiet" onclick={() => (dismissed = [...dismissed, i])}>Dismiss</button></div>
                    </div>
                  </li>
                {/if}
              {/each}
            </ol>
          {:else if view && view.review.length === 0 && busy !== 'review'}
            <p class="hint">Not read back yet.</p>
          {/if}
        </div>
        <div class="check-card">
          <div class="check-head"><span class="check-tile"><Globe size={16} /></span><div><strong>Fetch every source</strong><small>The desk requests each primary source the way a lesson would. Unreachable ones are listed with their topic; a lesson that cannot fetch a source says so, as bundled lessons do.</small></div><button type="button" class="ghost mono-ghost" onclick={verifySources} disabled={!!busy}>{busy === 'sources' ? 'Fetching…' : view?.sources.length ? 'Fetch again' : 'Fetch sources'}</button></div>
          {#if view?.sources.length}
            <div class="source-summary mono"><span class="ok"><CircleCheck size={11} /> {view.sources.filter((s) => s.state === 'reachable').length} reachable</span><span class:warn={unreachable.length > 0}><CircleDashed size={11} /> {unreachable.length} to look at</span></div>
            {#if unreachable.length}
              <ul class="sources">
                {#each unreachable as check (check.url + check.topic)}
                  <li><span class="state mono" class:off={check.state === 'off-host'}>{check.state}</span><span class="url mono">{check.url}</span><button type="button" class="text" onclick={() => jumpTo(check.topic)}>{check.topic} <ArrowRight size={11} /></button></li>
                {/each}
              </ul>
            {/if}
          {:else if busy !== 'sources'}
            <p class="hint">Not fetched yet.</p>
          {/if}
        </div>
      </div>
      <div class="check-card bank-card">
        <div class="check-head"><span class="check-tile"><ListChecks size={16} /></span><div><strong>Write the question bank</strong><small>Three cited four-choice questions per stage, on the course's core topics. They power the placement check and unit challenges; without them the class starts from scratch or a stage you choose. Held to the same shape as the bundled banks.</small></div><button type="button" class="ghost mono-ghost" onclick={writeBank} disabled={!!busy || issues.length > 0} title={issues.length ? 'Fix the editor first' : ''}>{busy === 'bank' ? 'Writing…' : bankQuestions.length ? 'Write it again' : 'Write the bank'}</button></div>
        {#if busy === 'bank' && logTail.length}<pre class="feed mono">{logTail.join('\n')}</pre>{/if}
        {#if bankQuestions.length}
          <div class="source-summary mono"><span class="ok"><CircleCheck size={11} /> {bankQuestions.length - voided.length} usable</span>{#if voided.length}<span class="warn"><CircleAlert size={11} /> {voided.length} disputed</span>{/if}{#if bankStale}<span class="warn"><CircleAlert size={11} /> some questions point at topics no longer in the draft; write it again</span>{/if}</div>
          <ol class="bank-list">
            {#each bankQuestions as q (q.id)}
              <li class:voided={q.voided}><span class="stage mono">{q.entry_point}</span><div class="q"><p>{q.prompt}</p><small>key: {q.choices.find((c) => c.id === q.answer)?.text ?? q.answer}{#if q.source} · <span class="mono">{q.source}</span>{/if}{#if q.voided} · <b>disputed:</b> {q.void_reason || 'no reason given'}{/if}</small></div></li>
            {/each}
          </ol>
        {:else if busy !== 'bank'}
          <p class="hint">Not written yet. Optional; it can be written after the class is published, from its Curriculum tab.</p>
        {/if}
      </div>
      <footer class="nav">
        <button type="button" class="ghost mono-ghost" onclick={() => (step = 'review')}><ArrowLeft size={12} />Back to the editor</button>
        <span class="hint mono">{highFindings ? `${highFindings} high-severity finding${highFindings === 1 ? '' : 's'} open` : issues.length ? `${issues.length} to fix in the editor` : 'ready'}</span>
        <button type="button" class="cta mono-cta" onclick={() => (step = 'enroll')}>Continue <ArrowRight size={13} /></button>
      </footer>
    </section>

  {:else if step === 'enroll' && draft && view}
    <section class="panel" aria-label="Enroll">
      <div class="panel-head"><h3>{view.status === 'published' ? `Publish version ${view.version + 1}` : 'Publish and enroll'}</h3><p>{view.status === 'published' ? 'The class keeps its history and its accepted path; the new version is what lessons are planned from next.' : 'Publishing makes the class real: it joins your classes with a custom badge and the starting-point flow opens.'}</p></div>
      <ul class="manifest">
        <li><span class="mf-label mono">CLASS</span><strong>{draft.label}</strong><small>{draft.short_code} · {draft.native_label || 'your own course'}</small></li>
        <li><span class="mf-label mono">TOPICS</span><strong>{draft.topics.length}</strong><small>{draft.topics.filter((t) => t.curriculum.core).length} core · {draft.entry_points.map((e) => e.label).join(' → ')}</small></li>
        <li><span class="mf-label mono">TUTOR</span><strong>{brief.agent} / {brief.model || 'runner default'}</strong><small>from the brief; changeable in the class's Settings tab</small></li>
        <li><span class="mf-label mono">CHECKS</span><strong class:warn={issues.length > 0}>{issues.length ? `${issues.length} to fix` : 'all pass'}</strong><small>{view.review.length ? `${view.review.length} review finding${view.review.length === 1 ? '' : 's'}` : 'not read back'} · {view.sources.length ? `${unreachable.length} source${unreachable.length === 1 ? '' : 's'} to look at` : 'sources not fetched'} · {bankQuestions.length ? `${bankQuestions.length - voided.length} questions` : 'no question bank'}</small></li>
      </ul>
      <p class="after mono">after publishing: choose a starting point ({bankQuestions.length ? 'from scratch, a stage, or the placement check' : 'from scratch or a stage; the placement check needs the question bank'}) → add study times → the class activates</p>
      <footer class="nav">
        <button type="button" class="ghost mono-ghost" onclick={() => (step = 'verify')}><ArrowLeft size={12} />Back</button>
        <span class="hint mono">{issues.length ? 'the desk will refuse until the editor is clean' : 'ready'}</span>
        <button type="button" class="ghost mono-ghost" onclick={exportFile} disabled={!!busy}><Save size={12} />Export class file</button>
        <button type="button" class="cta mono-cta" onclick={publish} disabled={!!busy || issues.length > 0}><Rocket size={13} />{busy === 'publish' ? 'Publishing…' : view.status === 'published' ? `Publish v${view.version + 1}` : 'Publish and enroll'}</button>
      </footer>
    </section>
  {/if}
</div>

<style>
  .builder { display: flex; flex-direction: column; gap: 16px; padding: 22px 26px 40px; min-width: 0; }
  .top { display: flex; align-items: flex-start; justify-content: space-between; gap: 16px; }
  .top h2 { margin: 6px 0 0; font: 24px var(--font-display); }
  .top-actions { display: flex; align-items: center; gap: 8px; flex-wrap: wrap; justify-content: flex-end; }
  .save-state { font-size: 9px; letter-spacing: 0.6px; color: var(--faint); min-width: 70px; text-align: right; }
  .error { display: flex; align-items: center; gap: 7px; margin: 0; padding: 10px 12px; border-left: 2px solid var(--led-err); background: var(--surface); color: var(--led-err); font-size: 12px; overflow-wrap: anywhere; }

  /* The rail, as the setup wizard draws it. */
  .steps { display: flex; gap: 8px; list-style: none; margin: 0; padding: 0; }
  .steps li { position: relative; flex: 1; min-width: 0; padding-bottom: 12px; }
  .steps li::after { content: ''; position: absolute; left: 0; right: 0; bottom: 0; height: 2px; border-radius: 2px; background: var(--node-border); transition: background 0.3s; }
  .steps li.done::after { background: var(--accent); }
  .steps li.current::after { background: linear-gradient(90deg, var(--accent) 0 40%, var(--node-border) 100%); }
  .steps button { display: flex; align-items: flex-start; gap: 10px; width: 100%; padding: 0 6px 0 0; border: 0; background: transparent; color: var(--faint); text-align: left; cursor: pointer; }
  .steps button:disabled { cursor: default; }
  .step-tile { flex: none; display: grid; place-items: center; width: 34px; height: 34px; border: 1px solid var(--node-border); border-radius: 10px; background: var(--surface); color: var(--faint); transition: all 0.25s; }
  .steps .current .step-tile { border-color: var(--accent); background: linear-gradient(180deg, color-mix(in srgb, var(--accent) 22%, var(--surface)), var(--surface)); color: var(--accent); box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 16%, transparent); }
  .steps .done .step-tile { border-color: color-mix(in srgb, var(--led-ok) 55%, var(--node-border)); background: var(--ok-bg); color: var(--ok-fg); }
  .step-text { display: flex; flex-direction: column; gap: 1px; min-width: 0; padding-top: 1px; }
  .step-no { font-size: 8.5px; letter-spacing: 1.2px; text-transform: uppercase; color: var(--faint); } .steps .current .step-no { color: var(--accent); } .steps .done .step-no { color: var(--ok-fg); }
  .step-name { font-size: 13px; color: var(--faint); } .steps .current .step-name { color: var(--fg); font-weight: 500; } .steps .done .step-name { color: var(--muted); }
  .step-sub { font-size: 10.5px; color: var(--faint); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; } .steps .current .step-sub, .steps .done .step-sub { color: var(--muted); }

  .panel { display: flex; flex-direction: column; gap: 16px; padding: 18px; border: 1px solid var(--node-border); border-radius: var(--radius-panel); background: var(--node-bg); }
  .panel.bare { padding: 0; border: 0; background: transparent; }
  .panel-head h3 { margin: 0 0 6px; font: 19px var(--font-display); } .panel-head p { margin: 0; font-size: 12.5px; line-height: 1.6; color: var(--muted); max-width: 70ch; }
  .brief-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 14px 18px; } .span-2 { grid-column: 1 / -1; }
  .field { display: flex; flex-direction: column; gap: 6px; min-width: 0; } .field > span { font-size: 12px; color: var(--muted); } .field > span small { margin-left: 6px; color: var(--faint); font-size: 10px; }
  .pick-label { font-size: 9px; letter-spacing: 1.2px; color: var(--faint); }
  input, textarea { width: 100%; padding: 10px 12px; border: 1px solid var(--node-border); border-radius: var(--radius-control); background: var(--bg); color: var(--fg); font: 13px/1.5 var(--font-body); } textarea { resize: vertical; }
  input:focus, textarea:focus { outline: none; border-color: var(--accent); }
  .hosts { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; padding: 6px; border: 1px solid var(--node-border); border-radius: var(--radius-control); background: var(--bg); }
  .host { display: inline-flex; align-items: center; gap: 5px; padding: 4px 6px 4px 9px; border: 1px solid var(--node-border); border-radius: 6px; background: var(--surface); font-size: 10.5px; color: var(--fg); }
  .host button { display: grid; place-items: center; width: 16px; height: 16px; border: 0; border-radius: 3px; background: transparent; color: var(--faint); cursor: pointer; } .host button:hover { color: var(--led-err); }
  .host-input { flex: 1; min-width: 160px; border: 0; background: transparent; padding: 4px 6px; font-size: 11.5px; }
  .nav { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; padding-top: 14px; border-top: 1px dashed var(--node-divider); } .nav .cta { margin-left: auto; display: inline-flex; align-items: center; gap: 7px; } .nav.back { border-top: 0; padding-top: 0; }
  .hint { font-size: 9.5px; letter-spacing: 0.5px; color: var(--faint); }
  .ways { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 12px; }
  .way { position: relative; display: flex; flex-direction: column; align-items: flex-start; gap: 8px; padding: 16px; border: 1px solid var(--node-border); border-radius: var(--radius-panel); background: var(--surface); color: var(--fg); text-align: left; cursor: pointer; transition: border-color 0.15s, transform 0.12s; }
  .way:hover:not(:disabled) { border-color: color-mix(in srgb, var(--accent) 50%, var(--node-border)); transform: translateY(-1px); }
  .way:disabled { opacity: 0.6; cursor: default; } .way.busy { opacity: 1; border-color: var(--accent); }
  .way-tile { display: grid; place-items: center; width: 42px; height: 42px; border: 1px solid color-mix(in srgb, var(--accent) 35%, var(--node-border)); border-radius: 11px; background: linear-gradient(180deg, color-mix(in srgb, var(--accent) 18%, var(--surface)), var(--surface)); color: var(--accent); }
  .way strong { font-size: 13.5px; font-weight: 500; } .way small { font-size: 11px; line-height: 1.5; color: var(--muted); }
  .working { font-size: 9.5px; letter-spacing: 0.5px; color: var(--accent); animation: pulse 1.2s ease-in-out infinite; } @keyframes pulse { 50% { opacity: 0.4; } }
  .feed { margin: 0; padding: 10px 12px; max-height: 140px; overflow: hidden; border: 1px solid var(--node-border); border-radius: var(--radius-control); background: #0b0d10; color: var(--muted); font-size: 10.5px; line-height: 1.6; white-space: pre-wrap; }
  .checks { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 12px; }
  .check-card { display: flex; flex-direction: column; gap: 12px; padding: 14px; border: 1px solid var(--node-border); border-radius: var(--radius-panel); background: var(--surface); }
  .check-head { display: flex; gap: 12px; align-items: flex-start; flex-wrap: wrap; } .check-head > div { flex: 1; min-width: 180px; } .check-head strong { display: block; font-size: 13px; font-weight: 500; } .check-head small { display: block; margin-top: 4px; font-size: 11px; line-height: 1.5; color: var(--muted); }
  .check-tile { flex: none; display: grid; place-items: center; width: 36px; height: 36px; border-radius: 10px; background: var(--surface-2); color: var(--accent); }
  .findings { display: flex; flex-direction: column; gap: 8px; margin: 0; padding: 0; list-style: none; }
  .findings li { display: flex; gap: 10px; padding: 10px 12px; border: 1px solid var(--node-border); border-left-width: 3px; border-radius: var(--radius-control); background: var(--node-bg); }
  .findings li.high { border-left-color: var(--led-err); } .findings li.medium { border-left-color: var(--warn-fg); } .findings li.low { border-left-color: var(--node-border); }
  .sev { flex: none; font-size: 9px; letter-spacing: 0.8px; text-transform: uppercase; color: var(--muted); padding-top: 3px; width: 52px; }
  .finding-body { display: flex; flex-direction: column; gap: 4px; min-width: 0; } .finding-body p { margin: 0; font-size: 12px; line-height: 1.5; } .fix { color: var(--muted); font-size: 11.5px !important; } .fix b { color: var(--fg); font-weight: 500; }
  .finding-actions { display: flex; gap: 12px; margin-top: 2px; }
  .text { display: inline-flex; align-items: center; gap: 4px; padding: 0; border: 0; background: transparent; color: var(--accent); font: 11px var(--font-body); cursor: pointer; } .text.quiet { color: var(--faint); }
  .source-summary { display: flex; gap: 14px; font-size: 10px; letter-spacing: 0.5px; color: var(--muted); } .source-summary span { display: inline-flex; align-items: center; gap: 5px; } .source-summary .ok { color: var(--ok-fg); } .source-summary .warn { color: var(--warn-fg); }
  .sources { display: flex; flex-direction: column; gap: 5px; margin: 0; padding: 0; list-style: none; max-height: 260px; overflow-y: auto; }
  .sources li { display: flex; align-items: center; gap: 10px; font-size: 11px; } .state { flex: none; width: 76px; font-size: 9px; letter-spacing: 0.6px; color: var(--warn-fg); } .state.off { color: var(--led-err); } .url { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--muted); font-size: 10.5px; }
  .bank-card { grid-column: 1 / -1; }
  .bank-list { display: flex; flex-direction: column; gap: 5px; margin: 0; padding: 0; list-style: none; max-height: 320px; overflow-y: auto; }
  .bank-list li { display: flex; gap: 10px; padding: 8px 10px; border: 1px solid var(--node-border); border-radius: var(--radius-control); background: var(--node-bg); } .bank-list li.voided { opacity: 0.6; border-style: dashed; }
  .bank-list .stage { flex: none; width: 82px; font-size: 9px; letter-spacing: 0.6px; color: var(--accent); padding-top: 2px; }
  .bank-list .q { display: flex; flex-direction: column; gap: 3px; min-width: 0; } .bank-list .q p { margin: 0; font-size: 12px; line-height: 1.45; } .bank-list .q small { font-size: 10.5px; color: var(--muted); overflow-wrap: anywhere; } .bank-list .q small b { color: var(--warn-fg); font-weight: 500; }
  .manifest { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 10px; margin: 0; padding: 0; list-style: none; }
  .manifest li { display: flex; flex-direction: column; gap: 3px; padding: 12px 14px; border: 1px solid var(--node-border); border-radius: var(--radius-panel); background: var(--surface); }
  .mf-label { font-size: 9px; letter-spacing: 1.2px; color: var(--faint); } .manifest strong { font-size: 13px; font-weight: 500; } .manifest strong.warn { color: var(--warn-fg); } .manifest small { font-size: 11px; color: var(--muted); line-height: 1.45; }
  .after { margin: 0; font-size: 9.5px; letter-spacing: 0.4px; color: var(--faint); line-height: 1.6; }
  .danger { --key: var(--led-err); }
  @media (max-width: 900px) { .ways, .checks, .manifest, .brief-grid { grid-template-columns: 1fr; } .step-text { display: none; } .steps button { justify-content: center; padding: 0; } }
</style>
