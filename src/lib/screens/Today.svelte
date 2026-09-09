<script lang="ts">
  import { api } from '../ipc';
  import { app } from '../stores.svelte';
  import { courseDefinition } from '../catalog';
  import { formatClassCountdown, formatClassTime, nextScheduledClass } from '../next-class';
  import NodeCard from '../components/NodeCard.svelte';
  import MetaBadge from '../components/MetaBadge.svelte';
  import StatusLED from '../components/StatusLED.svelte';
  import { ArrowRight, BookOpen, Clock, Play, Pause, Rocket } from 'lucide-svelte';
  let now = $state(new Date());
  let starting = $state(false);
  const next = $derived(nextScheduledClass(app.state, now));
  const primary = $derived(app.session?.focus || app.state?.selected_focus || 'javascript');
  const resumable = $derived(app.state?.active_classroom_sessions ?? []);
  const dueSlots = $derived((app.state?.classroom_slots ?? []).filter((slot) => slot.owed));
  const activeClasses = $derived((app.state?.classroom_programs ?? []).filter((program) => program.enabled));
  $effect(() => { const timer = setInterval(() => (now = new Date()), 1000); return () => clearInterval(timer); });
  async function begin() {
    if (starting) return;
    starting = true;
    try { await api.startSession(primary); await app.refresh(); }
    catch (error) { app.error = String(error); }
    finally { starting = false; }
  }
</script>

<div class="today">
  {#if app.state?.enforcement_disarmed}<p class="recovery mono" role="status">ENFORCEMENT DISARMED · ~/sdr-unlock is present.</p>{/if}
  <section class="idle-center" aria-labelledby="today-title">
    <div class="meta-label">{app.state?.owed ? 'INCIDENT — P1 · TRAINING SESSION DUE' : 'TODAY · ' + now.toLocaleDateString(undefined, { weekday: 'long', month: 'short', day: 'numeric' })}</div>
    <h1 id="today-title">{app.state?.owed ? app.session?.session_type === 'pop_quiz' ? 'Pop quiz. No new topic today.' : 'Your session starts now' : app.session?.status === 'in_progress' || resumable.length ? 'Your session is waiting' : 'Desk idle'}</h1>
    <p class="sub">{app.state?.owed ? 'Your daily commitment is due. Finish or skip it before starting another class.' : app.session?.status === 'in_progress' ? 'Resume your daily session at its saved step.' : resumable.length ? resumable[0].title : 'Next session deploys automatically. Showing up is the whole job.'}</p>
    {#if !app.state?.owed}
      <div class="node-wrap">
        <NodeCard Icon={app.state?.schedule_paused ? Pause : Clock} name="class-scheduler" badge={app.state?.schedule_paused ? 'paused' : next ? 'next' : 'idle'} badgeTone={app.state?.schedule_paused ? 'red' : 'amber'} accent={app.state?.schedule_paused ? 'var(--led-err)' : 'var(--node-border)'}>
          <div class="meta-label">{app.state?.schedule_paused ? 'SCHEDULER PAUSED' : 'NEXT_CLASS — T-minus'}</div>
          <div class="count mono">{next ? formatClassCountdown(next, now) : '—:—:—'}</div>
          <div class="next-class">{next?.label ?? 'No enabled appointments'}</div>
          <div class="sched mono">{formatClassTime(next, now)}</div>
          {#if app.state?.schedule_paused}<button class="ghost mono-ghost" onclick={() => app.navigate('schedule')}>manage schedule</button>{/if}
        </NodeCard>
      </div>
    {/if}
    <div class="badges">
      <MetaBadge tone="teal">● uptime {app.session?.streak ?? 0}d</MetaBadge>
      {#if app.state?.owed}<MetaBadge tone="violet">est. {app.session?.session_type === 'pop_quiz' ? '15' : '38'} min</MetaBadge>
      {:else}<MetaBadge tone="violet">{activeClasses.length} active classes</MetaBadge>{/if}
    </div>
    {#if app.state?.owed}
      <button class="cta mono-cta" onclick={begin} disabled={starting}><Rocket size={14} /> {starting ? 'starting…' : 'ack & begin session'}</button>
    {:else if app.session?.status === 'in_progress'}
      <button class="cta mono-cta" onclick={() => app.resumeSession()}><Play size={14} /> resume session — {app.session.step}</button>
    {:else if resumable.length}
      <button class="cta mono-cta" onclick={() => app.resumeClass(resumable[0].subject_id)}><Play size={14} /> continue learning</button>
    {:else}
      <button class="cta mono-cta" onclick={() => app.navigate('classes')}><BookOpen size={14} /> open classes</button>
    {/if}
    <div class="quick-links"><button class="ghost mono-ghost" onclick={() => app.navigate('progress')}>progress ledger</button><button class="ghost mono-ghost" onclick={() => app.navigate('schedule')}>reschedule</button><button class="ghost mono-ghost" onclick={() => app.navigate('settings')}>tutor: {app.state?.agent ?? 'claude'}</button></div>
  </section>

  {#if resumable.length > (app.state?.owed || app.session?.status === 'in_progress' ? 0 : 1)}
    <section class="saved" aria-label="Other saved sessions"><NodeCard Icon={Play} name="saved-sessions" badge="resumable" badgeTone="teal">
      {#each resumable.slice(app.state?.owed || app.session?.status === 'in_progress' ? 0 : 1) as session}
        <div class="study-row"><div><strong>{session.label}</strong><p>{session.title}</p></div><button class="ghost mono-ghost" disabled={app.state?.owed} onclick={() => app.resumeClass(session.subject_id)}>resume <ArrowRight size={12} /></button></div>
      {/each}
      {#if app.state?.owed}<p class="hint">Resume becomes available after the due daily commitment is handled.</p>{/if}
    </NodeCard></section>
  {/if}
  <div class="overview">
    <section aria-label="Scheduled study"><NodeCard Icon={Clock} name="schedule-queue" badge={dueSlots.length ? dueSlots.length + ' due' : 'clear'} badgeTone={dueSlots.length ? 'amber' : 'teal'}>
      {#if dueSlots.length}
        {#each dueSlots as slot}
          {@const program = app.state?.classroom_programs.find((item) => item.subject_id === slot.subject_id)}
          <div class="study-row"><div><strong>{slot.label}</strong><p class="mono">{String(slot.hour).padStart(2, '0')}:{String(slot.minute).padStart(2, '0')} · due</p></div><button class="ghost mono-ghost" disabled={app.state?.owed || !program?.enabled || resumable.some((session) => session.subject_id === slot.subject_id)} onclick={() => app.startClass(slot.subject_id, slot.id, program?.completed ?? false)}>start scheduled class</button></div>
        {/each}
      {:else}<div class="queue-state"><StatusLED tone="ok" /><p>No class appointments are due.</p></div>{/if}
      <button class="ghost mono-ghost" onclick={() => app.navigate('schedule')}>inspect schedule <ArrowRight size={12} /></button>
    </NodeCard></section>
    <section aria-label="Your classes"><NodeCard Icon={BookOpen} name="class-registry" badge={activeClasses.length + ' active'} badgeTone="violet">
      {#each activeClasses as program}
        <button class="class-link" onclick={() => app.navigate('classes')}><span class="mono">{program.short_code}</span><span>{program.label}<small>{program.progress_label}</small></span><ArrowRight size={12} /></button>
      {:else}<p class="hint">Choose your first course from the catalog.</p>{/each}
      <button class="ghost mono-ghost" onclick={() => app.navigate('classes')}>browse courses <ArrowRight size={12} /></button>
    </NodeCard></section>
  </div>
</div>

<style>
  .today { width: min(960px, 100%); padding: 26px 30px 40px; margin: 0 auto; }
  .idle-center { display: flex; flex-direction: column; align-items: center; text-align: center; padding: 32px 0 28px; }
  h1 { font-size: 34px; margin: 10px 0 6px; }
  .sub { color: var(--muted); font-size: 13px; margin: 0 0 22px; max-width: 620px; }
  .node-wrap { width: min(320px, 100%); text-align: left; margin-bottom: 18px; }
  .count { font-size: 30px; color: var(--accent); margin: 4px 0 2px; }
  .next-class { font-size: 13px; font-weight: 500; }
  .sched { font-size: 11px; color: var(--faint); }
  .badges { display: flex; gap: 10px; margin-bottom: 22px; }
  .quick-links { display: flex; gap: 10px; margin-top: 16px; flex-wrap: wrap; justify-content: center; }
  .overview { display: grid; grid-template-columns: 1fr 1fr; gap: 20px; padding-top: 22px; border-top: 1px dashed var(--node-border); }
  .saved { margin: 0 0 22px; }
  .study-row { display: flex; justify-content: space-between; align-items: center; gap: 16px; padding: 10px 0; border-bottom: 1px dashed var(--node-divider); margin-bottom: 12px; }
  .study-row strong { font-size: 13px; font-weight: 500; }
  .study-row p, .hint { font-size: 12px; color: var(--muted); margin: 5px 0 12px; }
  .queue-state { display: flex; gap: 9px; align-items: center; font-size: 12px; color: var(--muted); }
  .class-link { display: flex; align-items: center; gap: 12px; text-align: left; width: 100%; background: none; border: 0; border-bottom: 1px dashed var(--node-divider); padding: 10px 0; margin-bottom: 8px; color: var(--fg); font: 12px var(--font-body); cursor: pointer; }
  .class-link > span:first-child { color: var(--accent); font-size: 10px; }
  .class-link > span:nth-child(2) { flex: 1; }
  .class-link small { color: var(--muted); display: block; font-size: 10px; margin-top: 4px; }
  .recovery { color: var(--bad-fg); border: 1px dashed var(--led-err); background: var(--bad-bg); padding: 9px 12px; border-radius: 6px; font-size: 11px; }
  @media (max-width: 620px) { .today { padding: 22px 18px; } .overview { grid-template-columns: 1fr; } .idle-center { padding: 18px 0 25px; } h1 { font-size: 28px; } .study-row { flex-wrap: wrap; } }
</style>
