<script lang="ts">
  import { untrack } from 'svelte';
  import { api, type ClassroomProgramView, type CurriculumMapView, type FocusArea } from '../../ipc';
  import type { AcceptedPath } from '../../contracts/classes';
  import { app } from '../../stores.svelte';
  import { courseDefinition } from '../../catalog';
  import { LayoutDashboard, Settings2, Compass, BookOpen, CalendarClock, Play, ArrowRight } from 'lucide-svelte';
  import CourseGlyph from '../../components/CourseGlyph.svelte';
  import CurriculumMap from '../../components/CurriculumMap.svelte';
  import ClassSettings from './ClassSettings.svelte';
  import ClassSchedule from './ClassSchedule.svelte';
  import EnrollmentSetup from './EnrollmentSetup.svelte';
  import PathPreview from './PathPreview.svelte';
  import type { ClassTab } from './class-navigation';
  let { program, initialTab = 'overview', ontabchange }: { program: ClassroomProgramView; initialTab?: ClassTab; ontabchange?: (tab: ClassTab) => void } = $props();
  const uid = $props.id();
  const course = $derived(courseDefinition(program.subject_id)!);
  const preparing = $derived(app.preparingClass !== null);
  const active = $derived(app.state?.active_classroom_sessions.find(s => s.subject_id === program.subject_id));
  const slots = $derived(app.state?.classroom_slots.filter(s => s.subject_id === program.subject_id) ?? []);
  let tab = $state<ClassTab>(untrack(() => initialTab));
  let visited = $state<ClassTab[]>(untrack(() => [initialTab]));
  let opening = $state(false);
  /** A shared-runtime lesson whose preparation has not finished: Learn now retries it. */
  const pending = $derived(!!active && active.runtime === 'study' && ['planned', 'preparing'].includes(active.lifecycle));
  async function open(slotId: number | null = null, occurrenceId: string | null = null) { if (opening) return; opening = true; try { if (active && !pending) await app.resumeClass(program.subject_id); else await app.startClass(program.subject_id, slotId, program.completed, occurrenceId); } finally { opening = false; } }
  let busy = $state(false), error = $state('');
  async function discard() {
    if (!active || busy) return;
    busy = true; error = '';
    try { await api.skipClassLesson(active.session_id); await app.refresh(); } catch (cause) { error = String(cause); } finally { busy = false; }
  }
  let map = $state<CurriculumMapView | null>(null), mapLoading = $state(false), mapError = $state('');
  let path = $state<AcceptedPath | null>(null), pathLoading = $state(false), pathError = $state(''), pathLoaded = $state(false), editingPath = $state(false);
  const tabs = [
    { id: 'overview', label: 'Overview', icon: LayoutDashboard },
    { id: 'settings', label: 'Settings', icon: Settings2 },
    { id: 'entry', label: 'Starting point', icon: Compass },
    { id: 'curriculum', label: 'Curriculum', icon: BookOpen },
    { id: 'schedule', label: 'Schedule', icon: CalendarClock },
  ] as const;
  function select(id: ClassTab) { tab = id; ontabchange?.(id); if (!visited.includes(id)) visited = [...visited, id]; }
  function key(event: KeyboardEvent, index: number) {
    if (!['ArrowLeft','ArrowRight','Home','End'].includes(event.key)) return;
    event.preventDefault();
    const next = event.key === 'Home' ? 0 : event.key === 'End' ? tabs.length - 1 : (index + (event.key === 'ArrowRight' ? 1 : -1) + tabs.length) % tabs.length;
    select(tabs[next].id); document.getElementById(`${uid}-tab-${tabs[next].id}`)?.focus();
  }
  async function toggle() {
    if (busy) return;
    if (!program.enabled && !slots.some(slot => slot.enabled)) { select('schedule'); return; }
    busy = true; error = '';
    try {
      await api.configureClassroomProgram({ subject_id: program.subject_id, enabled: !program.enabled, agent: program.agent, model: program.model, custom_agent_bin: program.custom_agent_bin, session_minutes: program.session_minutes, start_level: program.language_progress?.start_level ?? null, target_level: program.language_progress?.target_level ?? null, weekly_minutes: program.language_progress?.weekly_minutes ?? null });
      await app.refresh();
    } catch (cause) { error = String(cause); } finally { busy = false; }
  }
  async function loadMap() { mapLoading = true; mapError = ''; try { map = await api.getCurriculumMap(program.subject_id as FocusArea); } catch (cause) { mapError = String(cause); } finally { mapLoading = false; } }
  async function loadPath() { pathLoading = true; pathError = ''; try { path = await api.getClassPath(program.subject_id); pathLoaded = true; } catch (cause) { pathError = String(cause); } finally { pathLoading = false; } }
  $effect(() => { if (tab === 'curriculum' && program.kind === 'engineering') untrack(() => { if (!map && !mapLoading && !mapError) void loadMap(); }); });
  $effect(() => { if (tab === 'entry') untrack(() => { if (!pathLoaded && !pathLoading && !pathError) void loadPath(); }); });
  async function setupClosed() { editingPath = false; await loadPath(); }
</script>

<article class="class-detail" aria-label={`${program.label} controls`}>
  <header class="class-header">
    <div class="identity"><CourseGlyph courseId={program.subject_id} size={46} /><div><div class="eyebrow mono">{program.kind === 'language' ? 'LANGUAGE' : 'ENGINEERING'} / {program.short_code}<span class:enabled={program.enabled} class="status">{program.completed ? 'Completed' : program.enabled ? 'Active' : 'Inactive'}</span></div><h2>{program.label}</h2><p>{program.native_label}</p></div></div>
    <div class="header-actions"><button class="ghost mono-ghost" onclick={toggle} disabled={busy || opening || preparing}>{busy ? 'Saving…' : program.enabled ? 'Pause class' : slots.some(slot => slot.enabled) ? 'Activate class' : 'Set study times'}</button><button class="cta mono-cta" disabled={(!program.enabled && !active) || opening || preparing} onclick={() => open()}><Play size={13} />{opening ? 'Opening…' : pending ? 'Retry preparation' : active ? 'Resume' : program.completed ? 'Revisit' : 'Learn now'}</button>{#if pending}<button class="ghost mono-ghost" onclick={discard} disabled={busy || opening || preparing}>Discard lesson</button>{/if}</div>
  </header>
  {#if error}<p class="banner error" role="alert">{error}</p>{/if}
  <div class="tabs" role="tablist" aria-label={`${program.label} sections`}>
    {#each tabs as item, index}<button role="tab" id={`${uid}-tab-${item.id}`} aria-selected={tab === item.id} aria-controls={`${uid}-panel-${item.id}`} tabindex={tab === item.id ? 0 : -1} class:selected={tab === item.id} onclick={() => select(item.id)} onkeydown={event => key(event,index)}><item.icon size={14} /><span>{item.label}</span>{#if item.id === 'schedule' && slots.length}<small>{slots.length}</small>{/if}</button>{/each}
  </div>
  {#each tabs as item}
    <div class="tab-content" id={`${uid}-panel-${item.id}`} role="tabpanel" aria-labelledby={`${uid}-tab-${item.id}`} hidden={tab !== item.id} tabindex="0">
      {#if visited.includes(item.id)}
        {#if item.id === 'overview'}
          <div class="overview">
            <div class="overview-intro"><span class="eyebrow mono">THE COURSE</span><h3>{course.summary}</h3><p>{course.outcome}</p></div>
            <div class="metrics"><div><span>Progress</span><strong>{Math.round(program.progress * 100)}<small>%</small></strong><p>{program.progress_label}</p><div class="progress-track" role="progressbar" aria-label="Course progress" aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(program.progress * 100)}><i style:width={`${program.progress * 100}%`}></i></div></div><div><span>Session length</span><strong>{program.session_minutes}<small>min</small></strong><p>{program.agent} / {program.model}</p></div><div><span>Study times</span><strong>{slots.length}</strong><button class="inline" onclick={() => select('schedule')}>{slots.length ? 'Manage schedule' : 'Set a study time'}<ArrowRight size={13} /></button></div></div>
            <div class="overview-grid"><section class="info-card"><span class="eyebrow mono">YOUR STARTING POINT</span><h4>{program.accepted_path?.entry_label ?? 'Begin where you are'}</h4><p>{program.accepted_path ? `Accepted path · revision ${program.accepted_path.revision}. Review your route and prerequisite refreshers.` : 'Start with the foundations, choose a stage or take a short check to find your level.'}</p><button class="ghost mono-ghost" onclick={() => select('entry')}>{program.accepted_path ? 'View personal path' : 'Set starting point'}<ArrowRight size={13} /></button></section><section class="info-card"><span class="eyebrow mono">WORKING ENVIRONMENT</span><p class="environment">{course.environment}</p>{#if course.prerequisite_courses.length}<p>Suggested preparation: {course.prerequisite_courses.map(id => courseDefinition(id)?.label ?? id).join(', ')} or equivalent experience.</p>{/if}<button class="ghost mono-ghost" onclick={() => select('curriculum')}>Explore curriculum<ArrowRight size={13} /></button></section></div>
            {#if pending}<p class="notice">Lesson preparation did not finish: <strong>{active?.title}</strong>. Retry it or discard it from the class header; nothing was graded.</p>{:else if active}<p class="notice">Saved session: <strong>{active.title}</strong>. Resume from the class header.</p>{:else if !program.enabled}<p class="notice">Set your starting point and add a study time, then activate this class when you’re ready.</p>{/if}
          </div>
        {:else if item.id === 'settings'}<ClassSettings {program} />
        {:else if item.id === 'schedule'}<ClassSchedule {program} opening={opening || preparing} onstart={open} onmakeup={(id) => open(null, id)} />
        {:else if item.id === 'entry'}
          {#if pathLoading}<p class="loading" role="status">Loading your starting point…</p>{:else if pathError}<p class="error" role="alert">{pathError}</p><button class="ghost mono-ghost" onclick={loadPath}>Retry</button>{:else if pathLoaded}
            {#if path && !editingPath}<PathPreview embedded path={path.recommendation} acceptedRevision={path.revision} onclose={() => select('overview')} onfoundations={() => editingPath = true} />{:else}<EnrollmentSetup embedded courseId={program.subject_id} onclose={setupClosed} />{/if}
          {/if}
        {:else if item.id === 'curriculum'}
          {#if program.kind === 'engineering'}
            {#if map}<CurriculumMap {map} embedded />{:else if mapLoading}<p class="loading" role="status">Loading curriculum…</p>{:else if mapError}<p class="error" role="alert">{mapError}</p><button class="ghost mono-ghost" onclick={loadMap}>Retry</button>{/if}
          {:else}
            <div class="language-map"><span class="eyebrow mono">LANGUAGE PATH</span><h3>{program.label} · learning bands</h3><p>{course.outcome}</p>{#if program.language_progress}<p class="notice">Current band: {program.language_progress.current_level} · Target: {program.language_progress.target_level} · {program.progress_label}</p>{/if}<ol>{#each course.entry_points as entry}<li><span class="mono">{entry.id}</span><div><h4>{entry.label}</h4></div></li>{/each}</ol><button class="ghost mono-ghost" onclick={() => select('entry')}>Review starting point<ArrowRight size={13} /></button></div>
          {/if}
        {/if}
      {/if}
    </div>
  {/each}
</article>

<style>
  .class-detail { display: flex; flex-direction: column; flex: 1; min-height: 0; min-width: 0; overflow: hidden; }
  .class-header { flex-shrink: 0; padding: 22px 24px; display: flex; justify-content: space-between; align-items: center; gap: 18px; background: linear-gradient(110deg,var(--surface),var(--node-bg)); }
  .identity { display: flex; align-items: center; gap: 14px; min-width: 0; } .eyebrow { font-size: 10px; color: var(--muted); letter-spacing: .7px; } h2 { font: 26px/1.2 var(--font-display); margin: 7px 0 5px; } .identity p { font: 10px var(--font-mono); margin: 0; color: var(--muted); }
  .status { color: var(--muted); background: var(--bg); border: 1px solid var(--node-border); border-radius: var(--radius-detail); margin-left: 10px; padding: 2px 6px; letter-spacing: 0; } .status.enabled { color: var(--led-ok); }
  .header-actions { display: flex; flex-wrap: wrap; gap: 8px; flex-shrink: 0; } .header-actions button { font-size: 11px; white-space: nowrap; padding: 10px 12px; }
  .tabs { display: flex; gap: 6px; padding: 0 24px; border-top: 1px solid var(--node-divider); border-bottom: 1px solid var(--node-border); flex-shrink: 0; overflow-x: auto; background: var(--node-bg); }
  .tabs button { position: relative; display: flex; align-items: center; justify-content: center; gap: 7px; flex-shrink: 0; border: 0; border-bottom: 2px solid transparent; border-radius: 0; background: none; padding: 16px 9px 14px; font: 11px var(--font-mono); color: var(--muted); cursor: pointer; white-space: nowrap; }
  .tabs button.selected { color: var(--accent); border-bottom-color: var(--accent); } .tabs button:hover { color: var(--fg); } .tabs small { color: var(--muted); padding: 1px 5px; background: var(--surface-2); border-radius: var(--radius-detail); font-size: 9px; }
  .tab-content { flex: 1; min-height: 0; min-width: 0; overflow-y: auto; overflow-x: hidden; overscroll-behavior: contain; scrollbar-gutter: stable; padding: 24px; scroll-padding-top: 24px; }
  .tab-content[hidden] { display: none; } .tab-content:focus-visible { outline: 1px solid var(--violet); outline-offset: -3px; }
  button:focus-visible { outline: 2px solid var(--accent); outline-offset: -2px; }
  .overview, .language-map { max-width: 1000px; margin: 0 auto; } .overview-intro h3 { font: 25px/1.45 var(--font-display); margin: 10px 0; max-width: 760px; } p { font-size: 13px; line-height: 1.7; color: var(--muted); }
  .metrics { display: grid; grid-template-columns: 1.3fr 1fr 1fr; border: 1px solid var(--node-border); border-radius: var(--radius-panel); margin: 24px 0; overflow: hidden; }
  .metrics > div { padding: 18px; min-width: 0; } .metrics > div + div { border-left: 1px solid var(--node-border); } .metrics span { display: block; color: var(--muted); font: 10px var(--font-mono); } .metrics strong { display: block; font: 30px var(--font-display); margin-top: 12px; } .metrics strong small { font: 12px var(--font-mono); color: var(--muted); margin-left: 6px; } .metrics p { font-size: 11px; margin: 8px 0; overflow-wrap: anywhere; }
  .progress-track { height: 3px; background: var(--node-border); margin-top: 12px; border-radius: 2px; overflow: hidden; } .progress-track i { height: 100%; display: block; background: var(--led-ok); }
  .inline { display: inline-flex; align-items: center; gap: 6px; padding: 0; margin-top: 9px; background: none; border: 0; color: var(--accent); font-size: 11px; text-align: left; cursor: pointer; }
  .overview-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; } .info-card { border: 1px solid var(--node-border); border-radius: var(--radius-panel); padding: 20px; background: var(--bg); } h4 { margin: 14px 0 8px; font: 19px var(--font-display); } .info-card p { font-size: 12px; } .info-card button { margin-top: 10px; font-size: 11px; } .environment { margin-top: 15px; }
  .notice { padding: 12px 15px; border-left: 2px solid var(--violet); background: var(--surface); font-size: 12px; margin-top: 20px; }
  .banner { margin: 0; padding: 9px 24px; color: var(--accent); font-size: 11px; background: var(--surface); flex-shrink: 0; } .error { color: var(--led-err); overflow-wrap: anywhere; } .loading { color: var(--muted); }
  .language-map h3 { font: 25px var(--font-display); } .language-map ol { list-style: none; padding: 0; display: grid; gap: 12px; } .language-map li { display: flex; gap: 16px; border: 1px solid var(--node-border); border-radius: var(--radius-panel); padding: 16px; } .language-map li > span { color: var(--accent); font-size: 12px; padding-top: 4px; } .language-map h4 { margin: 0; }
  @media(max-width:1100px) { .class-header { flex-wrap: wrap; padding: 18px; } .tabs { padding: 0 14px; gap: 0; } .tabs button { padding: 14px 9px 12px; } .overview-grid { grid-template-columns: 1fr; } }
  @media(max-width:700px) { .class-header { padding: 16px; gap: 12px; } .identity :global(svg) { display: none; } h2 { font-size: 23px; } .tab-content { padding: 16px; } .tabs button :global(svg) { display: none; } .metrics { grid-template-columns: 1fr; } .metrics > div + div { border-left: 0; border-top: 1px solid var(--node-border); } .overview-intro h3 { font-size: 21px; } }
</style>
