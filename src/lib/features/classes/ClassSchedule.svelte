<script lang="ts">
  import { untrack } from 'svelte';
  import { api, type ClassroomProgramView, type ClassroomPlanView } from '../../ipc';
  import { app } from '../../stores.svelte';
  import TimePicker from '../../components/TimePicker.svelte';
  import { Plus, Pencil, Trash2, Clock3, CalendarClock, ArrowRight, AlertTriangle } from 'lucide-svelte';
  import { scheduleConflicts, describeConflict, WEEKDAY_NAMES } from './schedule-conflicts';
  let { program, onstart, onmakeup, opening = false }: { program: ClassroomProgramView; onstart: (slotId: number) => void; onmakeup?: (occurrenceId: string) => void; opening?: boolean } = $props();
  const uid = $props.id();
  const DAYS = ['Mon','Tue','Wed','Thu','Fri','Sat','Sun'];
  const slots = $derived(app.state?.classroom_slots.filter(s => s.subject_id === program.subject_id) ?? []);
  const appointments = $derived((app.state?.appointments ?? []).filter(a => a.course_id === program.subject_id));
  async function skipAppointment(id: string) {
    if (busy) return;
    busy = true; error = ''; message = '';
    try { await api.skipAppointment(id); await app.refresh(); message = 'Appointment skipped.'; } catch (cause) { error = String(cause); } finally { busy = false; }
  }
  let view = $state<'times' | 'plan'>('times');
  let editing = $state(false), slotId = $state<number | null>(null), slotTime = $state('07:30'), slotDays = $state([1,2,3,4,5]);
  let busy = $state(false), error = $state(''), message = $state('');
  let goal = $state(untrack(() => program.learning_goal));
  let target = $state(untrack(() => program.target_weekly_minutes || program.language_progress?.weekly_minutes || 150));
  let windows = $state([{ weekdays: [1,2,3,4,5], start: '07:00', end: '08:00' }]);
  let preview = $state<ClassroomPlanView | null>(null), previewKey = $state('');
  const signature = $derived(JSON.stringify({ goal, target, windows, minutes: program.session_minutes }));
  const currentPreview = $derived(preview !== null && previewKey === signature);
  const validWindows = $derived(windows.every(w => w.weekdays.length > 0 && w.start < w.end));
  const time = (hour: number, minute: number) => `${String(hour).padStart(2,'0')}:${String(minute).padStart(2,'0')}`;
  // Same check the native command runs on save, shown while the learner is still editing.
  const editorConflicts = $derived.by(() => {
    if (!editing) return [];
    const [hour, minute] = slotTime.split(':').map(Number);
    return scheduleConflicts([{ subject_id: program.subject_id, hour, minute, weekdays: [...slotDays], session_minutes: program.session_minutes }], app.state?.classroom_slots ?? [], app.state?.classroom_programs ?? [], { excludedSlotIds: slotId === null ? [] : [slotId] });
  });
  const previewConflicts = $derived(preview?.conflicts ?? []);
  function days(values: number[]) { return values.length === 7 ? 'Every day' : values.join(',') === '1,2,3,4,5' ? 'Weekdays' : values.map(day => DAYS[day-1]).join(', '); }
  function flip(values: number[], day: number) { return values.includes(day) ? values.filter(d => d !== day) : [...values,day].sort(); }
  function edit(id?: number) {
    const slot = slots.find(s => s.id === id);
    slotId = slot?.id ?? null; slotTime = slot ? time(slot.hour,slot.minute) : '07:30'; slotDays = slot ? [...slot.weekdays] : [1,2,3,4,5]; editing = true; error = ''; message = '';
  }
  async function saveSlot() {
    if (busy || !slotDays.length) return;
    busy = true; error = ''; message = '';
    const [hour,minute] = slotTime.split(':').map(Number);
    try { await api.upsertClassroomSlot({ id: slotId, subject_id: program.subject_id, hour, minute, weekdays: [...slotDays], enabled: true }); await app.refresh(); editing = false; message = 'Study time saved.'; }
    catch (cause) { error = String(cause); } finally { busy = false; }
  }
  async function remove(id: number) {
    if (busy) return;
    busy = true; error = ''; message = '';
    const pausesClass = program.enabled && !slots.some(slot => slot.id !== id && slot.enabled);
    try { await api.deleteClassroomSlot(id); await app.refresh(); if (slotId === id) editing = false; message = pausesClass ? 'Study time removed. This class is now paused.' : 'Study time removed.'; } catch (cause) { error = String(cause); } finally { busy = false; }
  }
  async function plan(commit: boolean) {
    if (busy || !validWindows || (commit && !currentPreview)) return;
    busy = true; error = ''; message = '';
    const key = signature;
    const payload = { subject_id: program.subject_id, learning_goal: goal, target_weekly_minutes: target, windows: windows.map(w => { const [sh,sm] = w.start.split(':').map(Number); const [eh,em] = w.end.split(':').map(Number); return { weekdays: [...w.weekdays], start_hour: sh, start_minute: sm, end_hour: eh, end_minute: em }; }), commit };
    try {
      const result = await api.planClassroomSchedule(payload);
      if (commit) { await app.refresh(); preview = null; view = 'times'; message = 'Weekly schedule saved. Your manual study times are preserved.'; }
      else { preview = result; previewKey = key; }
    } catch (cause) { error = String(cause); if (!commit) preview = null; } finally { busy = false; }
  }
  function move(event: KeyboardEvent) {
    if (!['ArrowLeft','ArrowRight','Home','End'].includes(event.key)) return;
    event.preventDefault(); view = event.key === 'Home' ? 'times' : event.key === 'End' ? 'plan' : view === 'times' ? 'plan' : 'times';
    document.getElementById(`${uid}-${view}`)?.focus();
  }
</script>
<div class="schedule">
  <header><div><h3>Study schedule</h3><p>Make room for {program.label} in your week.</p></div><div class="switcher" role="tablist" aria-label="Schedule tools"><button role="tab" id={`${uid}-times`} aria-controls={`${uid}-times-panel`} aria-selected={view === 'times'} tabindex={view === 'times' ? 0 : -1} class:selected={view === 'times'} onclick={() => view = 'times'} onkeydown={move}><Clock3 size={14} />Study times</button><button role="tab" id={`${uid}-plan`} aria-controls={`${uid}-plan-panel`} aria-selected={view === 'plan'} tabindex={view === 'plan' ? 0 : -1} class:selected={view === 'plan'} onclick={() => view = 'plan'} onkeydown={move}><CalendarClock size={14} />Plan a week</button></div></header>
  {#if !program.enabled}<p class="notice">This class is inactive. Save a study time, then activate it from the header.</p>{/if}
  {#if error}<p class="error" role="alert">{error}</p>{/if}{#if message}<p class="message" role="status">{message}</p>{/if}
  <div id={`${uid}-times-panel`} role="tabpanel" aria-labelledby={`${uid}-times`} hidden={view !== 'times'} tabindex="0">
    <div class="list-heading"><span class="mono">{slots.length} RECURRING {slots.length === 1 ? 'TIME' : 'TIMES'}</span><button class="ghost mono-ghost" onclick={() => edit()} disabled={busy || editing}><Plus size={13} />Add study time</button></div>
    {#if editing}
      <div class="editor">
        <h4>{slotId === null ? 'New study time' : 'Edit study time'}</h4>
        <fieldset disabled={busy}><legend class="sr-only">Study time and days</legend><div class="editor-grid"><div><span class="label">Start time</span><TimePicker bind:value={slotTime} cron={false} compact label="Study start time" /></div><div><span class="label">Repeat on</span><div class="days" role="group" aria-label="Study days">{#each DAYS as day, index}<button class:active={slotDays.includes(index+1)} aria-pressed={slotDays.includes(index+1)} onclick={() => slotDays = flip(slotDays,index+1)}>{day}</button>{/each}</div>{#if !slotDays.length}<p class="error">Choose at least one day.</p>{/if}</div></div></fieldset>
        {#if editorConflicts.length}<div class="conflicts" role="alert"><span class="mono"><AlertTriangle size={12} />OVERLAPS ANOTHER STUDY TIME</span><ul>{#each editorConflicts as conflict}<li>{WEEKDAY_NAMES[conflict.weekday-1]} {time(conflict.hour,conflict.minute)} overlaps {describeConflict(conflict)}</li>{/each}</ul><p>Choose another time or shorten a session before saving.</p></div>{/if}
        <div class="actions"><button class="ghost mono-ghost" onclick={() => editing = false} disabled={busy}>Cancel</button><button class="cta mono-cta" onclick={saveSlot} disabled={busy || !slotDays.length || editorConflicts.length > 0}>{busy ? 'Saving…' : 'Save study time'}</button></div>
      </div>
    {/if}
    <ul class="slot-list">{#each slots as slot (slot.id)}<li><div class="slot-time mono">{time(slot.hour,slot.minute)}</div><div class="slot-copy"><strong>{days(slot.weekdays)}</strong><small>{slot.in_progress ? 'Session in progress' : slot.owed ? 'Due now' : !program.enabled || !slot.enabled ? 'Paused' : `Next: ${slot.next_fire_at}`}<span class="source">{slot.source === 'manual' ? 'Manual' : 'Planned'}</span></small></div><div class="slot-actions">{#if slot.owed}<button class="ghost mono-ghost" disabled={opening || busy || !program.enabled} onclick={() => onstart(slot.id)}>Start<ArrowRight size={12} /></button>{/if}<button class="icon" aria-label={`Edit ${days(slot.weekdays)} at ${time(slot.hour,slot.minute)}`} disabled={busy || editing} onclick={() => edit(slot.id)}><Pencil size={14} /></button><button class="icon" aria-label={`Delete ${days(slot.weekdays)} at ${time(slot.hour,slot.minute)}`} disabled={busy} onclick={() => remove(slot.id)}><Trash2 size={14} /></button></div></li>{:else}{#if !editing}<li class="empty"><CalendarClock size={28} /><h4>Your week starts here</h4><p>Add a recurring time or let the planner fit sessions into your availability.</p><button class="cta mono-cta" onclick={() => edit()}><Plus size={13} />Add your first study time</button></li>{/if}{/each}</ul>
    {#if appointments.length}<div class="list-heading"><span class="mono">APPOINTMENTS · TODAY AND MISSED</span></div><ul class="appointment-list">{#each appointments as appointment (appointment.id)}<li><span class="mono">{appointment.local_date} {appointment.local_time}</span><span class="disposition" class:missed={appointment.disposition === 'missed'} class:done={appointment.disposition === 'completed'}>{appointment.disposition}</span>{#if appointment.make_up}<span class="appointment-actions"><button class="ghost mono-ghost" disabled={opening || busy || !program.enabled} onclick={() => onmakeup?.(appointment.id)}>Make up</button><button class="ghost mono-ghost" disabled={busy} onclick={() => skipAppointment(appointment.id)}>Skip</button></span>{/if}</li>{/each}</ul>{/if}
  </div>
  <div id={`${uid}-plan-panel`} role="tabpanel" aria-labelledby={`${uid}-plan`} hidden={view !== 'plan'} tabindex="0">
    <p class="planner-intro">Set a weekly target and the times you’re free. Preview a timetable before saving. A saved plan replaces this class’s planned times and keeps manual times.</p>
    <form onsubmit={event => { event.preventDefault(); void plan(false); }}>
      <fieldset disabled={busy}><legend class="sr-only">Weekly plan preferences</legend>
        <div class="plan-fields"><label><span>Learning goal <small>Optional</small></span><textarea rows="2" bind:value={goal} placeholder="What would you like to work towards?"></textarea></label><label><span>Minutes per week</span><input type="number" min="0" max="2100" required bind:value={target} /><small>{program.session_minutes} minutes per session</small></label></div>
        <div class="list-heading"><span class="mono">AVAILABILITY</span><button type="button" class="ghost mono-ghost" onclick={() => windows = [...windows,{weekdays:[6,7],start:'10:00',end:'11:30'}]}><Plus size={13} />Add window</button></div>
        <div class="windows">{#each windows as window, index}<div class="window"><div class="window-header"><span class="mono">WINDOW {String(index+1).padStart(2,'0')}</span><button type="button" class="icon" disabled={windows.length === 1} aria-label={`Remove availability window ${index+1}`} onclick={() => windows = windows.filter((_,i) => i !== index)}><Trash2 size={14} /></button></div><div class="days" role="group" aria-label={`Days for window ${index+1}`}>{#each DAYS as day, i}<button type="button" class:active={window.weekdays.includes(i+1)} aria-pressed={window.weekdays.includes(i+1)} onclick={() => window.weekdays = flip(window.weekdays,i+1)}>{day}</button>{/each}</div><div class="window-times"><TimePicker compact cron={false} label={`Window ${index+1} start time`} bind:value={window.start} /><span>to</span><TimePicker compact cron={false} label={`Window ${index+1} end time`} bind:value={window.end} /></div>{#if !window.weekdays.length}<p class="error">Choose at least one day.</p>{/if}{#if window.start >= window.end}<p class="error">End time must be later than start time.</p>{/if}</div>{/each}</div>
      </fieldset>
      {#if preview}<div class="preview" class:stale={!currentPreview} class:conflict={currentPreview && previewConflicts.length > 0}><span class="mono">{currentPreview ? 'TIMETABLE PREVIEW' : 'PREVIEW OUT OF DATE'}</span><h4>{preview.total_weekly_minutes} minutes per week</h4><p>{!currentPreview ? 'Your preferences changed. Preview again before saving.' : preview.meets_target ? 'This timetable meets your weekly target.' : `This timetable is short of your ${preview.target_weekly_minutes} minute target.`}</p><ul>{#each preview.slots as slot}<li><strong class="mono">{time(slot.hour,slot.minute)}</strong><span>{days(slot.weekdays)}</span></li>{/each}</ul>{#if previewConflicts.length}<div class="conflicts" role="alert"><span class="mono"><AlertTriangle size={12} />OVERLAPS ANOTHER STUDY TIME</span><ul>{#each previewConflicts as conflict}<li>{WEEKDAY_NAMES[conflict.weekday-1]} {time(conflict.hour,conflict.minute)} overlaps {describeConflict(conflict)}</li>{/each}</ul><p>Change the availability windows or the other class’s times before saving this timetable.</p></div>{/if}</div>{/if}
      <div class="actions"><button type="submit" class="ghost mono-ghost" disabled={busy || !validWindows}>{busy ? 'Working…' : 'Preview timetable'}</button><button type="button" class="cta mono-cta" disabled={busy || !currentPreview || !validWindows || previewConflicts.length > 0} onclick={() => plan(true)}>Save weekly schedule</button></div>
    </form>
  </div>
</div>
<style>
  .schedule { max-width: 1000px; margin: 0 auto; } header { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 16px; margin-bottom: 22px; } h3 { margin: 0 0 7px; font: 23px var(--font-display); } p { color: var(--muted); font-size: 13px; line-height: 1.65; margin: 0; }
  .switcher { display: flex; border: 1px solid var(--node-border); border-radius: var(--radius-control); padding: 3px; background: var(--bg); gap: 3px; } .switcher button { display: flex; align-items: center; gap: 6px; padding: 9px 10px; border: 1px solid transparent; border-radius: var(--radius-detail); background: none; color: var(--muted); font: 10px var(--font-mono); cursor: pointer; } .switcher button.selected { border-color: var(--node-border); color: var(--violet-fg); background: var(--surface-2); }
  [role='tabpanel'][hidden] { display: none; } [role='tabpanel']:focus-visible { outline: 1px solid var(--violet); outline-offset: 4px; }
  .notice, .message { padding: 12px 14px; border-left: 2px solid var(--accent); background: var(--surface); margin-bottom: 18px; font-size: 12px; } .message { border-color: var(--led-ok); } .error { color: var(--led-err); font-size: 12px; margin: 12px 0; overflow-wrap: anywhere; }
  .list-heading { display: flex; justify-content: space-between; align-items: center; gap: 12px; margin: 18px 0 12px; } .list-heading > span { font-size: 10px; color: var(--muted); letter-spacing: .7px; } .list-heading button { font-size: 11px; }
  .slot-list { list-style: none; padding: 0; margin: 0; display: grid; gap: 10px; } .slot-list li { display: flex; align-items: center; gap: 18px; padding: 16px; background: var(--bg); border: 1px solid var(--node-border); border-radius: var(--radius-panel); } .slot-time { color: var(--accent); font-size: 20px; letter-spacing: -1px; } .slot-copy { min-width: 0; flex: 1; } .slot-copy strong { font-size: 13px; font-weight: 500; } .slot-copy small { display: flex; flex-wrap: wrap; gap: 8px; color: var(--muted); font-size: 11px; line-height: 1.5; margin-top: 6px; } .source { color: var(--violet-fg); } .slot-actions { display: flex; gap: 6px; }
  .appointment-list { list-style: none; padding: 0; margin: 0; display: grid; gap: 8px; } .appointment-list li { display: flex; align-items: center; gap: 12px; font-size: 12px; color: var(--muted); } .disposition { color: var(--fg); font-size: 11px; } .disposition.missed { color: var(--accent); } .disposition.done { color: var(--led-ok); } .appointment-actions { display: flex; gap: 6px; margin-left: auto; } .appointment-actions button { font-size: 10px; }
  .icon { display: inline-grid; place-items: center; width: 32px; height: 32px; border: 1px solid var(--node-border); border-radius: var(--radius-control); background: var(--node-bg); color: var(--muted); cursor: pointer; }
  .slot-list li.empty { display: flex; flex-direction: column; text-align: center; gap: 12px; padding: 42px 24px; border-style: dashed; } .empty :global(svg) { color: var(--accent); } .empty h4 { font: 22px var(--font-display); margin: 0; } .empty p { max-width: 350px; } .empty button { margin-top: 8px; }
  .editor { background: var(--surface); border: 1px solid var(--violet); border-radius: var(--radius-panel); padding: 20px; margin-bottom: 18px; } h4 { margin: 0 0 18px; font: 19px var(--font-display); } fieldset { padding: 0; margin: 0; border: 0; min-width: 0; } .editor-grid { display: flex; flex-wrap: wrap; gap: 24px; align-items: center; } .label, label > span { display: block; color: var(--muted); font-size: 12px; margin-bottom: 10px; }
  .days { display: flex; flex-wrap: wrap; gap: 5px; } .days button { padding: 9px 8px; border: 1px solid var(--node-border); background: var(--bg); color: var(--muted); border-radius: var(--radius-control); font: 10px var(--font-mono); cursor: pointer; } .days button.active { border-color: var(--violet); background: var(--violet-bg); color: var(--violet-fg); }
  .actions { display: flex; flex-wrap: wrap; gap: 10px; justify-content: flex-end; margin-top: 20px; } .actions button { font-size: 11px; }
  .planner-intro { max-width: 740px; margin-bottom: 20px; } .plan-fields { display: grid; grid-template-columns: minmax(0, 1fr) 170px; gap: 18px; } label { min-width: 0; } label small { color: var(--muted); font-size: 10px; } label > small { display: block; margin-top: 8px; } textarea, input { width: 100%; padding: 12px; border: 1px solid var(--node-border); border-radius: var(--radius-control); background: var(--bg); color: var(--fg); font: 13px/1.5 var(--font-body); } textarea { resize: vertical; min-height: 70px; }
  .windows { display: grid; grid-template-columns: repeat(auto-fit,minmax(min(100%,300px),1fr)); gap: 12px; } .window { border: 1px solid var(--node-border); border-radius: var(--radius-panel); padding: 16px; background: var(--surface); } .window-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 14px; } .window-header span { font-size: 10px; color: var(--muted); } .window-times { display: flex; align-items: center; gap: 12px; margin-top: 14px; color: var(--muted); font-size: 12px; }
  .preview { border: 1px solid var(--led-ok); border-radius: var(--radius-panel); padding: 20px; margin-top: 20px; background: var(--bg); } .preview.stale { border-color: var(--accent); } .preview.conflict { border-color: var(--accent); }
  .conflicts { margin-top: 14px; padding: 12px 14px; border-left: 2px solid var(--accent); background: var(--surface); font-size: 12px; } .conflicts > span { display: inline-flex; align-items: center; gap: 6px; font-size: 10px; color: var(--accent); letter-spacing: .7px; } .conflicts ul { margin: 8px 0; padding-left: 18px; display: block; } .conflicts li { display: list-item; margin: 4px 0; color: var(--fg); font-size: 12px; } .conflicts p { margin: 0; } .preview > span { font-size: 10px; color: var(--led-ok); } .preview.stale > span { color: var(--accent); } .preview h4 { margin: 12px 0 8px; } .preview ul { padding: 0; list-style: none; display: grid; gap: 8px; margin-bottom: 0; } .preview li { display: flex; gap: 12px; color: var(--muted); font-size: 12px; } .preview strong { color: var(--fg); font-weight: 400; }
  button:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; } button:disabled { opacity: .45; cursor: default; } .sr-only { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); }
  @media(max-width:800px) { .plan-fields { grid-template-columns: 1fr; } .plan-fields input { max-width: 180px; } .slot-list li { gap: 12px; flex-wrap: wrap; } .slot-time { font-size: 18px; } }
</style>
