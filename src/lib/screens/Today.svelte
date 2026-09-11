<script lang="ts">
  import { api, type AppointmentView, type ClassroomSlotView, type ClassroomSubjectId } from '../ipc';
  import { app } from '../stores.svelte';
  import { formatClassCountdown, formatClassTime, nextScheduledClass } from '../next-class';
  import NodeCard from '../components/NodeCard.svelte';
  import MetaBadge from '../components/MetaBadge.svelte';
  import StatusLED from '../components/StatusLED.svelte';
  import { ArrowRight, BookOpen, Clock, Play, Pause } from 'lucide-svelte';
  let now = $state(new Date()), busy = $state(false);
  async function snooze(minutes: number) {
    if (!alarm || busy) return; busy = true;
    try { await api.snoozeAlarm(alarm.occurrence_id, minutes); await app.refresh(); } catch (e) { app.error = String(e); } finally { busy = false; }
  }
  function untilClock(value: string) { const d = new Date(value); return Number.isNaN(d.getTime()) ? '' : d.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' }); }
  const next = $derived(nextScheduledClass(app.state, now));
  const resumable = $derived(app.state?.active_classroom_sessions ?? []);
  const activeClasses = $derived((app.state?.classroom_programs ?? []).filter(p => p.enabled));
  const slots = $derived((app.state?.classroom_slots ?? []).filter(s => s.enabled && activeClasses.some(p => p.subject_id === s.subject_id)));
  const dueSlots = $derived(slots.filter(s => s.owed));
  const alarm = $derived(app.state?.alarm ?? null);
  const alarmSlot = $derived(alarm ? slots.find(s => s.occurrence_id === alarm.occurrence_id) ?? null : null);
  const missed = $derived((app.state?.appointments ?? []).filter(a => a.disposition === 'missed'));
  /** Engineering classes with spaced review due and no saved session in the way. */
  const reviews = $derived(activeClasses.filter(p => p.kind === 'engineering' && p.review_due > 0 && !resumable.some(s => s.subject_id === p.subject_id)));
  const upcoming = $derived(slots.filter(s => !s.owed).sort((a,b) => a.next_fire_at.localeCompare(b.next_fire_at)).slice(0,4));
  /** The class whose focused session holds the desk; other classes wait. */
  const held = $derived(app.state?.focus?.course_id ?? null);
  const heldLabel = $derived(held ? (app.state?.classroom_programs.find(p => p.subject_id === held)?.label ?? held) : '');
  const heldElsewhere = (subject: string) => held !== null && held !== subject;
  $effect(() => { const timer = setInterval(() => now = new Date(),1000); return () => clearInterval(timer); });
  async function pause() {
    if (busy) return; busy = true;
    try { if (app.state?.schedule_paused) await api.resumeSchedule(); else await api.pauseSchedule(); await app.refresh(); } catch (cause) { app.error = String(cause); } finally { busy = false; }
  }
  async function start(slot: ClassroomSlotView) {
    if (busy) return; busy = true;
    try { const program = activeClasses.find(p => p.subject_id === slot.subject_id); await app.startClass(slot.subject_id,slot.id,program?.completed ?? false,slot.occurrence_id); } finally { busy = false; }
  }
  async function makeUp(appointment: AppointmentView) {
    if (busy) return; busy = true;
    try { const program = activeClasses.find(p => p.subject_id === appointment.course_id); await app.startClass(appointment.course_id as ClassroomSubjectId,null,program?.completed ?? false,appointment.id); } finally { busy = false; }
  }
  async function skipAppointment(appointment: AppointmentView) {
    if (busy) return; busy = true;
    try { await api.skipAppointment(appointment.id); await app.refresh(); } catch (cause) { app.error = String(cause); } finally { busy = false; }
  }
  function appointment(slot: ClassroomSlotView) {
    const date = new Date(slot.next_fire_at);
    return Number.isNaN(date.getTime()) ? 'Time unavailable' : date.toLocaleDateString(undefined,{weekday:'short',month:'short',day:'numeric'}) + ' · ' + `${String(slot.hour).padStart(2,'0')}:${String(slot.minute).padStart(2,'0')}`;
  }
</script>
<div class="today">
  {#if app.state?.enforcement_disarmed}<p class="recovery mono" role="status">ENFORCEMENT DISARMED · a principia-unlock release token is present.</p>{/if}
  {#if alarm}
    <section class="alarm" class:snoozed={!!alarm.snoozed_until} role="alert" aria-live="assertive" aria-label="Study alarm">
      <div class="alarm-copy">
        <span class="mono">{alarm.snoozed_until ? `SNOOZED · RINGS AGAIN AT ${untilClock(alarm.snoozed_until)}` : 'STUDY ALARM · RINGING'}</span>
        <strong>{alarm.label} is due{alarm.queued ? `, and ${alarm.queued} more ${alarm.queued === 1 ? 'is' : 'are'} waiting` : ''}.</strong>
        <p>The alarm stops when you start the lesson. You can break the glass once you are in it, but not before.</p>
      </div>
      <div class="alarm-actions">
        {#if alarmSlot}<button class="cta mono-cta" disabled={busy || !!app.preparingClass} onclick={() => start(alarmSlot)}><Play size={13} /> Start {alarm.label}</button>{/if}
        {#if !alarm.snoozed_until}{#each [5, 10, 15] as minutes (minutes)}<button class="ghost mono-ghost" disabled={busy} onclick={() => snooze(minutes)}>Snooze {minutes} min</button>{/each}{/if}
      </div>
    </section>
  {/if}
  <section class="idle-center" aria-labelledby="today-title">
    <div class="meta-label">TODAY · {now.toLocaleDateString(undefined,{weekday:'long',month:'short',day:'numeric'})}</div>
    <h1 id="today-title">{resumable.length ? 'Your study desk is waiting' : dueSlots.length ? 'Time for your next class' : 'Make room for learning'}</h1>
    <p class="sub">{resumable.length ? resumable[0].title : dueSlots.length ? `${dueSlots.length} class appointment${dueSlots.length === 1 ? ' is' : 's are'} due.` : 'Your class schedules bring the next session here.'}</p>
    <div class="node-wrap"><NodeCard Icon={app.state?.schedule_paused ? Pause : Clock} name="class-scheduler" badge={app.state?.schedule_paused ? 'paused' : next ? 'next' : 'idle'} badgeTone="amber">
      <div class="meta-label">{app.state?.schedule_paused ? 'SCHEDULING PAUSED' : 'NEXT CLASS'}</div><div class="count mono">{next ? formatClassCountdown(next,now) : '—:—:—'}</div><div class="next-class">{next?.label ?? 'No upcoming appointments'}</div><div class="sched mono">{formatClassTime(next,now)}</div>
    </NodeCard></div>
    <div class="badges"><MetaBadge tone="violet">{activeClasses.length} active classes</MetaBadge><MetaBadge tone="teal">{slots.length} study times</MetaBadge></div>
    <button class="cta mono-cta" onclick={() => app.openClass()}><BookOpen size={14} />Open classes</button>
    <div class="quick-links"><button class="ghost mono-ghost" onclick={() => app.navigate('progress')}>Progress ledger</button><button class="ghost mono-ghost" onclick={pause} disabled={busy}>{app.state?.schedule_paused ? 'Resume appointments' : 'Pause appointments'}</button></div>
    {#if held}<p class="held mono" role="status">A focused {heldLabel} session holds the desk. Other classes wait until it finishes.</p>{/if}
  </section>
  {#if resumable.length}
    <section class="saved" aria-label="Saved sessions"><NodeCard Icon={Play} name="saved-sessions" badge="resumable" badgeTone="teal">
      {#each resumable as session}{@const pending = session.runtime === 'study' && ['planned', 'preparing'].includes(session.lifecycle)}<div class="study-row"><div><strong>{session.label}</strong><p>{session.title}{pending ? ' · preparation did not finish' : ''}</p></div>{#if pending}<button class="ghost mono-ghost" disabled={busy || !!app.preparingClass || heldElsewhere(session.subject_id)} onclick={() => app.startClass(session.subject_id)}>Retry<ArrowRight size={12} /></button>{:else}<button class="ghost mono-ghost" disabled={heldElsewhere(session.subject_id)} onclick={() => app.resumeClass(session.subject_id)}>Resume<ArrowRight size={12} /></button>{/if}</div>{/each}
    </NodeCard></section>
  {/if}
  <div class="overview">
    <section aria-label="Class agenda"><NodeCard Icon={Clock} name="class-agenda" badge={dueSlots.length ? dueSlots.length+' due' : missed.length ? missed.length+' missed' : 'upcoming'} badgeTone={dueSlots.length || missed.length ? 'amber' : 'teal'}>
      {#each dueSlots as slot}<div class="study-row"><div><strong>{slot.label}</strong><p class="mono">{String(slot.hour).padStart(2,'0')}:{String(slot.minute).padStart(2,'0')} · due</p></div><button class="ghost mono-ghost" disabled={busy || !!app.preparingClass || resumable.some(s => s.subject_id === slot.subject_id) || heldElsewhere(slot.subject_id)} onclick={() => start(slot)}>Start class</button></div>{/each}
      {#each reviews as program (program.subject_id)}<div class="study-row"><div><strong>{program.label}</strong><p class="mono">{program.review_due} {program.review_due === 1 ? 'topic' : 'topics'} · review due</p></div><button class="ghost mono-ghost" disabled={busy || !!app.preparingClass || heldElsewhere(program.subject_id)} onclick={() => app.startReview(program.subject_id)}>Start review<ArrowRight size={12} /></button></div>{/each}
      {#each missed as appointment (appointment.id)}<div class="study-row"><div><strong>{appointment.label}</strong><p class="mono">{appointment.local_date} · {appointment.local_time} · missed</p></div><div class="row-actions"><button class="ghost mono-ghost" disabled={busy || !!app.preparingClass || resumable.some(s => s.subject_id === appointment.course_id) || !activeClasses.some(p => p.subject_id === appointment.course_id) || heldElsewhere(appointment.course_id)} onclick={() => makeUp(appointment)}>Make up</button><button class="ghost mono-ghost" disabled={busy} onclick={() => skipAppointment(appointment)}>Skip</button></div></div>{/each}
      {#if !app.state?.schedule_paused}{#each upcoming as slot}<button class="agenda-link" onclick={() => app.openClass(slot.subject_id,'schedule')}><span><strong>{slot.label}</strong><small>{appointment(slot)}</small></span><ArrowRight size={13} /></button>{/each}{/if}
      {#if !dueSlots.length && !missed.length && (!upcoming.length || app.state?.schedule_paused)}<div class="queue-state"><StatusLED tone="ok" /><p>{app.state?.schedule_paused ? 'Appointments are paused.' : 'Add study times inside a class to build your week.'}</p></div>{/if}
      <button class="ghost mono-ghost" onclick={() => app.openClass(null,'schedule')}>Manage class schedules<ArrowRight size={12} /></button>
    </NodeCard></section>
    <section aria-label="Your classes"><NodeCard Icon={BookOpen} name="class-registry" badge={activeClasses.length+' active'} badgeTone="violet">
      {#each activeClasses as program}<button class="class-link" onclick={() => app.openClass(program.subject_id)}><span class="mono">{program.short_code}</span><span>{program.label}<small>{program.progress_label}</small></span><ArrowRight size={12} /></button>{:else}<p class="hint">Choose your first class and add its study times.</p>{/each}
      <button class="ghost mono-ghost" onclick={() => app.openClass()}>Browse courses<ArrowRight size={12} /></button>
    </NodeCard></section>
  </div>
</div>
<style>
  .held { margin: 10px 0 0; font-size: 11px; color: var(--led-warn); }
  .today { width: min(1080px,100%); padding: 26px 30px 40px; margin: 0 auto; } .idle-center { display: flex; flex-direction: column; align-items: center; text-align: center; padding: 24px 0 28px; } h1 { font-size: 34px; margin: 10px 0 6px; } .sub { color: var(--muted); font-size: 13px; margin: 0 0 22px; max-width: 620px; }
  .node-wrap { width: min(370px,100%); text-align: left; margin-bottom: 18px; } .count { font-size: 30px; color: var(--accent); margin: 5px 0; } .next-class { font-size: 14px; font-weight: 500; } .sched { font-size: 11px; color: var(--muted); margin-top: 5px; } .badges,.quick-links { display: flex; gap: 10px; flex-wrap: wrap; justify-content: center; } .badges { margin-bottom: 22px; } .quick-links { margin-top: 16px; }
  .overview { display: grid; grid-template-columns: 1fr 1fr; gap: 20px; padding-top: 22px; border-top: 1px dashed var(--node-border); } .saved { margin: 0 0 22px; } .study-row { display: flex; justify-content: space-between; align-items: center; gap: 16px; padding: 10px 0; border-bottom: 1px dashed var(--node-divider); margin-bottom: 12px; } .study-row strong { font-size: 13px; font-weight: 500; } .study-row p,.hint { font-size: 12px; color: var(--muted); margin: 5px 0 12px; } .queue-state { display: flex; gap: 9px; align-items: center; font-size: 12px; color: var(--muted); }
  .class-link,.agenda-link { display: flex; align-items: center; gap: 12px; text-align: left; width: 100%; background: none; border: 0; border-bottom: 1px dashed var(--node-divider); padding: 12px 0; margin-bottom: 8px; color: var(--fg); font: 13px var(--font-body); cursor: pointer; } .class-link > span:first-child { color: var(--accent); font-size: 11px; } .class-link > span:nth-child(2),.agenda-link > span { flex: 1; } small { color: var(--muted); display: block; font-size: 11px; margin-top: 5px; } .agenda-link strong { font-weight: 500; }
  .row-actions { display: flex; gap: 6px; flex-shrink: 0; }
  .alarm { display: flex; align-items: center; justify-content: space-between; gap: 16px; flex-wrap: wrap; padding: 14px 16px; border: 1px solid var(--accent); border-radius: var(--radius-panel); background: color-mix(in srgb, var(--accent) 10%, var(--surface)); }
  .alarm.snoozed { border-style: dashed; background: var(--surface); }
  .alarm-copy { display: grid; gap: 4px; min-width: 0; } .alarm-copy .mono { font-size: 9px; letter-spacing: .7px; color: var(--accent); } .alarm.snoozed .alarm-copy .mono { color: var(--muted); }
  .alarm-copy strong { font-size: 14px; } .alarm-copy p { margin: 0; font-size: 11px; color: var(--muted); line-height: 1.55; }
  .alarm-actions { display: flex; gap: 8px; flex-wrap: wrap; }
  .recovery { color: var(--bad-fg); border: 1px dashed var(--led-err); background: var(--bad-bg); padding: 9px 12px; border-radius: var(--radius-control); font-size: 11px; }
  @media(max-width:620px) { .today { padding: 22px 18px; } .overview { grid-template-columns: 1fr; } h1 { font-size: 28px; } .study-row { flex-wrap: wrap; } }
</style>
