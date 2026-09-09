<script lang="ts">
  import { api, type ReviewData } from '../ipc';
  import { app } from '../stores.svelte';
  // Capture once: async work must keep the session that opened this screen.
  const sessionId = app.session?.session_id ?? '';
  import ClusterBar from '../components/ClusterBar.svelte';
  import NodeCard from '../components/NodeCard.svelte';
  import MetaBadge from '../components/MetaBadge.svelte';
  import Markdown from '../components/Markdown.svelte';
  import { Check, X, Circle, TriangleAlert, ArrowRight } from 'lucide-svelte';

  let review = $state<ReviewData | null>(null);
  let error = $state('');
  let idx = $state(0);
  let dwell = $state(10);

  const current = $derived(review?.items[idx]);
  const isLast = $derived(review ? idx === review.items.length - 1 : false);

  async function load() {
    error = '';
    try { review = await api.getReview(sessionId); }
    catch (cause) { error = String(cause); }
  }

  $effect(() => { void load(); });

  $effect(() => {
    idx;
    dwell = app.state?.debug_day ? 1 : 10;
    const id = setInterval(() => {
      dwell = Math.max(0, dwell - 1);
      if (dwell === 0) clearInterval(id);
    }, 1000);
    return () => clearInterval(id);
  });

  async function next() {
    if (!review) return;
    if (isLast) {
      await api.finishReview(sessionId);
      await app.refresh();
    } else {
      idx += 1;
    }
  }
</script>

<div class="review-wrap blueprint">
  <ClusterBar route="quiz/trace" status="responses graded" tone="ok" />
  {#if error}
    <div class="center"><p role="alert">{error}</p><button class="ghost mono-ghost" onclick={load}>Reload feedback</button></div>
  {:else if !review}
    <div class="center"><p class="sub mono">loading trace…</p></div>
  {:else if review.items.length === 0}
    <div class="center"><p class="sub mono">no requests today</p></div>
  {:else if current}
    <div class="review-body">
      <div class="head-row">
        <span class="mono head-meta">
          trace {idx + 1}/{review.items.length} · error budget {Math.round(review.score * 100)}%
        </span>
        {#if review.self_assess}
          <MetaBadge tone="amber">{#snippet children()}<TriangleAlert size={10} /> grader offline — self-assess{/snippet}</MetaBadge>
        {/if}
      </div>

      <NodeCard
        Icon={current.correct === false ? X : current.correct === true ? Check : Circle}
        name={`response trace — /quiz/${idx + 1}`}
        badge={current.correct === true
          ? '200 OK'
          : current.correct === false
            ? '422 → DLQ · retries tomorrow'
            : 'ungraded — self-assess'}
        badgeTone={current.correct === true ? 'teal' : current.correct === false ? 'red' : 'amber'}
        accent={current.correct === false ? 'var(--led-err)' : current.correct === true ? '#2b4a3f' : 'var(--led-warn)'}
      >
        {#snippet children()}
          <div class="prompt"><Markdown markdown={current.prompt} compact /></div>
          <div class="trace">
            <div class="trace-line">
              <span class="trace-key mono">your_answer</span>
              <div class="trace-value">
                {#if current.user_answer}<Markdown markdown={current.user_answer} compact />{:else}(blank){/if}
              </div>
            </div>
            <div class="trace-line">
              <span class="trace-key mono">expected</span>
              <div class="trace-value"><Markdown markdown={current.correct_answer} compact /></div>
            </div>
            {#if current.feedback}
              <div class="trace-line">
                <span class="trace-key mono">grader_log</span>
                <div class="trace-value"><Markdown markdown={current.feedback} compact /></div>
              </div>
            {/if}
            <div class="trace-line explain">
              <span class="trace-key mono">why</span>
              <div class="trace-value"><Markdown markdown={current.explanation} compact /></div>
            </div>
          </div>
        {/snippet}
      </NodeCard>

      <div class="actions">
        <button class="cta mono-cta" onclick={next} disabled={dwell > 0}>
          {dwell > 0 ? `read · ${dwell}s` : isLast ? 'to the rack' : 'next trace'}{#if dwell <= 0}<ArrowRight size={13} />{/if}
        </button>
      </div>
    </div>
  {/if}
</div>

<style>
  .review-wrap {
    flex: 1;
    display: flex;
    flex-direction: column;
    animation: fade-in 0.35s ease;
  }
  .center {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .sub {
    color: var(--faint);
    font-size: 12px;
  }
  .review-body {
    width: min(780px, 92vw);
    margin: 0 auto;
    flex: 1;
    display: flex;
    flex-direction: column;
    justify-content: center;
    padding: 24px;
    gap: 14px;
  }
  .head-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .head-meta {
    font-size: 11px;
    color: var(--faint);
  }
  .prompt {
    font-size: 17px;
    line-height: 1.45;
    margin-bottom: 14px;
    font-weight: 500;
    min-width: 0;
  }
  .trace {
    display: flex;
    flex-direction: column;
    gap: 12px;
    max-height: 50vh;
    overflow-y: auto;
  }
  .trace-line {
    display: grid;
    grid-template-columns: 110px 1fr;
    gap: 12px;
    align-items: start;
  }
  .trace-key {
    font-size: 10px;
    color: var(--faint);
    letter-spacing: 0.5px;
  }
  .trace-value {
    margin: 0;
    font-size: 14px;
    min-width: 0;
    overflow: hidden;
  }
  .explain .trace-value {
    color: var(--muted);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
  }
</style>
