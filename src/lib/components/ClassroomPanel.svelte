<script lang="ts">
  import {
    api,
    type AgentId,
    type CefrLevel,
    type ClassroomPlanView,
    type ClassroomProgramView,
    type ClassroomSubjectId,
    type CurriculumMapView,
    type FocusArea,
    type ModelId,
  } from '../ipc';
  import { courseDefinition } from '../catalog';
  import { app } from '../stores.svelte';
  import RunnerSetup from '../features/runners/RunnerSetup.svelte';
  import CurriculumMap from './CurriculumMap.svelte';
  import TimePicker from './TimePicker.svelte';
  import Dropdown from './Dropdown.svelte';
  import CourseGlyph from './CourseGlyph.svelte';
  import {
    BookOpen,
    CalendarClock,
    ChevronDown,
    ChevronUp,
    Clock3,
    Plus,
    Settings2,
    Search,
    X,
    Trash2,
  } from 'lucide-svelte';

  const DAYS = [
    { id: 1, short: 'M', label: 'Monday' },
    { id: 2, short: 'T', label: 'Tuesday' },
    { id: 3, short: 'W', label: 'Wednesday' },
    { id: 4, short: 'T', label: 'Thursday' },
    { id: 5, short: 'F', label: 'Friday' },
    { id: 6, short: 'S', label: 'Saturday' },
    { id: 7, short: 'S', label: 'Sunday' },
  ];
  const LEVELS: CefrLevel[] = ['A1', 'A2', 'B1', 'B2'];
  const levelOptions = LEVELS.map((value) => ({ value, label: value }));
  let { mode = 'classes', onsetup }: { mode?: 'classes' | 'schedule'; onsetup?: (id: ClassroomSubjectId) => void } = $props();
  let query = $state('');
  let courseSearch = $state<HTMLInputElement>();
  let filter = $state<'all' | 'active' | 'paused' | 'completed'>('all');

  const programs = $derived(app.state?.classroom_programs ?? []);
  const slots = $derived(app.state?.classroom_slots ?? []);
  const active = $derived(app.state?.active_classroom_sessions ?? []);
  const primaryOwed = $derived(app.state?.owed ?? false);
  const visiblePrograms = $derived(programs.filter((program) => {
    if (mode === 'schedule') return program.enabled || slots.some((slot) => slot.subject_id === program.subject_id);
    const search = query.trim().toLocaleLowerCase();
    if (search && !`${program.label} ${program.native_label} ${courseDefinition(program.subject_id)?.summary ?? ''}`.toLocaleLowerCase().includes(search)) return false;
    if (filter === 'active') return program.enabled && !program.completed;
    if (filter === 'paused') return !program.enabled && !program.completed && (program.progress > 0 || active.some((session) => session.subject_id === program.subject_id) || slots.some((slot) => slot.subject_id === program.subject_id));
    if (filter === 'completed') return program.completed;
    return true;
  }));

  let expanded = $state(true);
  let configuring = $state<ClassroomSubjectId | null>(null);
  let slotSubject = $state<ClassroomSubjectId | null>(null);
  let slotId = $state<number | null>(null);
  let slotTime = $state('07:30');
  let slotDays = $state<number[]>([1, 2, 3, 4, 5]);
  let saving = $state(false);
  let slotSaving = $state(false);

  let draftAgent = $state('claude');
  let draftModel = $state('opus');
  let draftCustom = $state('');
  let draftMinutes = $state(30);
  let draftStart = $state<CefrLevel>('A1');
  let draftTarget = $state<CefrLevel>('A2');
  let draftWeekly = $state(210);

  type PlanWindowDraft = { weekdays: number[]; start: string; end: string };
  let planningSubject = $state<ClassroomSubjectId | null>(null);
  let planGoal = $state('');
  let planTargetMinutes = $state(150);
  let planWindows = $state<PlanWindowDraft[]>([]);
  let planPreview = $state<ClassroomPlanView | null>(null);
  let planError = $state('');
  let planPreviewing = $state(false);
  let planSaving = $state(false);
  let curriculumMap = $state<CurriculumMapView | null>(null);
  let mapLoading = $state<ClassroomSubjectId | null>(null);

  function slotsFor(subjectId: ClassroomSubjectId) {
    return slots.filter((slot) => slot.subject_id === subjectId);
  }

  function activeFor(subjectId: ClassroomSubjectId) {
    return active.find((session) => session.subject_id === subjectId);
  }

  function loadDraft(program: ClassroomProgramView) {
    configuring = program.subject_id;
    draftAgent = program.agent;
    draftModel = program.model;
    draftCustom = program.custom_agent_bin;
    draftMinutes = program.session_minutes;
    draftStart = program.language_progress?.start_level ?? 'A1';
    draftTarget = program.language_progress?.target_level ?? 'A2';
    draftWeekly = program.language_progress?.weekly_minutes ?? 210;
  }

  async function persist(program: ClassroomProgramView, enabled = program.enabled) {
    saving = true;
    try {
      await api.configureClassroomProgram({
        subject_id: program.subject_id,
        enabled,
        agent: draftAgent as AgentId,
        model: draftModel as ModelId,
        custom_agent_bin: draftCustom,
        session_minutes: draftMinutes,
        start_level: program.kind === 'language' ? draftStart : null,
        target_level: program.kind === 'language' ? draftTarget : null,
        weekly_minutes: program.kind === 'language' ? draftWeekly : null,
      });
      configuring = null;
      await app.refresh();
    } catch (error) {
      app.error = String(error);
    } finally {
      saving = false;
    }
  }

  async function toggleProgram(program: ClassroomProgramView) {
    loadDraft(program);
    await persist(program, !program.enabled);
  }

  function editSlot(subjectId: ClassroomSubjectId, id?: number) {
    const existing = id == null ? null : slots.find((slot) => slot.id === id);
    slotSubject = subjectId;
    slotId = existing?.id ?? null;
    slotTime = existing
      ? `${String(existing.hour).padStart(2, '0')}:${String(existing.minute).padStart(2, '0')}`
      : '07:30';
    slotDays = existing?.weekdays ?? [1, 2, 3, 4, 5];
  }

  function toggleDay(day: number) {
    slotDays = slotDays.includes(day)
      ? slotDays.filter((candidate) => candidate !== day)
      : [...slotDays, day].sort();
  }

  async function saveSlot() {
    if (!slotSubject || slotDays.length === 0) return;
    const [hour, minute] = slotTime.split(':').map(Number);
    slotSaving = true;
    try {
      await api.upsertClassroomSlot({
        id: slotId,
        subject_id: slotSubject,
        hour,
        minute,
        weekdays: slotDays,
        enabled: true,
      });
      slotSubject = null;
      slotId = null;
      await app.refresh();
    } catch (error) {
      app.error = String(error);
    } finally {
      slotSaving = false;
    }
  }

  async function removeSlot(id: number) {
    try {
      await api.deleteClassroomSlot(id);
      await app.refresh();
    } catch (error) {
      app.error = String(error);
    }
  }

  function daySummary(days: number[]) {
    if (days.length === 7) return 'every day';
    if (days.join(',') === '1,2,3,4,5') return 'weekdays';
    return days.map((day) => DAYS.find((candidate) => candidate.id === day)?.short).join(' ');
  }

  function openPlanner(program: ClassroomProgramView) {
    planningSubject = program.subject_id;
    configuring = null;
    slotSubject = null;
    planGoal = program.learning_goal;
    planTargetMinutes =
      program.target_weekly_minutes || program.language_progress?.weekly_minutes || 150;
    planWindows = [{ weekdays: [1, 2, 3, 4, 5], start: '07:00', end: '08:00' }];
    planPreview = null;
    planError = '';
  }

  function addWindow() {
    planWindows = [...planWindows, { weekdays: [6, 7], start: '10:00', end: '11:30' }];
  }

  function removeWindow(index: number) {
    if (planWindows.length <= 1) return;
    planWindows = planWindows.filter((_, i) => i !== index);
  }

  function toggleWindowDay(index: number, day: number) {
    const window = planWindows[index];
    window.weekdays = window.weekdays.includes(day)
      ? window.weekdays.filter((candidate) => candidate !== day)
      : [...window.weekdays, day].sort();
  }

  function windowsPayload() {
    return planWindows.map((window) => {
      const [startHour, startMinute] = window.start.split(':').map(Number);
      const [endHour, endMinute] = window.end.split(':').map(Number);
      return {
        weekdays: window.weekdays,
        start_hour: startHour,
        start_minute: startMinute,
        end_hour: endHour,
        end_minute: endMinute,
      };
    });
  }

  async function previewPlan() {
    if (!planningSubject) return;
    planPreviewing = true;
    planError = '';
    try {
      planPreview = await api.planClassroomSchedule({
        subject_id: planningSubject,
        learning_goal: planGoal,
        target_weekly_minutes: planTargetMinutes,
        windows: windowsPayload(),
        commit: false,
      });
    } catch (error) {
      planError = String(error);
      planPreview = null;
    } finally {
      planPreviewing = false;
    }
  }

  async function savePlan() {
    if (!planningSubject) return;
    planSaving = true;
    planError = '';
    try {
      await api.planClassroomSchedule({
        subject_id: planningSubject,
        learning_goal: planGoal,
        target_weekly_minutes: planTargetMinutes,
        windows: windowsPayload(),
        commit: true,
      });
      planningSubject = null;
      planPreview = null;
      await app.refresh();
    } catch (error) {
      planError = String(error);
    } finally {
      planSaving = false;
    }
  }

  async function openCurriculum(program: ClassroomProgramView) {
    if (program.kind !== 'engineering') return;
    mapLoading = program.subject_id;
    try {
      curriculumMap = await api.getCurriculumMap(program.subject_id as FocusArea);
    } catch (error) {
      app.error = String(error);
    } finally {
      mapLoading = null;
    }
  }
</script>

<section class="classroom-panel" aria-labelledby="classroom-title">
  <button
    class="panel-trigger"
    type="button"
    aria-expanded={expanded}
    aria-controls="classroom-body"
    onclick={() => (expanded = !expanded)}
  >
    <span class="trigger-copy">
      <span class="trigger-icon"><BookOpen size={15} /></span>
      <span>
        <strong id="classroom-title">{mode === 'schedule' ? 'Class schedules' : 'Courses and classes'}</strong>
        <small>{mode === 'schedule' ? 'Recurring study times and weekly plans' : 'Explore nine courses and manage your learning'}</small>
      </span>
    </span>
    <span class="trigger-meta mono">
      {#if app.state?.classroom_due_count}
        <span class="due-count">{app.state.classroom_due_count} due</span>
      {:else}
        {programs.filter((program) => program.enabled).length} active
      {/if}
      {#if expanded}<ChevronUp size={15} />{:else}<ChevronDown size={15} />{/if}
    </span>
  </button>

  {#if expanded}
    <div id="classroom-body" class="classroom-body">
      {#if mode === 'classes'}<div class="classroom-intro">
        <div>
          <span class="eyebrow mono">COURSE CATALOG</span>
          <p>
            Explore a course and its curriculum, then enable a class to start learning.
            Each class keeps its own schedule, teacher preferences and progress.
          </p>
        </div>
      </div>
      <div class="catalog-toolbar"><div class="class-filters" aria-label="Filter classes">
        {#each ['all', 'active', 'paused', 'completed'] as choice}
          <button type="button" aria-pressed={filter === choice} class:chosen={filter === choice} onclick={() => (filter = choice as typeof filter)}>{choice === 'all' ? 'All courses' : choice[0].toUpperCase() + choice.slice(1)}</button>
        {/each}
      </div><div class="catalog-search" role="search"><Search size={13} /><input bind:this={courseSearch} type="search" bind:value={query} aria-label="Find a course" placeholder="Find a course…" />{#if query}<button class="clear-search" aria-label="Clear course search" onclick={() => { query = ''; courseSearch?.focus(); }}><X size={12} /></button>{/if}</div></div>{/if}
      {#if primaryOwed}
        <p class="primary-wins" role="status">
          Your daily study session is due now. Classroom starts and resumes unlock after the enforced
          session is completed or skipped; schedules and teacher settings remain available.
        </p>
      {/if}

      <div class="class-grid">
        {#each visiblePrograms as program (program.subject_id)}
          {@const running = activeFor(program.subject_id)}
          {@const course = courseDefinition(program.subject_id)}
          <article class:enabled={program.enabled} class:due={slotsFor(program.subject_id).some((s) => s.owed)} class="class-card">
            <header class="class-head">
              <CourseGlyph courseId={program.subject_id} />
              <span class="class-identity">
                <strong>{program.label}</strong>
                <small>{program.native_label}</small>
              </span>
              <span class:online={program.enabled} class:complete={program.completed} class="class-state mono">
                {program.completed ? 'COMPLETE' : program.enabled ? 'ACTIVE' : 'OFF'}
              </span>
            </header>

            {#if mode === 'classes'}
            {#if course}
              <p class="course-summary">{course.summary}</p>
              <details class="course-about">
                <summary>About this course</summary>
                <p>{course.outcome}</p>
                <p><strong>Environment:</strong> {course.environment}</p>
                {#if course.prerequisite_courses.length}
                  <p><strong>Recommended preparation:</strong> {course.prerequisite_courses.map((id) => courseDefinition(id)?.label ?? id).join(', ')} or equivalent experience.</p>
                {/if}
                <p><strong>Course stages:</strong> {course.entry_points.map((point) => point.label).join(' → ')}</p>
              </details>
            {/if}

            <div class="progress-copy">
              <span>{program.progress_label}</span>
              <span class="mono">{Math.round(program.progress * 100)}%</span>
            </div>
            <div
              class="progress-track"
              role="progressbar"
              aria-label={`${program.label} progress`}
              aria-valuemin="0"
              aria-valuemax="100"
              aria-valuenow={Math.round(program.progress * 100)}
            >
              <span style={`width:${Math.round(program.progress * 100)}%`}></span>
            </div>

            <div class="profile-line mono">
              <span>{program.agent} / {program.model}</span>
              <span>{program.session_minutes} min / session</span>
            </div>

            {#if program.language_progress}
              <p class="program-detail">
                {program.language_progress.current_level} → {program.language_progress.target_level}
                · {program.language_progress.weekly_minutes} min/week
              </p>
            {:else}
              <p class="program-detail">
                {program.session_minutes}-minute learning sessions.
              </p>
            {/if}

            {#if !program.enabled}
              <p class="disabled-hint">
                <Settings2 size={11} />
                Enable this class to start learning or add study times.
              </p>
            {/if}
            {/if}

            {#if running}
              <button
                class="resume-button"
                type="button"
                disabled={primaryOwed}
                title={primaryOwed ? 'complete or skip the due daily session first' : undefined}
                onclick={() => app.resumeClass(program.subject_id)}
              >
                resume · {running.title}
              </button>
            {/if}

            <ul class="slot-list" aria-label={`${program.label} schedule`}>
              {#each slotsFor(program.subject_id) as slot (slot.id)}
                <li class:owed={slot.owed}>
                  <button type="button" class="slot-main" onclick={() => editSlot(program.subject_id, slot.id)}>
                    <Clock3 size={12} />
                    <span class="mono">{String(slot.hour).padStart(2, '0')}:{String(slot.minute).padStart(2, '0')}</span>
                    <small>{daySummary(slot.weekdays)}</small>
                    {#if slot.owed}<em>due</em>{/if}
                  </button>
                  {#if slot.owed}
                    <button
                      class="text-action"
                      type="button"
                      disabled={!program.enabled || !!running || primaryOwed}
                      onclick={() => app.startClass(program.subject_id, slot.id, program.completed)}
                    >
                      Start scheduled class
                    </button>
                  {/if}
                  <button
                    class="slot-delete"
                    type="button"
                    aria-label={`Delete ${program.label} slot at ${slot.hour}:${String(slot.minute).padStart(2, '0')}`}
                    onclick={() => removeSlot(slot.id)}
                  >
                    <Trash2 size={12} />
                  </button>
                </li>
              {/each}
            </ul>

            <footer class="class-actions">
              <button
                class="start-button"
                type="button"
                disabled={!program.enabled || !!running || primaryOwed}
                title={primaryOwed
                  ? 'complete or skip the due daily session first'
                  : program.enabled
                    ? program.completed
                      ? 'every module is completed — revisit is opt-in'
                      : undefined
                    : `enable ${program.label} first`}
                onclick={() =>
                  app.startClass(program.subject_id, null, program.completed)}
              >
                {program.completed ? 'Revisit a lesson' : 'Learn now'}
              </button>
              <button class="power-button mono" type="button" onclick={() => toggleProgram(program)}>
                {program.enabled ? 'disable' : 'enable'}
              </button>
              <div class="class-tools">
                <button
                  class="text-action"
                  type="button"
                  disabled={!program.enabled}
                  title={program.enabled ? undefined : `enable ${program.label} first`}
                  onclick={() => editSlot(program.subject_id)}
                >
                  <Plus size={12} /> slot
                </button>
                <button class="text-action" type="button" onclick={() => loadDraft(program)}>
                  <Settings2 size={12} /> Class settings
                </button>
                {#if mode === 'classes' && onsetup}
                  <button class="text-action" type="button" onclick={() => onsetup?.(program.subject_id)}>Starting preferences</button>
                {/if}
                {#if mode === 'classes' && program.kind === 'engineering'}
                  <button
                    class="text-action"
                    type="button"
                    disabled={mapLoading === program.subject_id}
                    onclick={() => openCurriculum(program)}
                  >
                    <BookOpen size={12} />
                    {mapLoading === program.subject_id ? 'Loading…' : 'Curriculum'}
                  </button>
                {/if}
                <button
                  class="text-action"
                  type="button"
                  disabled={!program.enabled}
                  title={program.enabled ? undefined : `enable ${program.label} first`}
                  onclick={() => openPlanner(program)}
                >
                  <CalendarClock size={12} /> Plan schedule
                </button>
              </div>
            </footer>

            {#if configuring === program.subject_id}
              <div class="settings-pane">
                <div class="settings-title">
                  <span class="mono">CLASS SETTINGS · {program.short_code}</span>
                  <button type="button" onclick={() => (configuring = null)}>close</button>
                </div>
                <p>
                  This provider and prompt profile are scoped to {program.label}; changing them
                  does not change another class or the primary daily session.
                </p>
                <RunnerSetup
                  bind:agent={draftAgent}
                  bind:model={draftModel}
                  bind:customBin={draftCustom}
                  allowKeyEditing={false}
                />
                <label class="number-field">
                  <span>session minutes</span>
                  <input type="number" min="15" max="90" bind:value={draftMinutes} />
                </label>
                {#if program.kind === 'language'}
                  <div class="language-fields">
                    <Dropdown label="Start level" bind:value={draftStart} options={levelOptions} />
                    <Dropdown label="Target level" bind:value={draftTarget} options={levelOptions} />
                    <label>
                      <span>weekly minutes</span>
                      <input type="number" min="60" max="2100" step="30" bind:value={draftWeekly} />
                    </label>
                  </div>
                {/if}
                <button class="save-button" type="button" disabled={saving} onclick={() => persist(program)}>
                  {saving ? 'Saving…' : 'Save class settings'}
                </button>
              </div>
            {/if}

            {#if slotSubject === program.subject_id}
              <div class="slot-editor">
                <div class="settings-title">
                  <span class="mono">{slotId ? 'EDIT SLOT' : 'NEW SLOT'} · {program.short_code}</span>
                  <button type="button" onclick={() => (slotSubject = null)}>close</button>
                </div>
                <TimePicker bind:value={slotTime} />
                <fieldset>
                  <legend>practice days</legend>
                  <div class="day-row">
                    {#each DAYS as day}
                      <button
                        type="button"
                        class:active={slotDays.includes(day.id)}
                        aria-pressed={slotDays.includes(day.id)}
                        aria-label={day.label}
                        onclick={() => toggleDay(day.id)}
                      >{day.short}</button>
                    {/each}
                  </div>
                </fieldset>
                {#if slotDays.length === 0}<p class="field-error">Select at least one day.</p>{/if}
                <button class="save-button" type="button" disabled={slotSaving || slotDays.length === 0} onclick={saveSlot}>
                  {slotSaving ? 'saving…' : 'save class slot'}
                </button>
              </div>
            {/if}

            {#if planningSubject === program.subject_id}
              <div class="plan-pane">
                <div class="settings-title">
                  <span class="mono">PLAN SCHEDULE · {program.short_code}</span>
                  <button type="button" onclick={() => (planningSubject = null)}>close</button>
                </div>
                <p>
                  Say what you want from {program.label}, how many minutes a week you can commit,
                  and when you're actually free — the planner proposes a recurring timetable
                  inside those windows. This never touches slots you've placed by hand.
                </p>
                <label class="goal-field">
                  <span>what do you want to learn</span>
                  <textarea
                    rows="2"
                    bind:value={planGoal}
                    placeholder="e.g. conversational travel German, or micro-frontend boundaries"
                  ></textarea>
                </label>
                <div class="plan-numbers">
                  <label>
                    <span>target minutes / week</span>
                    <input type="number" min="0" max="2100" step="15" bind:value={planTargetMinutes} />
                  </label>
                </div>
                <fieldset class="windows-field">
                  <legend>when are you available</legend>
                  {#each planWindows as window, index (index)}
                    <div class="window-row">
                      <div class="day-row small">
                        {#each DAYS as day}
                          <button
                            type="button"
                            class:active={window.weekdays.includes(day.id)}
                            aria-pressed={window.weekdays.includes(day.id)}
                            aria-label={day.label}
                            onclick={() => toggleWindowDay(index, day.id)}
                          >{day.short}</button>
                        {/each}
                      </div>
                      <div class="window-times">
                        <TimePicker bind:value={window.start} cron={false} compact label="Availability start time" />
                        <span>to</span>
                        <TimePicker bind:value={window.end} cron={false} compact label="Availability end time" />
                      </div>
                      <button
                        class="window-remove"
                        type="button"
                        aria-label="remove this availability window"
                        disabled={planWindows.length === 1}
                        onclick={() => removeWindow(index)}
                      >
                        <Trash2 size={12} />
                      </button>
                    </div>
                  {/each}
                  <button class="text-action" type="button" onclick={addWindow}>
                    <Plus size={12} /> availability window
                  </button>
                </fieldset>
                {#if planError}<p class="field-error">{planError}</p>{/if}
                <div class="plan-actions">
                  <button
                    class="ghost-action"
                    type="button"
                    disabled={planPreviewing}
                    onclick={previewPlan}
                  >
                    {planPreviewing ? 'previewing…' : 'preview timetable'}
                  </button>
                  {#if planPreview}
                    <button class="save-button" type="button" disabled={planSaving} onclick={savePlan}>
                      {planSaving ? 'saving…' : 'save schedule'}
                    </button>
                  {/if}
                </div>
                {#if planPreview}
                  <div class="plan-preview">
                    <p class:short={!planPreview.meets_target} class="preview-summary mono">
                      ~{planPreview.total_weekly_minutes} min/week from {planPreview.slots.length}
                      session{planPreview.slots.length === 1 ? '' : 's'}
                      {#if planPreview.target_weekly_minutes > 0}
                        —
                        {planPreview.meets_target
                          ? 'meets your target'
                          : `short of your ${planPreview.target_weekly_minutes} min/week target`}
                      {/if}
                    </p>
                    <ul class="preview-slots">
                      {#each planPreview.slots as slot}
                        <li class="mono">
                          {String(slot.hour).padStart(2, '0')}:{String(slot.minute).padStart(2, '0')}
                          · {daySummary(slot.weekdays)}
                        </li>
                      {/each}
                    </ul>
                  </div>
                {/if}
              </div>
            {/if}
          </article>
        {:else}
          <p class="empty-classes">{mode === 'schedule' ? 'Enable a class from Classes to add its study times.' : 'No classes match this filter. Browse All courses to explore the catalog.'}</p>
        {/each}
      </div>
      {#if curriculumMap}
        <CurriculumMap map={curriculumMap} onclose={() => (curriculumMap = null)} />
      {/if}
    </div>
  {/if}
</section>

<style>
  .catalog-toolbar { display: flex; align-items: center; justify-content: space-between; gap: 12px; flex-wrap: wrap; margin-bottom: 18px; }
  .catalog-search { display: flex; align-items: center; gap: 8px; color: var(--faint); border: 1px solid var(--node-border); border-radius: 5px; padding: 6px 9px; background: var(--bg); }
  .catalog-search:focus-within { border-color: var(--accent); }
  .catalog-search input { min-width: 0; width: 150px; border: 0; background: transparent; color: var(--fg); font: 11px var(--font-mono); outline: none; }
  .clear-search { display: grid; place-items: center; border: 0; padding: 2px; background: none; color: var(--muted); cursor: pointer; }
  .class-tools { width: 100%; display: flex; flex-wrap: wrap; gap: 6px; border-top: 1px dashed var(--node-divider); padding-top: 10px; margin-top: 4px; }
  .class-filters { display: flex; flex-wrap: wrap; gap: 8px; margin: 0; }
  .class-filters button { border: 1px solid var(--border); background: var(--bg); color: var(--muted); border-radius: 5px; padding: 7px 10px; font: 11px var(--font-mono); cursor: pointer; }
  .class-filters button.chosen { color: var(--violet-fg); border-color: var(--violet); background: var(--surface-2); }
  .empty-classes { color: var(--muted); font-size: 13px; grid-column: 1 / -1; }
  .classroom-panel {
    width: min(1120px, 100%);
    margin: 18px auto 24px;
    flex: 0 0 auto;
    border: 1px solid var(--node-border);
    border-radius: 10px;
    background: var(--node-bg);
    overflow: hidden;
  }
  .panel-trigger {
    width: 100%;
    min-height: 54px;
    border: 0;
    padding: 10px 14px;
    background: transparent;
    color: var(--text);
    display: flex;
    align-items: center;
    justify-content: space-between;
    cursor: pointer;
    text-align: left;
  }
  .panel-trigger:hover { background: var(--surface-2); }
  .trigger-copy, .trigger-meta, .class-head, .class-actions, .profile-line,
  .progress-copy, .settings-title {
    display: flex;
    align-items: center;
  }
  .trigger-copy { gap: 10px; }
  .trigger-copy strong { display: block; font-size: 13px; }
  .trigger-copy small { display: block; color: var(--faint); font-size: 12px; margin-top: 2px; }
  .trigger-icon {
    width: 30px;
    height: 30px;
    border: 1px solid var(--node-border);
    border-radius: 7px;
    display: grid;
    place-items: center;
    color: var(--accent);
  }
  .trigger-meta { gap: 8px; color: var(--muted); font-size: 12px; }
  .due-count { color: var(--amber); }
  .classroom-body {
    border-top: 1px solid var(--node-border);
    padding: 16px;
  }
  .classroom-intro {
    display: flex;
    justify-content: space-between;
    gap: 24px;
    padding: 4px 2px 14px;
  }
  .classroom-intro p { margin: 5px 0 0; color: var(--muted); font-size: 12px; line-height: 1.55; max-width: 740px; }
  .eyebrow { color: var(--accent); font-size: 11px; letter-spacing: 1.5px; }
  .primary-wins {
    margin: 0 2px 12px;
    border-left: 3px solid var(--amber);
    background: color-mix(in srgb, var(--amber) 9%, transparent);
    color: var(--muted);
    padding: 9px 11px;
    font-size: 11px;
    line-height: 1.45;
  }
  .class-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 16px; align-items: start; }
  .class-card {
    position: relative;
    border: 1px solid var(--node-border);
    border-radius: 9px;
    padding: 13px;
    background: var(--bg);
    min-width: 0;
  }
  .course-summary { font-size: 13px; line-height: 1.55; color: var(--fg); margin: 0; }
  .course-about { font-size: 13px; line-height: 1.55; color: var(--muted); }
  .course-about summary { cursor: pointer; color: var(--fg); }
  .course-about p { margin: 10px 0 0; }
  .course-about summary:focus-visible { outline: 2px solid var(--accent); outline-offset: 4px; }
  .class-card.enabled { border-color: color-mix(in srgb, var(--accent) 30%, var(--node-border)); }
  .class-card.due { box-shadow: inset 3px 0 0 var(--amber); }
  .class-head { gap: 9px; padding: 9px 12px; margin: -13px -13px 13px; border-bottom: 1px solid var(--node-divider); background: var(--node-bg); border-radius: 8px 8px 0 0; }
  .class-identity { flex: 1; min-width: 0; }
  .class-identity strong, .class-identity small { display: block; }
  .class-identity strong { font: 12px/1.5 var(--font-mono); color: var(--fg); }
  .class-identity small { color: var(--faint); font: 9px var(--font-mono); margin-top: 2px; }
  .class-state { color: var(--muted); font-size: 9px; letter-spacing: 0.4px; border-radius: 3px; background: var(--surface-2); padding: 2px 6px; }
  .class-state.online { color: var(--ok-fg); background: var(--ok-bg); }
  .class-state.complete { color: var(--violet-fg); background: var(--violet-bg); }
  .progress-copy { justify-content: space-between; color: var(--muted); font-size: 11px; margin: 13px 0 5px; }
  .progress-track { height: 3px; background: var(--surface-2); overflow: hidden; }
  .progress-track span { display: block; height: 100%; background: var(--accent); }
  .profile-line { justify-content: space-between; gap: 8px; margin-top: 10px; color: var(--faint); font-size: 11px; overflow-wrap: anywhere; }
  .program-detail { min-height: 30px; margin: 8px 0; color: var(--muted); font-size: 12px; line-height: 1.45; }
  .disabled-hint {
    display: flex;
    align-items: center;
    gap: 5px;
    margin: 0 0 8px;
    color: var(--amber);
    font-size: 11px;
    line-height: 1.4;
  }
  .resume-button, .start-button, .save-button {
    border: 1px solid var(--accent);
    background: var(--accent);
    color: var(--bg);
    border-radius: 6px;
    min-height: 32px;
    padding: 6px 10px;
    cursor: pointer;
    font: 11px var(--font-mono);
    text-transform: uppercase;
    letter-spacing: 0.4px;
  }
  .resume-button { width: 100%; margin: 3px 0 8px; text-align: left; }
  .slot-list { list-style: none; padding: 0; margin: 8px 0; display: grid; gap: 4px; }
  .slot-list li { display: flex; border: 1px solid var(--node-border); border-radius: 5px; min-height: 30px; }
  .slot-list li.owed { border-color: var(--amber); }
  .slot-main {
    flex: 1;
    min-width: 0;
    border: 0;
    background: transparent;
    color: var(--muted);
    display: flex;
    align-items: center;
    gap: 7px;
    text-align: left;
    cursor: pointer;
    padding: 5px 7px;
  }
  .slot-main small { color: var(--faint); }
  .slot-main em { margin-left: auto; color: var(--amber); font-style: normal; font-size: 11px; }
  .slot-delete {
    width: 30px;
    border: 0;
    border-left: 1px solid var(--node-border);
    background: transparent;
    color: var(--faint);
    cursor: pointer;
  }
  .class-actions { gap: 8px; flex-wrap: wrap; }
  .class-actions button { font-family: var(--font-mono); }
  .text-action, .power-button {
    min-height: 30px;
    border: 1px solid var(--node-border);
    border-radius: 5px;
    background: transparent;
    color: var(--muted);
    padding: 5px 8px;
    cursor: pointer;
    font-size: 11px;
  }
  .text-action { display: flex; align-items: center; gap: 4px; }
  .start-button { margin-right: auto; }
  .power-button { font-size: 11px; }
  button:disabled { opacity: 0.4; cursor: not-allowed; }
  button:focus-visible, input:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .settings-pane, .slot-editor, .plan-pane {
    position: relative;
    margin: 12px -13px -13px;
    padding: 13px;
    border-top: 1px solid var(--node-border);
    background: var(--surface-2);
  }
  .settings-title { justify-content: space-between; color: var(--accent); font-size: 11px; }
  .settings-title button { border: 0; background: none; color: var(--muted); cursor: pointer; min-height: 28px; }
  .settings-pane > p, .plan-pane > p { color: var(--muted); font-size: 12px; line-height: 1.5; }
  .number-field { display: grid; gap: 5px; color: var(--muted); font-size: 11px; }
  .number-field { margin: 12px 0; max-width: 150px; }
  .number-field input, .language-fields input {
    border: 1px solid var(--node-border);
    border-radius: 5px;
    background: var(--bg);
    color: var(--text);
    min-height: 32px;
    padding: 5px 7px;
  }
  .language-fields label { display: grid; gap: 5px; color: var(--muted); font: 11px var(--font-mono); }
  .language-fields { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 8px; margin: 12px 0; }
  .save-button { margin-top: 10px; }
  .slot-editor fieldset, .windows-field { border: 0; padding: 0; margin: 10px 0; }
  .slot-editor legend, .windows-field legend { color: var(--muted); font-size: 11px; margin-bottom: 6px; }
  .day-row { display: flex; gap: 5px; }
  .day-row.small button { min-width: 26px; min-height: 26px; font-size: 11px; }
  .day-row button {
    min-width: 32px;
    min-height: 32px;
    border: 1px solid var(--node-border);
    border-radius: 5px;
    background: var(--bg);
    color: var(--muted);
    cursor: pointer;
  }
  .day-row button.active { border-color: var(--accent); color: var(--accent); background: var(--surface); }
  .field-error { color: var(--red); font-size: 11px; }
  .goal-field {
    display: grid;
    gap: 5px;
    color: var(--muted);
    font-size: 11px;
    margin: 12px 0;
  }
  .goal-field textarea {
    border: 1px solid var(--node-border);
    border-radius: 5px;
    background: var(--bg);
    color: var(--text);
    padding: 7px;
    font-family: var(--font-body, inherit);
    font-size: 11px;
    resize: vertical;
  }
  .plan-numbers {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 8px;
    margin: 12px 0;
  }
  .plan-numbers label { display: grid; gap: 5px; color: var(--muted); font-size: 11px; }
  .plan-numbers input {
    border: 1px solid var(--node-border);
    border-radius: 5px;
    background: var(--bg);
    color: var(--text);
    min-height: 32px;
    padding: 5px 7px;
  }
  .window-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 6px;
    flex-wrap: wrap;
  }
  .window-times { display: flex; align-items: center; gap: 6px; color: var(--faint); font-size: 11px; }
  .window-remove {
    min-width: 28px;
    min-height: 28px;
    border: 1px solid var(--node-border);
    border-radius: 5px;
    background: transparent;
    color: var(--faint);
    cursor: pointer;
    margin-left: auto;
  }
  .plan-actions { display: flex; gap: 8px; align-items: center; margin-top: 8px; flex-wrap: wrap; }
  .ghost-action {
    min-height: 32px;
    border: 1px solid var(--node-border);
    border-radius: 6px;
    background: transparent;
    color: var(--muted);
    padding: 6px 10px;
    cursor: pointer;
    font-size: 12px;
  }
  .plan-preview {
    margin-top: 12px;
    padding-top: 10px;
    border-top: 1px dashed var(--node-border);
  }
  .preview-summary { color: var(--led-ok, var(--accent)); font-size: 11px; line-height: 1.5; }
  .preview-summary.short { color: var(--amber); }
  .preview-slots { list-style: none; padding: 0; margin: 8px 0 0; display: grid; gap: 4px; }
  .preview-slots li {
    color: var(--muted);
    font-size: 11px;
    border: 1px solid var(--node-border);
    border-radius: 5px;
    padding: 5px 8px;
  }
  @media (max-width: 760px) {
    .class-grid { grid-template-columns: 1fr; }
    .classroom-intro { display: block; }
    .language-fields label { display: grid; gap: 5px; color: var(--muted); font: 11px var(--font-mono); }
  .language-fields { grid-template-columns: 1fr; }
    .plan-numbers { grid-template-columns: 1fr; }
  }
  @media (prefers-reduced-motion: reduce) {
    * { scroll-behavior: auto !important; }
  }
</style>
