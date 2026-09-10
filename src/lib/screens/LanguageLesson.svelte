<script lang="ts">
  import { api, type LanguageSessionResult } from '../ipc';
  import type { AssessmentRoundId } from '../contracts/assessments';
  import { app } from '../stores.svelte';
  import ClusterBar from '../components/ClusterBar.svelte';
  import Dropdown from '../components/Dropdown.svelte';
  import Markdown from '../components/Markdown.svelte';
  import {
    ArrowLeft,
    CheckCircle2,
    Headphones,
    MessageSquareText,
    Pause,
    Play,
    RotateCcw,
    Send,
    Volume2,
  } from 'lucide-svelte';

  const confidenceOptions = [
    { value: 1, label: '1 · Needed the script' }, { value: 2, label: '2 · Many pauses' },
    { value: 3, label: '3 · Understandable with help' }, { value: 4, label: '4 · Mostly clear' },
    { value: 5, label: '5 · Clear and independent' },
  ];
  const lesson = $derived(app.languageLesson);
  /** Shared-runtime lessons persist answers and production work with the session. */
  const study = $derived(lesson?.runtime === 'study');
  let roundId = $state<AssessmentRoundId | null>(null);
  let checkRevision = $state(0);
  let answerStatus = $state<'saved' | 'saving' | 'error'>('saved');
  let answerError = $state('');
  let saveQueue: Promise<void> = Promise.resolve();
  let answers = $state<number[]>([]);
  let writingResponse = $state('');
  let speakingCompleted = $state(false);
  let listened = $state(false);
  let confidence = $state(3);
  let speaking = $state(false);
  let submitting = $state(false);
  let result = $state<LanguageSessionResult | null>(null);
  let preparedSession = $state<string | null>(null);

  $effect(() => {
    if (lesson && preparedSession !== lesson.session_id) {
      const restored: number[] = Array(lesson.questions.length).fill(-1);
      if (lesson.check) {
        for (const [index, question] of lesson.questions.entries()) {
          const saved = lesson.check.responses[String(question.id)];
          const choice = saved?.status === 'answered' ? Number(saved.answer) : NaN;
          if (Number.isInteger(choice) && choice >= 0) restored[index] = choice;
        }
      }
      answers = restored;
      const work = lesson.checkpoint?.body.work ?? {};
      writingResponse = typeof work.writing_response === 'string' ? work.writing_response : '';
      speakingCompleted = work.speaking_completed === true;
      listened = work.listened === true;
      confidence = typeof work.confidence === 'number' && work.confidence >= 1 && work.confidence <= 5 ? work.confidence : 3;
      result = lesson.outcome ?? null;
      roundId = lesson.check?.round_id ?? null;
      checkRevision = lesson.check?.revision ?? 0;
      answerStatus = 'saved';
      answerError = '';
      preparedSession = lesson.session_id;
    }
  });

  function choose(questionIndex: number, choiceIndex: number) {
    if (result) return;
    answers[questionIndex] = choiceIndex;
    answers = [...answers];
    if (!lesson || !study || !roundId) return;
    const sessionId = lesson.session_id;
    const round = roundId;
    const questionId = lesson.questions[questionIndex].id;
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

  /** Production work is saved with the lesson so a paused session restores it. */
  async function pause() {
    stopSpeaking();
    if (lesson && study && !result) {
      try {
        await saveQueue;
        await api.saveClassLessonWork({ session_id: lesson.session_id, expected_revision: null, work: { writing_response: writingResponse, speaking_completed: speakingCompleted, listened, confidence } });
        await api.pauseClassLesson(lesson.session_id);
      } catch (error) {
        app.error = String(error);
      }
    }
    app.screen = 'idle';
    void app.refresh();
  }

  function stopSpeaking() {
    if (typeof speechSynthesis !== 'undefined') speechSynthesis.cancel();
    speaking = false;
  }

  function speak(text: string) {
    if (typeof speechSynthesis === 'undefined' || !lesson) {
      app.error = 'Speech synthesis is not available in this webview.';
      return;
    }
    speechSynthesis.cancel();
    const utterance = new SpeechSynthesisUtterance(text);
    utterance.lang = lesson.speech_locale;
    utterance.rate = lesson.level === 'A1' ? 0.82 : lesson.level === 'A2' ? 0.9 : 0.96;
    utterance.onstart = () => {
      speaking = true;
      listened = true;
    };
    utterance.onend = () => (speaking = false);
    utterance.onerror = () => {
      speaking = false;
      app.error = 'The selected system voice could not play this passage.';
    };
    speechSynthesis.speak(utterance);
  }

  async function submit() {
    if (!lesson || answers.some((answer) => answer < 0) || submitting) return;
    submitting = true;
    try {
      if (study) {
        await saveQueue;
        if (answerStatus === 'error') throw new Error(answerError || 'Saved answers could not be stored. Try again.');
        if (!roundId) throw new Error('The knowledge check is not ready yet.');
        result = await api.submitClassLanguageCheck(lesson.session_id, roundId, checkRevision, { writing_response: writingResponse, speaking_completed: speakingCompleted, listened, confidence });
      } else {
        result = await api.submitLanguageSession({
          session_id: Number(lesson.session_id),
          answers,
          writing_response: writingResponse,
          speaking_completed: speakingCompleted,
          listened,
          confidence,
        });
      }
      stopSpeaking();
    } catch (error) {
      app.error = String(error);
    } finally {
      submitting = false;
    }
  }

  function correctionFor(id: number) {
    return result?.corrections.find((correction) => correction.question_id === id);
  }
</script>

{#if lesson}
  <div class="language-screen blueprint">
    <ClusterBar
      route={`languages/${lesson.language}/${lesson.level.toLowerCase()}`}
      status="advisory practice · frontend schedule remains independent"
      tone="ok"
    />

    <header class="lesson-head">
      <button class="back" type="button" onclick={pause}>
        <ArrowLeft size={14} /> pause and return
      </button>
      <div class="lesson-identity">
        <span class="code mono">{lesson.language === 'german' ? 'DE' : 'IT'}</span>
        <div>
          <span class="meta-label">{lesson.label} · CEFR {lesson.level} · pass {lesson.phase}</span>
          <h1>{lesson.title}</h1>
          <p>{lesson.phase_label} · about {lesson.estimated_minutes} minutes</p>
        </div>
      </div>
      <div class="evidence-status mono" aria-label="Session evidence status">
        <span class:ready={listened}>audio {listened ? '✓' : '○'}</span>
        <span class:ready={speakingCompleted}>speech {speakingCompleted ? '✓' : '○'}</span>
        <span class:ready={writingResponse.trim().length > 0}>writing {writingResponse.trim().length > 0 ? '✓' : '○'}</span>
      </div>
    </header>

    <main class="lesson-layout">
      <article class="reading-pane">
        <div class="mission">
          <span class="meta-label">ACTION-ORIENTED MISSION</span>
          <p>{lesson.scenario}</p>
          <strong>{lesson.can_do}</strong>
        </div>

        <div class="audio-console">
          <div>
            <Volume2 size={16} />
            <span>
              <strong>Listen to the model exchange</strong>
              <small>System voice · {lesson.speech_locale} · slowed at beginner levels</small>
            </span>
          </div>
          {#if speaking}
            <button type="button" onclick={stopSpeaking}><Pause size={13} /> stop</button>
          {:else}
            <button type="button" onclick={() => speak(lesson.listen_text)}><Play size={13} /> play dialogue</button>
          {/if}
        </div>

        <div class="paper theme-scholar">
          <Markdown markdown={lesson.markdown} />
        </div>
      </article>

      <aside class="practice-pane" aria-labelledby="practice-title">
        <div class="practice-intro">
          <span class="meta-label">RETRIEVAL + PRODUCTION</span>
          <h2 id="practice-title">Show what you can do</h2>
          <p>Answer from memory. Wrong answers return with a concrete explanation.</p>
          {#if study && !result}<p class="save-state mono" class:err={answerStatus === 'error'} role="status">{answerStatus === 'saving' ? 'saving answers…' : answerStatus === 'error' ? `answers not saved: ${answerError}` : 'answers saved with this lesson'}</p>{/if}
        </div>

        <div class="questions">
          {#each lesson.questions as question, questionIndex (question.id)}
            {@const correction = correctionFor(question.id)}
            <fieldset class:correct={correction?.correct} class:incorrect={correction && !correction.correct}>
              <legend>
                <span class="question-number mono">{String(questionIndex + 1).padStart(2, '0')}</span>
                {question.prompt}
              </legend>
              <span class="strand mono">{question.strand.replaceAll('_', ' ')}</span>
              <div class="choices">
                {#each question.choices as choice, choiceIndex}
                  <label
                    class:selected={answers[questionIndex] === choiceIndex}
                    class:right={!!correction && choice === correction.correct_answer}
                    class:wrong={!!correction && answers[questionIndex] === choiceIndex && !correction.correct}
                  >
                    <input
                      type="radio"
                      name={`language-question-${question.id}`}
                      value={choiceIndex}
                      checked={answers[questionIndex] === choiceIndex}
                      disabled={!!result}
                      onchange={() => choose(questionIndex, choiceIndex)}
                    />
                    <span class="choice-key mono">{String.fromCharCode(65 + choiceIndex)}</span>
                    <span>{choice}</span>
                  </label>
                {/each}
              </div>
              {#if correction}
                <div class:good={correction.correct} class="correction" role="status">
                  <strong>{correction.correct ? 'Correct' : `Correct answer: ${correction.correct_answer}`}</strong>
                  <p>{correction.explanation}</p>
                </div>
              {/if}
            </fieldset>
          {/each}
        </div>

        <section class="production">
          <div class="production-head">
            <MessageSquareText size={15} />
            <h3>Writing evidence</h3>
          </div>
          <p>{lesson.writing_prompt}</p>
          <textarea
            bind:value={writingResponse}
            disabled={!!result}
            rows="6"
            aria-label="Writing response in the language you are learning"
            placeholder={`Write in ${lesson.native_label}. Aim for at least 12 words; clear, level-appropriate language is enough.`}
          ></textarea>
          <div class="word-count mono">{writingResponse.trim() ? writingResponse.trim().split(/\s+/).length : 0} words</div>
        </section>

        <section class="production">
          <div class="production-head">
            <Headphones size={15} />
            <h3>Speaking evidence</h3>
          </div>
          <p>{lesson.speaking_prompt}</p>
          <label class="speaking-check">
            <input type="checkbox" bind:checked={speakingCompleted} disabled={!!result} />
            <span>I said this aloud without reading every line</span>
          </label>
          <div class="confidence"><Dropdown label="How understandable did it feel?" bind:value={confidence} options={confidenceOptions} disabled={!!result} /></div>
        </section>

        {#if result}
          <section class:passed={result.passed} class="result-card" aria-live="polite">
            {#if result.passed}<CheckCircle2 size={20} />{:else}<RotateCcw size={20} />{/if}
            <div>
              <span class="meta-label">{result.passed ? 'EVIDENCE RECORDED' : 'RETRIEVAL NEEDS ANOTHER PASS'}</span>
              <h3>{Math.round(result.score * 100)}% session score</h3>
              <p>
                {result.passed
                  ? `Progress remains at ${result.current_level} until every skill strand clears the level gate.`
                  : 'This scenario will return. Review the explanations rather than memorising choice letters.'}
              </p>
              {#if result.level_advanced_to}
                <strong>Level gate passed — now entering {result.level_advanced_to}.</strong>
              {/if}
            </div>
          </section>
          <button class="finish-button" type="button" onclick={() => app.finishLanguage()}>
            return to learning programs
          </button>
        {:else}
          <button
            class="submit-button"
            type="button"
            disabled={answers.some((answer) => answer < 0) || submitting}
            onclick={submit}
          >
            <Send size={14} />
            {submitting ? 'recording evidence…' : 'check answers and record practice'}
          </button>
          {#if answers.some((answer) => answer < 0)}
            <p class="submit-hint">Answer all {lesson.questions.length} checks to submit. Writing and speaking can be added now or strengthened in a later pass.</p>
          {/if}
        {/if}
      </aside>
    </main>
  </div>
{:else}
  <div class="screen">
    <p>No active language lesson.</p>
    <button class="ghost" type="button" onclick={() => (app.screen = 'idle')}>return</button>
  </div>
{/if}

<style>
  .language-screen {
    min-height: 100%;
    display: flex;
    flex: 1;
    flex-direction: column;
    overflow: hidden;
  }
  .lesson-head {
    display: grid;
    grid-template-columns: minmax(140px, 0.7fr) minmax(0, 1.6fr) minmax(180px, 0.7fr);
    align-items: center;
    gap: 24px;
    padding: 14px 22px;
    border-bottom: 1px solid var(--border);
    background: var(--node-bg);
  }
  .back {
    width: fit-content;
    min-height: 34px;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    color: var(--muted);
    background: transparent;
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
    padding: 6px 10px;
    font: 10px var(--font-mono);
    cursor: pointer;
  }
  .back:hover {
    color: var(--fg);
    border-color: var(--muted);
  }
  button:focus-visible,
  input:focus-visible,
  textarea:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .lesson-identity {
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .code {
    width: 38px;
    height: 38px;
    display: grid;
    flex: none;
    place-items: center;
    color: var(--accent);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-panel);
    font-size: 11px;
  }
  .lesson-identity h1 {
    overflow: hidden;
    margin: 2px 0 0;
    font-size: 20px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .lesson-identity p {
    margin: 2px 0 0;
    color: var(--muted);
    font-size: 10px;
  }
  .evidence-status {
    display: flex;
    justify-content: flex-end;
    gap: 7px;
    font-size: 8px;
    text-transform: uppercase;
  }
  .evidence-status span {
    padding: 4px 6px;
    color: var(--faint);
    border: 1px solid var(--border);
    border-radius: var(--radius-detail);
  }
  .evidence-status span.ready {
    color: var(--ok-fg);
    background: var(--ok-bg);
    border-color: var(--led-ok);
  }
  .lesson-layout {
    min-height: 0;
    display: grid;
    flex: 1;
    grid-template-columns: minmax(0, 1.35fr) minmax(390px, 0.65fr);
  }
  .reading-pane,
  .practice-pane {
    overflow-y: auto;
    padding: 24px;
  }
  .reading-pane {
    border-right: 1px solid var(--border);
  }
  .mission {
    max-width: 760px;
    margin: 0 auto 12px;
    padding: 14px 16px;
    color: var(--warn-fg);
    background: var(--warn-bg);
    border-radius: var(--radius-panel);
  }
  .mission p {
    margin: 4px 0;
    font-size: 12px;
  }
  .mission strong {
    display: block;
    font-size: 12px;
  }
  .audio-console {
    max-width: 760px;
    min-height: 62px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    margin: 0 auto 12px;
    padding: 10px 14px;
    color: var(--fg);
    background: var(--node-bg);
    border: 1px solid var(--border);
    border-radius: var(--radius-panel);
  }
  .audio-console > div {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .audio-console strong,
  .audio-console small {
    display: block;
  }
  .audio-console strong {
    font-size: 11px;
  }
  .audio-console small {
    color: var(--muted);
    font-size: 9px;
  }
  .audio-console button {
    min-height: 34px;
    display: inline-flex;
    flex: none;
    align-items: center;
    gap: 6px;
    padding: 6px 10px;
    color: var(--accent-fg);
    background: var(--accent);
    border: 0;
    border-radius: var(--radius-control);
    font: 9px var(--font-mono);
    text-transform: uppercase;
    cursor: pointer;
  }
  .paper {
    max-width: 760px;
    margin: 0 auto;
    padding: 32px clamp(24px, 6vw, 58px);
    color: var(--fg);
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: var(--radius-panel);
    user-select: text;
  }
  .practice-pane {
    background: color-mix(in srgb, var(--node-bg) 96%, transparent);
  }
  .save-state { color: var(--faint); font-size: 9px; margin: 6px 0 0; }
  .save-state.err { color: var(--red); }
  .practice-intro {
    margin-bottom: 18px;
  }
  .practice-intro h2 {
    margin-top: 3px;
    font-size: 23px;
  }
  .practice-intro p {
    margin: 4px 0 0;
    color: var(--muted);
    font-size: 11px;
  }
  .questions {
    display: grid;
    gap: 10px;
  }
  fieldset {
    min-width: 0;
    margin: 0;
    padding: 13px;
    border: 1px solid var(--border);
    border-radius: var(--radius-panel);
    background: var(--bg);
  }
  fieldset.correct {
    border-color: var(--led-ok);
  }
  fieldset.incorrect {
    border-color: var(--led-err);
  }
  legend {
    width: 100%;
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 8px;
    padding: 0;
    color: var(--fg);
    font-size: 12px;
    line-height: 1.45;
  }
  .question-number {
    color: var(--accent);
    font-size: 9px;
  }
  .strand {
    display: block;
    margin: 4px 0 8px 25px;
    color: var(--faint);
    font-size: 7px;
    text-transform: uppercase;
  }
  .choices {
    display: grid;
    gap: 5px;
  }
  .choices label {
    min-height: 38px;
    display: grid;
    grid-template-columns: auto 22px 1fr;
    align-items: center;
    gap: 6px;
    padding: 6px 8px;
    color: var(--muted);
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
    cursor: pointer;
    font-size: 10px;
  }
  .choices label:hover,
  .choices label.selected {
    color: var(--fg);
    border-color: var(--accent);
    background: var(--surface);
  }
  .choices label.right {
    color: var(--ok-fg);
    border-color: var(--led-ok);
    background: var(--ok-bg);
  }
  .choices label.wrong {
    color: var(--bad-fg);
    border-color: var(--led-err);
    background: var(--bad-bg);
  }
  .choices input {
    margin: 0;
    accent-color: var(--accent);
  }
  .choice-key {
    color: var(--faint);
  }
  .correction {
    margin-top: 8px;
    padding: 8px 9px;
    color: var(--bad-fg);
    background: var(--bad-bg);
    border-radius: var(--radius-control);
    font-size: 9px;
  }
  .correction.good {
    color: var(--ok-fg);
    background: var(--ok-bg);
  }
  .correction p {
    margin: 2px 0 0;
  }
  .production {
    margin-top: 12px;
    padding: 14px;
    border: 1px solid var(--border);
    border-radius: var(--radius-panel);
    background: var(--bg);
  }
  .production-head {
    display: flex;
    align-items: center;
    gap: 7px;
  }
  .production h3 {
    font-family: var(--font-body);
    font-size: 12px;
    font-weight: 600;
  }
  .production > p {
    margin: 6px 0 10px;
    color: var(--muted);
    font-size: 10px;
  }
  textarea {
    resize: vertical;
    font-size: 11px;
    line-height: 1.55;
  }
  .word-count {
    margin-top: 3px;
    color: var(--faint);
    font-size: 8px;
    text-align: right;
  }
  .speaking-check {
    min-height: 38px;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 9px;
    border: 1px solid var(--border);
    border-radius: var(--radius-control);
    font-size: 10px;
    cursor: pointer;
  }
  .speaking-check input {
    accent-color: var(--accent);
  }
  .confidence { margin-top: 12px; max-width: 360px; }
  .submit-button,
  .finish-button {
    width: 100%;
    min-height: 44px;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 7px;
    margin-top: 12px;
    color: var(--accent-fg);
    background: var(--accent);
    border: 0;
    border-radius: var(--radius-control);
    font: 10px var(--font-mono);
    text-transform: uppercase;
    cursor: pointer;
  }
  .submit-button:disabled {
    cursor: not-allowed;
    opacity: 0.4;
  }
  .submit-hint {
    margin: 5px 0 0;
    color: var(--faint);
    font-size: 8px;
    text-align: center;
  }
  .result-card {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 10px;
    margin-top: 12px;
    padding: 14px;
    color: var(--bad-fg);
    background: var(--bad-bg);
    border: 1px solid var(--led-err);
    border-radius: var(--radius-panel);
  }
  .result-card.passed {
    color: var(--ok-fg);
    background: var(--ok-bg);
    border-color: var(--led-ok);
  }
  .result-card h3 {
    margin-top: 2px;
    font-family: var(--font-body);
    font-size: 15px;
    font-weight: 600;
  }
  .result-card p {
    margin: 3px 0;
    font-size: 9px;
  }
  @media (max-width: 980px) {
    .lesson-head {
      grid-template-columns: auto 1fr;
    }
    .evidence-status {
      display: none;
    }
    .lesson-layout {
      display: block;
      overflow-y: auto;
    }
    .reading-pane,
    .practice-pane {
      overflow: visible;
    }
    .reading-pane {
      border-right: 0;
    }
  }
  @media (max-width: 620px) {
    .lesson-head {
      gap: 10px;
      padding: 10px;
    }
    .back {
      font-size: 0;
    }
    .lesson-identity h1 {
      font-size: 16px;
    }
    .reading-pane,
    .practice-pane {
      padding: 12px;
    }
    .paper {
      padding: 22px 18px;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    * {
      animation: none !important;
      transition: none !important;
    }
  }
</style>
