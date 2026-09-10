<script lang="ts">
  import { api, type ClassroomSlotView } from '../ipc';
  import { app } from '../stores.svelte';
  import { formatClassCountdown, formatClassTime, nextScheduledClass } from '../next-class';
  import NodeCard from '../components/NodeCard.svelte';
  import MetaBadge from '../components/MetaBadge.svelte';
  import StatusLED from '../components/StatusLED.svelte';
  import { ArrowRight, BookOpen, Clock, Play, Pause } from 'lucide-svelte';
  let now = $state(new Date()), busy = $state(false);
  const next = $derived(nextScheduledClass(app.state, now));
  const resumable = $derived(app.state?.active_classroom_sessions ?? []);
  const activeClasses = $derived((app.state?.classroom_programs ?? []).filter(p => p.enabled));
  const slots = $derived((app.state?.classroom_slots ?? []).filter(s => s.enabled && activeClasses.some(p => p.subject_id === s.subject_id)));
  const dueSlots = $derived(slots.filter(s => s.owed));
  const upcoming = $derived(slots.filter(s => !s.owed).sort((a,b) => a.next_fire_at.localeCompare(b.next_fire_at)).slice(0,4));
  const legacy = $derived(app.session?.status === 'in_progress');
  $effect(() => { const timer = setInterval(() => now = new Date(),1000); return () => clearInterval(timer); });
  async function pause() {
    if (busy) return; busy = true;
    try { if (app.state?.schedule_paused) await api.resumeSchedule(); else await api.pauseSchedule(); await app.refresh(); } catch (cause) { app.error = String(cause); } finally { busy = false; }
  }
  async function start(slot: ClassroomSlotView) {
    if (busy) return; busy = true;
    try { const program = activeClasses.find(p => p.subject_id === slot.subject_id); await app.startClass(slot.subject_id,slot.id,program?.completed ?? false); } finally { busy = false; }
  }
  function appointment(slot: ClassroomSlotView) {
    const date = new Date(slot.next_fire_at);
    return Number.isNaN(date.getTime()) ? 'Time unavailable' : date.toLocaleDateString(undefined,{weekday:'short',month:'short',day:'numeric'}) + ' · ' + `${String(slot.hour).padStart(2,'0')}:${String(slot.minute).padStart(2,'0')}`;
  }
</script>
<div class="today">
  {#if app.state?.enforcement_disarmed}<p class="recovery mono" role="status">ENFORCEMENT DISARMED · ~/sdr-unlock is present.</p>{/if}
  <section class="idle-center" aria-labelledby="today-title">
    <div class="meta-label">TODAY · {now.toLocaleDateString(undefined,{weekday:'long',month:'short',day:'numeric'})}</div>
    <h1 id="today-title">{resumable.length || legacy ? 'Your study desk is waiting' : dueSlots.length ? 'Time for your next class' : 'Make room for learning'}</h1>
    <p class="sub">{resumable.length ? resumable[0].title : dueSlots.length ? `${dueSlots.length} class appointment${dueSlots.length === 1 ? ' is' : 's are'} due.` : 'Your class schedules bring the next session here.'}</p>
    <div class="node-wrap"><NodeCard Icon={app.state?.schedule_paused ? Pause : Clock} name="class-scheduler" badge={app.state?.schedule_paused ? 'paused' : next ? 'next' : 'idle'} badgeTone="amber">
      <div class="meta-label">{app.state?.schedule_paused ? 'SCHEDULING PAUSED' : 'NEXT CLASS'}</div><div class="count mono">{next ? formatClassCountdown(next,now) : '—:—:—'}</div><div class="next-class">{next?.label ?? 'No upcoming appointments'}</div><div class="sched mono">{formatClassTime(next,now)}</div>
    </NodeCard></div>
    <div class="badges"><MetaBadge tone="violet">{activeClasses.length} active classes</MetaBadge><MetaBadge tone="teal">{slots.length} study times</MetaBadge></div>
    <button class="cta mono-cta" onclick={() => app.openClass()}><BookOpen size={14} />Open classes</button>
    <div class="quick-links"><button class="ghost mono-ghost" onclick={() => app.navigate('progress')}>Progress ledger</button><button class="ghost mono-ghost" onclick={pause} disabled={busy}>{app.state?.schedule_paused ? 'Resume appointments' : 'Pause appointments'}</button></div>
  </section>
  {#if resumable.length || legacy}
    <section class="saved" aria-label="Saved sessions"><NodeCard Icon={Play} name="saved-sessions" badge="resumable" badgeTone="teal">
      {#if legacy}<div class="study-row"><div><strong>Saved daily session</strong><p>The daily routine has been retired. Continue this existing session at its saved step.</p></div><button class="ghost mono-ghost" onclick={() => app.resumeSession()}>Resume<ArrowRight size={12} /></button></div>{/if}
      {#each resumable as session}<div class="study-row"><div><strong>{session.label}</strong><p>{session.title}</p></div><button class="ghost mono-ghost" onclick={() => app.resumeClass(session.subject_id)}>Resume<ArrowRight size={12} /></button></div>{/each}
    </NodeCard></section>
  {/if}
  <div class="overview">
    <section aria-label="Class agenda"><NodeCard Icon={Clock} name="class-agenda" badge={dueSlots.length ? dueSlots.length+' due' : 'upcoming'} badgeTone={dueSlots.length ? 'amber' : 'teal'}>
      {#each dueSlots as slot}<div class="study-row"><div><strong>{slot.label}</strong><p class="mono">{String(slot.hour).padStart(2,'0')}:{String(slot.minute).padStart(2,'0')} · due</p></div><button class="ghost mono-ghost" disabled={busy || !!app.preparingClass || resumable.some(s => s.subject_id === slot.subject_id)} onclick={() => start(slot)}>Start class</button></div>{/each}
      {#if !app.state?.schedule_paused}{#each upcoming as slot}<button class="agenda-link" onclick={() => app.openClass(slot.subject_id,'schedule')}><span><strong>{slot.label}</strong><small>{appointment(slot)}</small></span><ArrowRight size={13} /></button>{/each}{/if}
      {#if !dueSlots.length && (!upcoming.length || app.state?.schedule_paused)}<div class="queue-state"><StatusLED tone="ok" /><p>{app.state?.schedule_paused ? 'Appointments are paused.' : 'Add study times inside a class to build your week.'}</p></div>{/if}
      <button class="ghost mono-ghost" onclick={() => app.openClass(null,'schedule')}>Manage class schedules<ArrowRight size={12} /></button>
    </NodeCard></section>
    <section aria-label="Your classes"><NodeCard Icon={BookOpen} name="class-registry" badge={activeClasses.length+' active'} badgeTone="violet">
      {#each activeClasses as program}<button class="class-link" onclick={() => app.openClass(program.subject_id)}><span class="mono">{program.short_code}</span><span>{program.label}<small>{program.progress_label}</small></span><ArrowRight size={12} /></button>{:else}<p class="hint">Choose your first class and add its study times.</p>{/each}
      <button class="ghost mono-ghost" onclick={() => app.openClass()}>Browse courses<ArrowRight size={12} /></button>
    </NodeCard></section>
  </div>
</div>
<style>
  .today { width: min(1080px,100%); padding: 26px 30px 40px; margin: 0 auto; } .idle-center { display: flex; flex-direction: column; align-items: center; text-align: center; padding: 24px 0 28px; } h1 { font-size: 34px; margin: 10px 0 6px; } .sub { color: var(--muted); font-size: 13px; margin: 0 0 22px; max-width: 620px; }
  .node-wrap { width: min(370px,100%); text-align: left; margin-bottom: 18px; } .count { font-size: 30px; color: var(--accent); margin: 5px 0; } .next-class { font-size: 14px; font-weight: 500; } .sched { font-size: 11px; color: var(--muted); margin-top: 5px; } .badges,.quick-links { display: flex; gap: 10px; flex-wrap: wrap; justify-content: center; } .badges { margin-bottom: 22px; } .quick-links { margin-top: 16px; }
  .overview { display: grid; grid-template-columns: 1fr 1fr; gap: 20px; padding-top: 22px; border-top: 1px dashed var(--node-border); } .saved { margin: 0 0 22px; } .study-row { display: flex; justify-content: space-between; align-items: center; gap: 16px; padding: 10px 0; border-bottom: 1px dashed var(--node-divider); margin-bottom: 12px; } .study-row strong { font-size: 13px; font-weight: 500; } .study-row p,.hint { font-size: 12px; color: var(--muted); margin: 5px 0 12px; } .queue-state { display: flex; gap: 9px; align-items: center; font-size: 12px; color: var(--muted); }
  .class-link,.agenda-link { display: flex; align-items: center; gap: 12px; text-align: left; width: 100%; background: none; border: 0; border-bottom: 1px dashed var(--node-divider); padding: 12px 0; margin-bottom: 8px; color: var(--fg); font: 13px var(--font-body); cursor: pointer; } .class-link > span:first-child { color: var(--accent); font-size: 11px; } .class-link > span:nth-child(2),.agenda-link > span { flex: 1; } small { color: var(--muted); display: block; font-size: 11px; margin-top: 5px; } .agenda-link strong { font-weight: 500; }
  .recovery { color: var(--bad-fg); border: 1px dashed var(--led-err); background: var(--bad-bg); padding: 9px 12px; border-radius: var(--radius-control); font-size: 11px; }
  @media(max-width:620px) { .today { padding: 22px 18px; } .overview { grid-template-columns: 1fr; } h1 { font-size: 28px; } .study-row { flex-wrap: wrap; } }
</style>
