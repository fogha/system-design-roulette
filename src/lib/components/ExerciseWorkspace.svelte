<script lang="ts">
  import { api, type ExerciseOwner, type ExerciseView } from '../ipc';
  import { createDraftSaver, type SaveStatus } from '../draft-save';
  import Markdown from './Markdown.svelte';
  import { Lightbulb, Copy, Check, RotateCcw } from 'lucide-svelte';

  let {
    courseId,
    classroomSessionId,
    studySessionId,
  }: { courseId?: number; classroomSessionId?: number; studySessionId?: string } = $props();

  let exercise = $state<ExerciseView | null>(null);
  let loading = $state(true);
  let loadError = $state('');
  let draft = $state('');
  let reflection = $state('');
  let completed = $state(false);
  let revealedHints = $state(0);
  let saveStatus = $state<SaveStatus>('idle');
  let completionStatus = $state<'idle' | 'saving' | 'saved' | 'error'>('idle');
  let completionError = $state('');
  let copied = $state(false);
  let saver: ReturnType<typeof createDraftSaver> | undefined;
  let loadVersion = 0;
  const owner: ExerciseOwner = $derived(
    studySessionId !== undefined
      ? { study_session_id: studySessionId }
      : classroomSessionId !== undefined
        ? { classroom_session_id: classroomSessionId }
        : { course_id: courseId },
  );

  $effect(() => {
    const capturedOwner = { ...owner };
    const ownerKey = studySessionId !== undefined
      ? `study:${studySessionId}`
      : classroomSessionId !== undefined
        ? `classroom:${classroomSessionId}` : `course:${courseId}`;
    const version = ++loadVersion;
    let disposed = false;
    const currentSaver = createDraftSaver(
      ownerKey,
      (text) => api.saveExerciseDraft(capturedOwner, text),
      (status) => { if (!disposed) saveStatus = status; },
    );
    saver = currentSaver;
    loading = true;
    loadError = '';
    exercise = null;
    revealedHints = 0;
    saveStatus = 'idle';
    completionStatus = 'idle';
    completionError = '';
    currentSaver.flush()
      .then(() => api.getExercise(capturedOwner))
      .then((e) => {
        if (disposed) return;
        exercise = e;
        const recovered = currentSaver.recoveredDraft();
        draft = recovered ?? e?.draft ?? e?.starter_code ?? '';
        reflection = e?.reflection ?? '';
        completed = e?.completed ?? false;
        loading = false;
        if (recovered !== null && e) currentSaver.schedule(recovered);
      })
      .catch((e) => {
        if (disposed) return;
        loadError = String(e);
        loading = false;
      });
    return () => {
      disposed = true;
      void currentSaver.flush();
      if (version === loadVersion) saver = undefined;
    };
  });

  function scheduleSave() {
    if (!exercise) return;
    saver?.schedule(draft);
  }

  function onDraftInput(e: Event) {
    draft = (e.target as HTMLTextAreaElement).value;
    scheduleSave();
  }

  function resetToStarter() {
    if (!exercise?.starter_code) return;
    draft = exercise.starter_code;
    scheduleSave();
  }

  async function copyStarter() {
    if (!exercise?.starter_code) return;
    try {
      await navigator.clipboard.writeText(exercise.starter_code);
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch {
      // Clipboard API unavailable — nothing to recover, ignore silently.
    }
  }

  function revealNextHint() {
    if (exercise && revealedHints < exercise.hints.length) revealedHints += 1;
  }

  async function saveCompletion(nextCompleted: boolean) {
    if (!exercise) return;
    const capturedOwner = { ...owner };
    const version = loadVersion;
    const savedReflection = reflection;
    completionError = '';
    if (nextCompleted && reflection.trim().split(/\s+/).filter(Boolean).length < 5) {
      completionStatus = 'error';
      completionError = 'Add a short note naming your evidence and the trade-off you chose.';
      return;
    }
    completionStatus = 'saving';
    try {
      await saver?.flush();
      if (version !== loadVersion) return;
      if (saveStatus === 'error') throw new Error('Save the exercise draft before marking it complete.');
      await api.saveExerciseCompletion(capturedOwner, nextCompleted, savedReflection);
      if (version !== loadVersion) return;
      completed = nextCompleted;
      completionStatus = 'saved';
    } catch (e) {
      if (version !== loadVersion) return;
      completionStatus = 'error';
      completionError = String(e);
    }
  }
</script>

<div class="exercise-workspace">
  {#if loading}
    <p class="mono dim" role="status">loading exercise…</p>
  {:else if loadError}
    <p class="mono err" role="alert">{loadError}</p>
  {:else if !exercise}
    <p class="mono dim">No structured exercise was generated for this course.</p>
  {:else}
    <header class="ex-head">
      <h3>{exercise.title}</h3>
      {#if exercise.deliverable}
        <p class="deliverable"><strong>Done looks like:</strong> {exercise.deliverable}</p>
      {/if}
    </header>

    <div class="ex-instructions">
      <Markdown markdown={exercise.instructions} />
    </div>

    {#if exercise.starter_code}
      <div class="starter-block">
        <div class="starter-head mono">
          <span>starter code</span>
          <div class="starter-actions">
            <button class="ghost mono-ghost small" onclick={copyStarter}>
              {#if copied}<Check size={11} /> copied{:else}<Copy size={11} /> copy{/if}
            </button>
            <button class="ghost mono-ghost small" onclick={resetToStarter}>
              <RotateCcw size={11} /> reset draft to starter
            </button>
          </div>
        </div>
        <pre class="starter-code mono">{exercise.starter_code}</pre>
      </div>
    {/if}

    <div class="draft-block">
      <div class="draft-head mono">
        <span>your draft — autosaved</span>
        <span class="save-status" class:err={saveStatus === 'error'}>
          {saveStatus === 'saving'
            ? 'saving…'
            : saveStatus === 'saved'
              ? 'saved'
              : saveStatus === 'error'
                ? 'save failed — recovery copy retained'
                : ''}
        </span>
        {#if saveStatus === 'error'}
          <button type="button" onclick={() => saver?.flush()}>Retry save</button>
        {/if}
      </div>
      <textarea
        class="draft-input mono"
        value={draft}
        oninput={onDraftInput}
        placeholder="Build the artifact here. The draft never blocks leaving; mark it complete only when your evidence meets the acceptance criteria."
        rows="12"
        aria-label="exercise draft"
      ></textarea>
    </div>

    <div class="evidence-block" class:complete={completed}>
      <div class="evidence-head">
        <div>
          <h4>{completed ? 'Practice evidence recorded' : 'Close the learning loop'}</h4>
          <p>
            Name what proves it works, the trade-off you chose, and what would fail at 10× scale
            or team size.
          </p>
        </div>
        {#if completed}<Check size={18} aria-hidden="true" />{/if}
      </div>
      <textarea
        class="reflection-input mono"
        bind:value={reflection}
        rows="4"
        aria-label="exercise evidence and trade-off reflection"
        placeholder="Evidence: …&#10;Trade-off: …&#10;10× failure: …"
      ></textarea>
      <div class="evidence-actions">
        <button
          class="ghost mono-ghost"
          class:complete={completed}
          onclick={() => saveCompletion(true)}
          disabled={completionStatus === 'saving'}
        >
          <Check size={12} />
          {completionStatus === 'saving'
            ? 'saving…'
            : completed
              ? 'update evidence'
              : 'mark practice complete'}
        </button>
        {#if completed}
          <button class="ghost mono-ghost small" onclick={() => saveCompletion(false)}>
            reopen
          </button>
        {/if}
        {#if completionStatus === 'saved'}
          <span class="mono completion-status">saved to learner record</span>
        {/if}
      </div>
      {#if completionError}
        <p class="mono completion-error" role="alert">{completionError}</p>
      {/if}
    </div>

    {#if exercise.hints.length > 0}
      <div class="hints-block">
        <div class="hints-head mono"><Lightbulb size={12} /> hints</div>
        {#each exercise.hints.slice(0, revealedHints) as hint, i}
          <p class="hint"><span class="hint-num mono">{i + 1}.</span> {hint}</p>
        {/each}
        {#if revealedHints < exercise.hints.length}
          <button class="ghost mono-ghost small" onclick={revealNextHint}>
            reveal hint {revealedHints + 1} of {exercise.hints.length}
          </button>
        {/if}
      </div>
    {/if}
  {/if}
</div>

<style>
  .exercise-workspace {
    max-width: 68ch;
    margin: 0 auto;
    padding: 28px 24px 80px;
  }
  .dim {
    color: var(--faint);
    font-size: 12px;
  }
  .err {
    color: var(--bad-fg);
    font-size: 12px;
  }
  .ex-head h3 {
    font-size: 22px;
    margin-bottom: 8px;
  }
  .deliverable {
    font-size: 13px;
    color: var(--muted);
    line-height: 1.5;
    margin: 0 0 4px;
  }
  .ex-instructions {
    margin: 18px 0 6px;
  }
  .starter-block {
    margin: 20px 0;
  }
  .starter-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 10.5px;
    letter-spacing: 0.5px;
    color: var(--faint);
    margin-bottom: 6px;
  }
  .starter-actions {
    display: flex;
    gap: 6px;
  }
  .ghost.small {
    padding: 3px 10px;
    font-size: 10.5px;
  }
  .starter-code {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-panel);
    padding: 14px 16px;
    font-size: 12.5px;
    line-height: 1.6;
    overflow-x: auto;
    white-space: pre;
    margin: 0;
  }
  .draft-block {
    margin: 22px 0;
  }
  .draft-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 10.5px;
    letter-spacing: 0.5px;
    color: var(--faint);
    margin-bottom: 6px;
  }
  .save-status {
    color: var(--muted);
  }
  .save-status.err {
    color: var(--bad-fg);
  }
  .draft-input {
    font-size: 13px;
    line-height: 1.6;
    resize: vertical;
    min-height: 220px;
  }
  .evidence-block {
    margin: 22px 0;
    padding: 16px;
    border: 1px solid var(--border);
    border-radius: var(--radius-panel);
    background: color-mix(in srgb, var(--surface) 82%, transparent);
  }
  .evidence-block.complete {
    border-color: color-mix(in srgb, var(--good-fg) 45%, var(--border));
  }
  .evidence-head {
    display: flex;
    justify-content: space-between;
    gap: 16px;
    color: var(--muted);
  }
  .evidence-head h4 {
    margin: 0 0 4px;
    color: var(--text);
    font-size: 14px;
  }
  .evidence-head p {
    margin: 0 0 12px;
    font-size: 12px;
    line-height: 1.5;
  }
  .reflection-input {
    min-height: 92px;
    resize: vertical;
    font-size: 12px;
    line-height: 1.55;
  }
  .evidence-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 9px;
  }
  .evidence-actions .complete {
    border-color: color-mix(in srgb, var(--good-fg) 45%, var(--border));
    color: var(--good-fg);
  }
  .completion-status {
    color: var(--muted);
    font-size: 10px;
  }
  .completion-error {
    margin: 8px 0 0;
    color: var(--bad-fg);
    font-size: 10.5px;
  }
  .hints-block {
    margin-top: 22px;
    padding-top: 16px;
    border-top: 1px solid var(--border);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .hints-head {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 10.5px;
    letter-spacing: 0.5px;
    color: var(--faint);
  }
  .hint {
    font-size: 13px;
    line-height: 1.5;
    color: var(--fg);
    margin: 0;
  }
  .hint-num {
    color: var(--accent);
    font-size: 11px;
    margin-right: 4px;
  }
</style>
