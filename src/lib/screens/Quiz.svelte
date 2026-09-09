<script lang="ts">
  import { onMount } from 'svelte';
  import { api, type QuizQuestionView, type QuizRoundView } from '../ipc';
  import type { AssessmentWork } from '../contracts/assessments';
  import { assessmentEditor, type AssessmentEditorState } from '../features/assessments/work-editor';
  import { app } from '../stores.svelte';
  // Capture once: async work must keep the session that opened this screen.
  const sessionId = app.session?.session_id ?? '';
  import ClusterBar from '../components/ClusterBar.svelte';
  import NodeCard from '../components/NodeCard.svelte';
  import StatusLED from '../components/StatusLED.svelte';
  import Markdown from '../components/Markdown.svelte';
  import { Inbox, TriangleAlert, Zap, ArrowUp, Check } from 'lucide-svelte';

  let questions = $state<QuizQuestionView[]>([]);
  let idx = $state(0);
  let answer = $state('');
  let loading = $state(true);
  let grading = $state(false);
  let advancing = $state(false);
  let error = $state('');
  let work = $state<AssessmentEditorState | null>(null);
  let editor = $state<ReturnType<typeof assessmentEditor>>();
  let unsubscribe: (() => void) | undefined;
  let alive = false;
  const current = $derived(questions[idx]);
  const progress = $derived(questions.length ? (idx / questions.length) * 100 : 0);
  const isAudit = $derived(app.session?.session_type === 'pop_quiz');
  const needsRecovery = $derived(work?.status === 'error' || work?.status === 'conflict');
  $effect(() => { answer = current ? work?.responses[String(current.id)]?.answer ?? current.draft ?? '' : ''; });

  function initialWork(round: QuizRoundView): AssessmentWork {
    if (!round.round_id) throw new Error('The quiz has no saved round. Reload the session.');
    return { roundId: round.round_id, revision: round.revision, responses: Object.fromEntries(round.questions.map((q) => [String(q.id), { answer: q.draft ?? '', status: q.answered ? 'answered' : 'draft' }])) };
  }
  function firstUnanswered() {
    const index = questions.findIndex((q) => work?.responses[String(q.id)]?.status !== 'answered');
    idx = index < 0 ? questions.length : index;
  }
  async function load() {
    loading = true; error = '';
    try {
      const round = await api.getQuiz(sessionId);
      if (!alive) return;
      questions = round.questions;
      if (!questions.length) { await api.finishReview(sessionId); if (alive) await app.refresh(); return; }
      const initial = initialWork(round);
      editor = assessmentEditor(initial, (id, response, revision) => api.submitAnswer(sessionId, initial.roundId, revision, Number(id), response.answer, response.status === 'answered'));
      unsubscribe?.();
      unsubscribe = editor.subscribe((value) => { work = value; });
      firstUnanswered();
      if (work?.status === 'saving') void editor.flush();
    } catch (cause) { error = String(cause); }
    finally { loading = false; }
  }
  onMount(() => {
    alive = true; void load();
    return () => { alive = false; unsubscribe?.(); void editor?.flush(); };
  });
  function edit(value: string) {
    answer = value;
    if (current && editor) editor.edit(String(current.id), { answer: value, status: 'draft' });
  }
  async function grade() {
    if (!editor || !work || grading || needsRecovery) return;
    grading = true; error = '';
    try {
      if (!await editor.flush()) return;
      const saved = editor.snapshot();
      await api.finishQuiz(sessionId, saved.roundId, saved.revision);
      if (alive) await app.refresh();
    } catch (cause) { error = String(cause); }
    finally { grading = false; }
  }
  async function submit() {
    if (!current || !answer.trim() || !editor || advancing || grading || needsRecovery) return;
    advancing = true; error = '';
    try {
      editor.edit(String(current.id), { answer: answer.trim(), status: 'answered' });
      if (!await editor.flush() || !alive) return;
      idx += 1;
      if (idx >= questions.length) await grade();
    } finally { advancing = false; }
  }
  async function recover(keepLocal: boolean) {
    if (!editor) return;
    try {
      const round = await api.getQuiz(sessionId);
      if (!alive) return;
      await editor.resolve(initialWork(round), keepLocal);
      firstUnanswered(); error = '';
    } catch (cause) { error = String(cause); }
  }
</script>

<div class="quiz-wrap blueprint">
  <ClusterBar
    route={isAudit ? 'audit' : 'quiz'}
    status={isAudit ? 'surprise compliance check · no new topic today' : 'session locked'}
    tone="warn"
  />
  {#if loading}
    <div class="center"><StatusLED tone="pending" label="loading requests…" /></div>
  {:else if grading}
    <div class="center">
      <StatusLED tone="pending" label="grading in flight" />
      <p class="sub mono">free-text answers dispatched to agent-backend · rubric grading</p>
    </div>
  {:else if !questions.length && error}
    <div class="center"><p class="save-error" role="alert">{error}</p><button class="ghost mono-ghost" onclick={load}>Reload quiz</button></div>
  {:else if current}
    <div class="quiz-body">
      <div class="progress-track"><div class="progress-fill" style="width: {progress}%"></div></div>
      <div class="spacer"></div>
      {#if isAudit && app.session?.plan_reason}
        <div class="audit-note mono"><Zap size={11} /> POP QUIZ — {app.session.plan_reason}</div>
      {/if}
      <NodeCard
        Icon={Inbox}
        name={`incoming request — POST /quiz/${idx + 1} of ${questions.length}`}
        badge={current.origin === 'carryover' ? 'retry · from DLQ' : current.kind === 'mcq' ? 'multiple choice' : 'free text'}
        badgeTone={current.origin === 'carryover' ? 'amber' : 'muted'}
        accent={current.origin === 'carryover' ? 'var(--led-warn)' : 'var(--node-border)'}
      >
        {#snippet children()}
          {#if current.origin === 'carryover'}
            <div class="dlq-note mono"><TriangleAlert size={11} /> you failed this before — it returns until you pass it</div>
          {/if}
          <div class="prompt"><Markdown markdown={current.prompt} compact /></div>
          {#if current.kind === 'mcq' && current.choices}
            <div class="choices" role="radiogroup" aria-label="answer choices">
              {#each current.choices as choice, i}
                <button
                  class="choice"
                  role="radio"
                  aria-checked={answer === choice}
                  class:selected={answer === choice}
                  onclick={() => edit(choice)}
                  disabled={advancing || needsRecovery}
                >
                  <span class="choice-key mono">{String.fromCharCode(65 + i)}</span>
                  <Markdown markdown={choice} compact />
                </button>
              {/each}
            </div>
          {:else}
            <textarea rows="4" aria-label="Your answer" placeholder="2-4 sentences — graded against a rubric" value={answer} disabled={advancing || needsRecovery} oninput={(event) => edit(event.currentTarget.value)}></textarea>
          {/if}
          <div class="actions">
            {#if idx > 0}<button class="ghost mono-ghost" disabled={advancing} onclick={() => (idx -= 1)}>Previous answer</button>{/if}
            <button class="cta mono-cta" onclick={submit} disabled={!answer.trim() || advancing || needsRecovery}>
              <ArrowUp size={13} />{advancing ? 'saving…' : idx === questions.length - 1 ? 'send & grade all' : 'send response'}
            </button>
          </div>
        {/snippet}
      </NodeCard>
    </div>
  {:else if questions.length}
    <div class="quiz-body"><NodeCard Icon={Check} name="responses-saved" badge="ready" badgeTone="teal">
      <p>Every answer is saved. Submit this round for feedback.</p>
      <div class="actions"><button class="ghost mono-ghost" onclick={() => (idx = 0)}>Review answers</button><button class="cta mono-cta" onclick={grade} disabled={needsRecovery}>Grade saved answers</button></div>
    </NodeCard></div>
  {/if}
  {#if !loading && !grading && questions.length}
    <div class="save-state" role="status">
      {work?.status === 'saving' ? 'Saving this round…' : work?.status === 'saved' ? 'Answers saved on this device' : ''}
      {#if needsRecovery}<p class="save-error" role="alert">{work?.error} Your local draft is retained.</p><div class="recovery-actions">
        {#if work?.status === 'error'}<button class="ghost mono-ghost" onclick={() => editor?.flush()}>Retry save</button>{/if}
        <button class="ghost mono-ghost" onclick={() => recover(false)}>Use saved answers</button>
        {#if editor?.hasLocalChanges()}<button class="ghost mono-ghost" onclick={() => recover(true)}>Keep local answers</button>{/if}
      </div>{/if}
      {#if error}<p class="save-error" role="alert">{error}</p>{/if}
    </div>
  {/if}
</div>

<style>
  .save-state { width: min(760px, 92vw); margin: 0 auto; padding: 0 24px 20px; color: var(--muted); font: 11px/1.6 var(--font-mono); }
  .save-error { color: var(--bad-fg); }
  .recovery-actions { display: flex; flex-wrap: wrap; gap: 8px; }
  .quiz-wrap {
    flex: 1;
    display: flex;
    flex-direction: column;
    animation: fade-in 0.35s ease;
  }
  .center {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 14px;
  }
  .sub {
    color: var(--faint);
    font-size: 11px;
  }
  .quiz-body {
    width: min(760px, 92vw);
    margin: 0 auto;
    flex: 1;
    display: flex;
    flex-direction: column;
    justify-content: center;
    padding: 24px;
  }
  .progress-track {
    border-radius: var(--radius-detail);
    overflow: hidden;
  }
  .spacer {
    height: 18px;
  }
  .audit-note {
    font-size: 11px;
    color: var(--violet-fg);
    background: var(--violet-bg);
    border: 1px dashed var(--violet);
    border-radius: var(--radius-control);
    padding: 7px 12px;
    margin-bottom: 14px;
    align-self: flex-start;
  }
  .dlq-note {
    font-size: 11px;
    color: var(--warn-fg);
    background: var(--warn-bg);
    border-radius: var(--radius-control);
    padding: 6px 10px;
    margin-bottom: 12px;
    display: inline-block;
  }
  .prompt {
    font-size: 20px;
    line-height: 1.5;
    margin: 6px 0 18px;
    font-weight: 500;
    min-width: 0;
  }
  .choices {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .choice {
    text-align: left;
    background: var(--bg);
    border: 1px solid var(--node-border);
    color: var(--fg);
    border-radius: var(--radius-panel);
    padding: 12px 14px;
    font-family: var(--font-body);
    font-size: 14px;
    cursor: pointer;
    transition: border-color 0.15s ease;
    display: flex;
    gap: 12px;
    align-items: flex-start;
    min-width: 0;
    overflow: hidden;
  }
  .choice:hover {
    border-color: var(--muted);
  }
  .choice.selected {
    border-color: var(--accent);
    background: var(--surface-2);
  }
  .choice-key {
    font-size: 11px;
    color: var(--faint);
    border: 1px solid var(--node-border);
    border-radius: var(--radius-detail);
    padding: 1px 7px;
    flex-shrink: 0;
    margin-top: 2px;
  }
  .choice :global(.md) {
    flex: 1;
    min-width: 0;
  }
  .choice.selected .choice-key {
    color: var(--accent);
    border-color: var(--accent);
  }
  textarea {
    background: var(--bg);
    border-color: var(--node-border);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    justify-content: flex-end;
    margin-top: 16px;
  }
</style>
