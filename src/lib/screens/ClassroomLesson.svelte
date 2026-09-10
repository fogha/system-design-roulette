<script lang="ts">
  import { api, type EngineeringSessionResult } from '../ipc';
  import type { AssessmentRoundId } from '../contracts/assessments';
  import { app } from '../stores.svelte';
  import ClusterBar from '../components/ClusterBar.svelte';
  import CourseChat from '../components/CourseChat.svelte';
  import CoursePurpose from '../components/CoursePurpose.svelte';
  import ExerciseWorkspace from '../components/ExerciseWorkspace.svelte';
  import Markdown from '../components/Markdown.svelte';
  import {
    ArrowLeft,
    CheckCircle2,
    ExternalLink,
    MessageCircle,
    Send,
    Sparkles,
  } from 'lucide-svelte';

  const lesson = $derived(app.engineeringLesson);
  /** Shared-runtime lessons persist answers against their frozen check round. */
  const study = $derived(lesson?.runtime === 'study');
  let answers = $state<number[]>([]);
  let reflection = $state('');
  let submitting = $state(false);
  let result = $state<EngineeringSessionResult | null>(null);
  let prepared = $state<string | null>(null);
  let chatOpen = $state(false);
  let roundId = $state<AssessmentRoundId | null>(null);
  let checkRevision = $state(0);
  let answerStatus = $state<'saved' | 'saving' | 'error'>('saved');
  let answerError = $state('');
  let saveQueue: Promise<void> = Promise.resolve();

  $effect(() => {
    if (lesson && prepared !== lesson.session_id) {
      prepared = lesson.session_id;
      const restored: number[] = Array(lesson.questions.length).fill(-1);
      if (lesson.check) {
        for (const [index, question] of lesson.questions.entries()) {
          const saved = lesson.check.responses[String(question.id)];
          const choice = saved?.status === 'answered' ? Number(saved.answer) : NaN;
          if (Number.isInteger(choice) && choice >= 0) restored[index] = choice;
        }
      }
      answers = restored;
      const savedReflection = lesson.checkpoint?.body.work.reflection;
      reflection = typeof savedReflection === 'string' ? savedReflection : '';
      result = lesson.outcome ?? null;
      chatOpen = false;
      roundId = lesson.check?.round_id ?? null;
      checkRevision = lesson.check?.revision ?? 0;
      answerStatus = 'saved';
      answerError = '';
    }
  });

  const complete = $derived(lesson ? answers.every((answer) => answer >= 0) : false);

  function choose(index: number, choiceIndex: number) {
    const next = [...answers];
    next[index] = choiceIndex;
    answers = next;
    if (!lesson || !study || !roundId) return;
    const sessionId = lesson.session_id;
    const round = roundId;
    const questionId = lesson.questions[index].id;
    answerStatus = 'saving';
    saveQueue = saveQueue.then(async () => {
      try {
        const saved = await api.saveClassCheckAnswer(sessionId, round, checkRevision, questionId, choiceIndex);
        checkRevision = saved.revision;
        answerStatus = 'saved';
        answerError = '';
      } catch (error) {
        answerStatus = 'error';
        answerError = String(error);
      }
    });
  }

  async function pause() {
    if (lesson && study) {
      try {
        await saveQueue;
        await api.pauseClassLesson(lesson.session_id);
      } catch (error) {
        app.error = String(error);
      }
    }
    app.screen = 'idle';
    void app.refresh();
  }

  /** Name the publisher so the learner can weigh a source before opening it. */
  function publisher(url: string): string {
    try {
      return new URL(url).host.replace(/^www\./, '');
    } catch {
      return 'unverified source';
    }
  }

  async function submit() {
    if (!lesson || !complete || submitting) return;
    submitting = true;
    try {
      if (study) {
        await saveQueue;
        if (answerStatus === 'error') throw new Error(answerError || 'Saved answers could not be stored. Try again.');
        if (!roundId) throw new Error('The knowledge check is not ready yet.');
        result = await api.submitClassCheck(lesson.session_id, roundId, checkRevision, reflection);
      } else {
        result = await api.submitClassroomEngineeringSession({
          session_id: Number(lesson.session_id),
          answers,
          reflection,
        });
      }
    } catch (error) {
      app.error = String(error);
    } finally {
      submitting = false;
    }
  }
</script>

{#if lesson}
  <div class="class-screen blueprint">
    <ClusterBar
      route={`classroom/${lesson.subject_id}/${lesson.category}`}
      status={`${lesson.prompt_version} · ${lesson.agent_used}`}
      tone="ok"
    />

    <header class="lesson-head">
      <button class="back-button" type="button" onclick={pause}>
        <ArrowLeft size={14} /> pause class
      </button>
      <div class="identity">
        <span class="class-code mono">{lesson.short_code}</span>
        <div>
          <span class="eyebrow mono">{lesson.label} · {lesson.estimated_minutes} MIN</span>
          <h1>{lesson.title}</h1>
          <p>{lesson.concept_title} · {lesson.category}</p>
        </div>
      </div>
      <div class="lesson-actions">
        <span class="quality mono"><Sparkles size={11} /> isolated teacher</span>
        <button
          class="chat-button"
          class:active={chatOpen}
          type="button"
          onclick={() => (chatOpen = !chatOpen)}
          aria-expanded={chatOpen}
          aria-controls="course-chat-drawer"
        >
          <MessageCircle size={12} /> {chatOpen ? 'close chat' : 'ask about this course'}
        </button>
      </div>
    </header>

    <CoursePurpose
      whyNow={lesson.why_now}
      curriculum={lesson.curriculum}
      prerequisites={lesson.prerequisites}
    />

    <main class="reading-layout">
      <article class="reading-pane">
        <Markdown markdown={lesson.markdown} />

        {#if lesson.resources.length}
          <section class="sources" aria-labelledby="class-sources-title">
            <span class="eyebrow mono">PRIMARY EVIDENCE</span>
            <h2 id="class-sources-title">Continue the investigation</h2>
            <ul>
              {#each lesson.resources as resource}
                <li>
                  <a href={resource.url} target="_blank" rel="noreferrer">
                    {resource.title} <ExternalLink size={11} />
                  </a>
                  <span class="source-host mono">{publisher(resource.url)}</span>
                  {#if resource.why}<p>{resource.why}</p>{/if}
                </li>
              {/each}
            </ul>
          </section>
        {/if}

        <section class="exercise-end" aria-label="course exercise">
          <ExerciseWorkspace classroomSessionId={study ? undefined : Number(lesson.session_id)} studySessionId={study ? lesson.session_id : undefined} />
        </section>

        <aside class="practice-pane" aria-labelledby="class-check-title">
        <span class="eyebrow mono">RETRIEVAL GATE</span>
        <h2 id="class-check-title">Prove the mechanism</h2>
        <p class="practice-intro">
          Answer from the lesson’s mechanism and evidence. Your result updates only this class.
        </p>
        {#if study && !result}
          <p class="save-state mono" class:err={answerStatus === 'error'} role="status">
            {answerStatus === 'saving' ? 'saving answers…' : answerStatus === 'error' ? `answers not saved: ${answerError}` : 'answers saved with this lesson'}
          </p>
        {/if}

        {#if result}
          <div class:passed={result.passed} class="result-card" aria-live="polite">
            <CheckCircle2 size={18} />
            <div>
              <strong>{Math.round(result.score * 100)}% · {result.passed ? 'evidence recorded' : 'review due'}</strong>
              <p>Corrections remain visible below; the class never locks the app.</p>
            </div>
          </div>
        {/if}

        <div class="question-list">
          {#each lesson.questions as question, index (question.id)}
            {@const correction = result?.corrections.find((item) => item.question_id === question.id)}
            <fieldset class:incorrect={correction && !correction.correct} class:correct={correction?.correct}>
              <legend>
                <span class="question-number mono">{String(index + 1).padStart(2, '0')}</span>
                {question.prompt}
              </legend>
              <p class="objective mono">{question.learning_objective}</p>
              <div class="choice-list">
                {#each question.choices as choice, choiceIndex}
                  <label>
                    <input
                      type="radio"
                      name={`class-question-${question.id}`}
                      value={choiceIndex}
                      checked={answers[index] === choiceIndex}
                      disabled={!!result}
                      onchange={() => choose(index, choiceIndex)}
                    />
                    <span>{choice}</span>
                  </label>
                {/each}
              </div>
              {#if correction}
                <div class="correction" role="status">
                  <strong>{correction.correct ? 'Correct' : `Correct answer: ${correction.correct_answer}`}</strong>
                  <p>{correction.explanation}</p>
                </div>
              {/if}
            </fieldset>
          {/each}
        </div>

        <label class="reflection-field">
          <span>Implementation reflection <small>(optional)</small></span>
          <textarea
            bind:value={reflection}
            disabled={!!result}
            placeholder="What will you test, change, or measure in a real frontend?"
          ></textarea>
        </label>

        {#if result}
          <button class="submit-button" type="button" onclick={() => app.finishClass()}>
            return to classroom
          </button>
        {:else}
          <button class="submit-button" type="button" disabled={!complete || submitting} onclick={submit}>
            <Send size={13} /> {submitting ? 'recording…' : 'check and record evidence'}
          </button>
        {/if}
        </aside>
      </article>
    </main>

    <CourseChat classroomSessionId={study ? undefined : Number(lesson.session_id)} studySessionId={study ? lesson.session_id : undefined} bind:open={chatOpen} />
  </div>
{:else}
  <div class="empty">
    <p>No engineering class is active.</p>
    <button type="button" onclick={() => (app.screen = 'idle')}>return to classroom</button>
  </div>
{/if}

<style>
  .class-screen { min-height: 100vh; display: flex; flex-direction: column; background: var(--bg); }
  .lesson-head {
    min-height: 96px;
    padding: 14px 22px;
    border-bottom: 1px solid var(--node-border);
    display: grid;
    grid-template-columns: 150px 1fr minmax(220px, auto);
    align-items: center;
    gap: 18px;
  }
  .back-button {
    justify-self: start;
    border: 1px solid var(--node-border);
    border-radius: var(--radius-control);
    background: transparent;
    color: var(--muted);
    min-height: 34px;
    padding: 6px 9px;
    display: flex;
    align-items: center;
    gap: 6px;
    cursor: pointer;
  }
  .identity { display: flex; align-items: center; gap: 13px; min-width: 0; }
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
  h1 { font-size: 19px; margin: 4px 0; }
  .identity p { color: var(--muted); font-size: 10px; margin: 0; }
  .lesson-actions { justify-self: end; display: flex; align-items: center; gap: 9px; }
  .quality { display: flex; align-items: center; gap: 5px; color: var(--faint); font-size: 8px; }
  .chat-button {
    min-height: 32px;
    padding: 6px 9px;
    border: 1px solid var(--node-border);
    border-radius: var(--radius-control);
    background: transparent;
    color: var(--muted);
    display: inline-flex;
    align-items: center;
    gap: 6px;
    cursor: pointer;
    font-size: 9px;
  }
  .chat-button.active { border-color: var(--accent); color: var(--accent); }
  .reading-layout { flex: 1; min-height: 0; overflow-y: auto; }
  .reading-pane { width: min(100%, 920px); margin: 0 auto; padding: 28px clamp(24px, 5vw, 72px) 64px; }
  .exercise-end { margin-top: 40px; border-top: 1px solid var(--node-border); }
  .practice-pane { margin-top: 12px; border-top: 1px solid var(--node-border); border-radius: var(--radius-panel); padding: 24px; background: var(--surface); }
  .practice-pane h2, .sources h2 { font-size: 16px; margin: 6px 0; }
  .practice-intro { color: var(--muted); font-size: 11px; line-height: 1.5; }
  .save-state { color: var(--faint); font-size: 9px; margin: 0 0 6px; } .save-state.err { color: var(--red); }
  .sources { margin-top: 28px; border: 1px solid var(--node-border); border-radius: var(--radius-panel); padding: 16px; }
  .sources p { color: var(--muted); font-size: 12px; line-height: 1.6; }
  .sources ul { padding-left: 18px; }
  .sources a { color: var(--accent); display: inline-flex; gap: 5px; align-items: center; }
  .source-host { color: var(--faint); font-size: 9px; margin-left: 6px; text-transform: lowercase; }
  .question-list { display: grid; gap: 10px; margin-top: 16px; }
  fieldset { border: 1px solid var(--node-border); border-radius: var(--radius-control); padding: 11px; min-width: 0; }
  fieldset.correct { border-color: var(--green); }
  fieldset.incorrect { border-color: var(--red); }
  legend { padding: 0 5px; font-size: 11px; line-height: 1.45; }
  .question-number { color: var(--accent); margin-right: 6px; font-size: 8px; }
  .objective { color: var(--faint); font-size: 8px; margin: 2px 0 9px; }
  .choice-list { display: grid; gap: 5px; }
  .choice-list label {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    color: var(--muted);
    font-size: 10px;
    line-height: 1.4;
    cursor: pointer;
    padding: 5px;
    border-radius: var(--radius-detail);
  }
  .choice-list label:hover { background: var(--surface-2); }
  .choice-list input { margin-top: 2px; accent-color: var(--accent); }
  .correction { margin-top: 9px; border-top: 1px solid var(--node-border); padding-top: 8px; font-size: 10px; }
  .correction p { margin: 4px 0 0; color: var(--muted); line-height: 1.5; }
  .result-card { display: flex; gap: 9px; border: 1px solid var(--amber); border-radius: var(--radius-control); padding: 10px; margin-top: 13px; color: var(--amber); }
  .result-card.passed { border-color: var(--green); color: var(--green); }
  .result-card p { margin: 3px 0 0; color: var(--muted); font-size: 9px; }
  .reflection-field { display: grid; gap: 6px; margin-top: 14px; color: var(--muted); font-size: 10px; }
  .reflection-field textarea { min-height: 92px; resize: vertical; background: var(--bg); color: var(--text); border: 1px solid var(--node-border); border-radius: var(--radius-control); padding: 9px; }
  .submit-button {
    width: 100%;
    min-height: 38px;
    margin-top: 12px;
    border: 1px solid var(--accent);
    border-radius: var(--radius-control);
    background: var(--accent);
    color: var(--bg);
    display: flex;
    justify-content: center;
    align-items: center;
    gap: 6px;
    cursor: pointer;
  }
  .submit-button:disabled { opacity: 0.4; cursor: not-allowed; }
  button:focus-visible, input:focus-visible, textarea:focus-visible, a:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .empty { min-height: 100vh; display: grid; place-content: center; gap: 10px; }
  @media (max-width: 860px) {
    .lesson-head { grid-template-columns: 1fr; }
    .lesson-actions { justify-self: start; flex-wrap: wrap; }
  }
</style>
