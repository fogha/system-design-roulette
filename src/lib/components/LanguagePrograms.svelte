<script lang="ts">
  import {
    api,
    type CefrLevel,
    type LanguageId,
    type LanguageProgramView,
    type LanguageSlotView,
  } from '../ipc';
  import { app } from '../stores.svelte';
  import {
    BookOpen,
    CalendarDays,
    ChevronDown,
    ChevronUp,
    Clock3,
    Headphones,
    MessageCircle,
    Plus,
    Settings2,
    Trash2,
  } from 'lucide-svelte';

  const LEVELS: CefrLevel[] = ['A1', 'A2', 'B1', 'B2'];
  const DAYS = [
    { id: 1, short: 'M', label: 'Monday' },
    { id: 2, short: 'T', label: 'Tuesday' },
    { id: 3, short: 'W', label: 'Wednesday' },
    { id: 4, short: 'T', label: 'Thursday' },
    { id: 5, short: 'F', label: 'Friday' },
    { id: 6, short: 'S', label: 'Saturday' },
    { id: 7, short: 'S', label: 'Sunday' },
  ];

  const programs = $derived(app.state?.language_programs ?? []);
  const slots = $derived(app.state?.language_slots ?? []);
  const active = $derived(app.state?.active_language_session ?? null);

  let expanded = $state(false);
  let editing = $state<LanguageId | null>(null);
  let startLevel = $state<CefrLevel>('A1');
  let targetLevel = $state<CefrLevel>('A2');
  let weeklyMinutes = $state(210);
  let sessionMinutes = $state(30);
  let slotTime = $state('07:30');
  let weekdays = $state<number[]>([1, 2, 3, 4, 5, 6]);
  let saving = $state(false);
  let addingSlot = $state(false);
  let now = $state(Date.now());

  $effect(() => {
    const id = setInterval(() => (now = Date.now()), 30_000);
    return () => clearInterval(id);
  });

  function slotsFor(language: LanguageId) {
    return slots.filter((slot) => slot.language === language);
  }

  function openSettings(program: LanguageProgramView) {
    editing = editing === program.language ? null : program.language;
    startLevel = program.start_level;
    targetLevel = program.target_level;
    weeklyMinutes = program.weekly_minutes;
    sessionMinutes = program.session_minutes;
  }

  function toggleDay(day: number) {
    weekdays = weekdays.includes(day)
      ? weekdays.filter((candidate) => candidate !== day)
      : [...weekdays, day].sort();
  }

  async function saveProgram(program: LanguageProgramView, enabled = true) {
    if (saving) return;
    saving = true;
    try {
      await api.configureLanguageProgram({
        language: program.language,
        enabled,
        start_level: startLevel,
        target_level: targetLevel,
        weekly_minutes: weeklyMinutes,
        session_minutes: sessionMinutes,
      });
      editing = enabled ? editing : null;
      await app.refresh();
    } catch (error) {
      app.error = String(error);
    } finally {
      saving = false;
    }
  }

  async function activate(program: LanguageProgramView) {
    editing = program.language;
    startLevel = 'A1';
    targetLevel = 'A2';
    weeklyMinutes = 210;
    sessionMinutes = 30;
    await saveProgram(program, true);
  }

  async function addSlot(program: LanguageProgramView) {
    if (addingSlot || weekdays.length === 0) return;
    const [hour, minute] = slotTime.split(':').map(Number);
    addingSlot = true;
    try {
      await api.upsertLanguageSlot({
        language: program.language,
        hour,
        minute,
        weekdays,
        enabled: true,
      });
      await app.refresh();
    } catch (error) {
      app.error = String(error);
    } finally {
      addingSlot = false;
    }
  }

  async function removeSlot(id: number) {
    try {
      await api.deleteLanguageSlot(id);
      await app.refresh();
    } catch (error) {
      app.error = String(error);
    }
  }

  function formatClock(hour: number, minute: number) {
    return `${String(hour).padStart(2, '0')}:${String(minute).padStart(2, '0')}`;
  }

  function countdown(slot: LanguageSlotView) {
    if (slot.owed) return 'due now';
    const target = new Date(slot.next_fire_at).getTime();
    const difference = Math.max(0, target - now);
    const hours = Math.floor(difference / 3_600_000);
    const minutes = Math.floor((difference % 3_600_000) / 60_000);
    if (hours >= 24) return `in ${Math.floor(hours / 24)}d ${hours % 24}h`;
    return `in ${hours}h ${minutes}m`;
  }

  function shortDate(value: string) {
    return new Intl.DateTimeFormat(undefined, {
      month: 'short',
      day: 'numeric',
      year: 'numeric',
    }).format(new Date(`${value}T12:00:00`));
  }
</script>

<section class="language-panel" aria-labelledby="language-programs-title">
  <button
    class="panel-trigger"
    type="button"
    aria-expanded={expanded}
    aria-controls="language-programs-body"
    onclick={() => (expanded = !expanded)}
  >
    <span class="trigger-copy">
      <span class="trigger-icon"><MessageCircle size={15} /></span>
      <span>
        <strong id="language-programs-title">Language programs</strong>
        <small>German and Italian · independent schedules · never locks the app</small>
      </span>
    </span>
    <span class="trigger-meta mono">
      {#if app.state?.language_due_count}
        <span class="due-count">{app.state.language_due_count} due</span>
      {:else}
        {programs.filter((program) => program.enabled).length} active
      {/if}
      {#if expanded}<ChevronUp size={15} />{:else}<ChevronDown size={15} />{/if}
    </span>
  </button>

  {#if expanded}
    <div id="language-programs-body" class="panel-body">
      <div class="principle-note">
        <BookOpen size={15} />
        <p>
          A1 in one month and A2 by month three are <strong>stretch planning targets</strong>.
          Levels advance only after skill evidence across listening, reading, interaction,
          production, writing, grammar, and vocabulary—not when a date passes.
        </p>
      </div>

      {#if active}
        <div class="active-session" role="status">
          <span>
            <span class="meta-label">PRACTICE PAUSED</span>
            <strong>{active.label} {active.level} · {active.title}</strong>
          </span>
          <button class="compact-primary" type="button" onclick={() => app.resumeLanguage()}>
            resume
          </button>
        </div>
      {/if}

      <div class="program-grid">
        {#each programs as program (program.language)}
          <article class:enabled={program.enabled} class="program-card">
            <header class="program-head">
              <div>
                <span class="language-code mono">{program.language === 'german' ? 'DE' : 'IT'}</span>
                <h3>{program.label} <em>{program.native_label}</em></h3>
              </div>
              {#if program.enabled}
                <span class="level-chip mono">{program.current_level} → {program.target_level}</span>
              {:else}
                <span class="inactive-chip mono">not configured</span>
              {/if}
            </header>

            {#if program.enabled}
              <div class="progress-copy">
                <span>{program.completed_steps} / {program.required_steps} evidence steps in {program.current_level}</span>
                <strong>{Math.round(program.progress * 100)}%</strong>
              </div>
              <div
                class="progress-line"
                role="progressbar"
                aria-label={`${program.label} ${program.current_level} progress`}
                aria-valuemin="0"
                aria-valuemax="100"
                aria-valuenow={Math.round(program.progress * 100)}
              >
                <span style={`width:${Math.round(program.progress * 100)}%`}></span>
              </div>

              <div class="milestones" aria-label={`${program.label} CEFR target dates`}>
                {#each program.milestones as milestone}
                  <div class:reached={milestone.reached}>
                    <span class="mono">{milestone.level}</span>
                    <small>{shortDate(milestone.target_date)}</small>
                  </div>
                {/each}
              </div>

              <p class:warning={program.pace_status !== 'on_track'} class="pace-note">
                {program.pace_message}
              </p>

              <div class="skill-grid" aria-label={`${program.label} skill evidence`}>
                {#each program.skills as skill}
                  <div class="skill">
                    <span>{skill.label}</span>
                    <span class="mono">{skill.encounters === 0 ? '—' : `${Math.round(skill.score * 100)}%`}</span>
                    <div class="skill-track" aria-hidden="true">
                      <span style={`width:${Math.round(skill.score * 100)}%`}></span>
                    </div>
                  </div>
                {/each}
              </div>

              <div class="slot-list" aria-label={`${program.label} practice slots`}>
                {#each slotsFor(program.language) as slot (slot.id)}
                  <div class:due={slot.owed} class="slot-row">
                    <span class="slot-time mono">
                      <Clock3 size={13} />
                      {formatClock(slot.hour, slot.minute)}
                    </span>
                    <span class="slot-days mono">
                      {slot.weekdays.map((day) => DAYS[day - 1]?.short).join('')}
                    </span>
                    <span class="slot-countdown">{countdown(slot)}</span>
                    <button
                      class="slot-start"
                      type="button"
                      disabled={!!active}
                      onclick={() => app.startLanguage(program.language, slot.id)}
                    >
                      {slot.owed ? 'begin due practice' : 'start early'}
                    </button>
                    <button
                      class="icon-button"
                      type="button"
                      aria-label={`Delete ${program.label} slot at ${formatClock(slot.hour, slot.minute)}`}
                      onclick={() => removeSlot(slot.id)}
                    >
                      <Trash2 size={13} />
                    </button>
                  </div>
                {/each}
              </div>

              {#if slotsFor(program.language).length === 0}
                <div class="empty-slot">
                  <CalendarDays size={16} />
                  <span>No reminder yet. Practice can still start voluntarily.</span>
                </div>
              {/if}

              <div class="program-actions">
                <button
                  class="compact-primary"
                  type="button"
                  disabled={!!active}
                  onclick={() => app.startLanguage(program.language)}
                >
                  <Headphones size={13} /> start now
                </button>
                <button class="compact-ghost" type="button" onclick={() => openSettings(program)}>
                  <Settings2 size={13} /> settings & slots
                </button>
              </div>
            {:else}
              <p class="inactive-copy">
                A CEFR-aligned path with real-life scenarios, pronunciation, pragmatics,
                listening, speaking, writing, and delayed retrieval.
              </p>
              <button class="compact-primary" type="button" onclick={() => activate(program)}>
                configure {program.label}
              </button>
            {/if}

            {#if editing === program.language}
              <div class="config" aria-label={`${program.label} program settings`}>
                <div class="config-grid">
                  <label>
                    <span>Starting level</span>
                    <select bind:value={startLevel} disabled={program.total_completed_steps > 0}>
                      {#each LEVELS as level}<option value={level}>{level}</option>{/each}
                    </select>
                  </label>
                  <label>
                    <span>Current target</span>
                    <select bind:value={targetLevel}>
                      {#each LEVELS as level}
                        {#if LEVELS.indexOf(level) >= LEVELS.indexOf(startLevel)}
                          <option value={level}>{level}</option>
                        {/if}
                      {/each}
                    </select>
                  </label>
                  <label>
                    <span>Weekly total practice</span>
                    <input type="number" min="60" max="2100" step="15" bind:value={weeklyMinutes} />
                  </label>
                  <label>
                    <span>Session minutes</span>
                    <input type="number" min="15" max="90" step="5" bind:value={sessionMinutes} />
                  </label>
                </div>
                <div class="config-actions">
                  <button
                    class="compact-primary"
                    type="button"
                    disabled={saving}
                    onclick={() => saveProgram(program, true)}
                  >
                    {saving ? 'saving…' : 'save learning plan'}
                  </button>
                  {#if program.enabled}
                    <button
                      class="danger-link"
                      type="button"
                      disabled={saving || !!active}
                      onclick={() => saveProgram(program, false)}
                    >
                      disable program
                    </button>
                  {/if}
                </div>

                {#if program.enabled}
                  <div class="new-slot">
                    <div>
                      <span class="meta-label">ADD NON-BLOCKING SLOT</span>
                      <div class="slot-editor">
                        <input aria-label="Practice time" type="time" bind:value={slotTime} />
                        <div class="day-picker" aria-label="Practice weekdays">
                          {#each DAYS as day}
                            <button
                              type="button"
                              class:selected={weekdays.includes(day.id)}
                              aria-pressed={weekdays.includes(day.id)}
                              aria-label={day.label}
                              onclick={() => toggleDay(day.id)}
                            >
                              {day.short}
                            </button>
                          {/each}
                        </div>
                        <button
                          class="compact-ghost"
                          type="button"
                          disabled={addingSlot || weekdays.length === 0}
                          onclick={() => addSlot(program)}
                        >
                          <Plus size={13} /> {addingSlot ? 'adding…' : 'add slot'}
                        </button>
                      </div>
                    </div>
                  </div>
                {/if}
              </div>
            {/if}
          </article>
        {/each}
      </div>
    </div>
  {/if}
</section>

<style>
  .language-panel {
    width: min(980px, 94vw);
    margin: 18px auto 0;
    border: 1px solid var(--node-border);
    border-radius: 10px;
    background: color-mix(in srgb, var(--node-bg) 96%, transparent);
    overflow: hidden;
  }
  .panel-trigger {
    width: 100%;
    min-height: 64px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 20px;
    padding: 12px 16px;
    border: 0;
    background: transparent;
    color: var(--fg);
    cursor: pointer;
    text-align: left;
  }
  .panel-trigger:hover {
    background: var(--surface);
  }
  .panel-trigger:focus-visible,
  button:focus-visible,
  select:focus-visible,
  input:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  .trigger-copy,
  .trigger-meta,
  .program-head,
  .program-head > div,
  .program-actions,
  .active-session,
  .slot-time,
  .slot-editor,
  .config-actions {
    display: flex;
    align-items: center;
  }
  .trigger-copy {
    gap: 12px;
  }
  .trigger-copy strong {
    display: block;
    font-family: var(--font-display);
    font-size: 17px;
    font-weight: 500;
  }
  .trigger-copy small {
    display: block;
    color: var(--muted);
    font-size: 11px;
  }
  .trigger-icon {
    width: 34px;
    height: 34px;
    display: grid;
    place-items: center;
    color: var(--accent);
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--surface);
  }
  .trigger-meta {
    flex: none;
    gap: 8px;
    color: var(--faint);
    font-size: 10px;
    text-transform: uppercase;
  }
  .due-count {
    color: var(--warn-fg);
  }
  .panel-body {
    border-top: 1px solid var(--node-divider);
    padding: 16px;
  }
  .principle-note {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 10px;
    padding: 12px 14px;
    color: var(--warn-fg);
    background: var(--warn-bg);
    border-radius: 8px;
    font-size: 12px;
  }
  .principle-note p {
    margin: 0;
  }
  .active-session {
    justify-content: space-between;
    gap: 16px;
    margin-top: 12px;
    padding: 12px 14px;
    border: 1px solid var(--led-warn);
    border-radius: 8px;
    background: var(--node-bg);
  }
  .active-session strong {
    display: block;
    margin-top: 2px;
    font-size: 13px;
  }
  .program-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 12px;
    margin-top: 12px;
  }
  .program-card {
    min-width: 0;
    padding: 16px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--bg);
  }
  .program-card.enabled {
    border-color: color-mix(in srgb, var(--led-ok) 38%, var(--border));
  }
  .program-head {
    justify-content: space-between;
    gap: 12px;
  }
  .program-head > div {
    min-width: 0;
    gap: 10px;
  }
  .program-head h3 {
    font-size: 18px;
  }
  .program-head h3 em {
    color: var(--muted);
    font-family: var(--font-body);
    font-size: 11px;
    font-style: normal;
    font-weight: 400;
  }
  .language-code {
    display: grid;
    width: 32px;
    height: 32px;
    place-items: center;
    color: var(--accent);
    background: var(--surface);
    border-radius: 7px;
    font-size: 11px;
  }
  .level-chip,
  .inactive-chip {
    flex: none;
    padding: 4px 7px;
    border-radius: 5px;
    font-size: 9px;
  }
  .level-chip {
    color: var(--ok-fg);
    background: var(--ok-bg);
  }
  .inactive-chip {
    color: var(--faint);
    border: 1px solid var(--border);
  }
  .progress-copy {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    margin-top: 16px;
    color: var(--muted);
    font-size: 10px;
  }
  .progress-copy strong {
    color: var(--fg);
  }
  .progress-line,
  .skill-track {
    overflow: hidden;
    background: var(--surface);
  }
  .progress-line {
    height: 5px;
    margin-top: 6px;
    border-radius: 999px;
  }
  .progress-line span,
  .skill-track span {
    display: block;
    height: 100%;
    background: var(--led-ok);
  }
  .milestones {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 4px;
    margin-top: 12px;
  }
  .milestones div {
    padding: 7px 6px;
    color: var(--faint);
    border: 1px solid var(--border);
    border-radius: 6px;
  }
  .milestones div.reached {
    color: var(--ok-fg);
    border-color: var(--ok-bg);
  }
  .milestones span,
  .milestones small {
    display: block;
  }
  .milestones span {
    font-size: 10px;
  }
  .milestones small {
    margin-top: 2px;
    font-size: 8px;
  }
  .pace-note {
    min-height: 42px;
    margin: 10px 0;
    color: var(--muted);
    font-size: 10px;
    line-height: 1.45;
  }
  .pace-note.warning {
    color: var(--warn-fg);
  }
  .skill-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 7px 12px;
    margin: 12px 0;
  }
  .skill {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 3px 8px;
    color: var(--muted);
    font-size: 9px;
  }
  .skill > span:first-child {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .skill-track {
    grid-column: 1 / -1;
    height: 2px;
  }
  .slot-list {
    display: grid;
    gap: 6px;
  }
  .slot-row {
    display: grid;
    grid-template-columns: auto auto 1fr auto auto;
    align-items: center;
    gap: 8px;
    min-height: 40px;
    padding: 6px 7px;
    border: 1px solid var(--border);
    border-radius: 7px;
    background: var(--node-bg);
  }
  .slot-row.due {
    border-color: var(--led-warn);
    background: var(--warn-bg);
  }
  .slot-time {
    gap: 5px;
    color: var(--fg);
    font-size: 10px;
  }
  .slot-days {
    color: var(--faint);
    font-size: 8px;
    letter-spacing: 1px;
  }
  .slot-countdown {
    color: var(--muted);
    font-size: 9px;
  }
  .slot-start,
  .icon-button {
    min-height: 28px;
    border: 1px solid var(--border);
    background: transparent;
    color: var(--muted);
    border-radius: 5px;
    cursor: pointer;
  }
  .slot-start {
    padding: 4px 8px;
    font-family: var(--font-mono);
    font-size: 8px;
    text-transform: uppercase;
  }
  .icon-button {
    width: 28px;
    display: grid;
    place-items: center;
  }
  .slot-start:hover,
  .icon-button:hover {
    color: var(--fg);
    border-color: var(--muted);
  }
  button:disabled {
    cursor: not-allowed;
    opacity: 0.4;
  }
  .empty-slot {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px;
    color: var(--faint);
    border: 1px dashed var(--border);
    border-radius: 7px;
    font-size: 10px;
  }
  .program-actions {
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 12px;
  }
  .compact-primary,
  .compact-ghost {
    min-height: 32px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 6px 10px;
    border-radius: 6px;
    font-family: var(--font-mono);
    font-size: 9px;
    text-transform: uppercase;
    cursor: pointer;
  }
  .compact-primary {
    color: var(--accent-fg);
    background: var(--accent);
    border: 1px solid var(--accent);
  }
  .compact-ghost {
    color: var(--muted);
    background: transparent;
    border: 1px solid var(--border);
  }
  .compact-ghost:hover {
    color: var(--fg);
    border-color: var(--muted);
  }
  .inactive-copy {
    min-height: 66px;
    color: var(--muted);
    font-size: 11px;
  }
  .config {
    margin-top: 14px;
    padding-top: 14px;
    border-top: 1px dashed var(--border);
  }
  .config-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px;
  }
  .config label > span {
    display: block;
    margin-bottom: 4px;
    color: var(--faint);
    font-family: var(--font-mono);
    font-size: 8px;
    text-transform: uppercase;
  }
  select,
  input[type='number'],
  input[type='time'] {
    width: 100%;
    min-height: 36px;
    padding: 7px 9px;
    color: var(--fg);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 6px;
    font: 11px var(--font-mono);
  }
  .config-actions {
    justify-content: space-between;
    gap: 8px;
    margin-top: 10px;
  }
  .danger-link {
    min-height: 32px;
    padding: 4px;
    color: var(--bad-fg);
    background: transparent;
    border: 0;
    font-size: 10px;
    cursor: pointer;
  }
  .new-slot {
    margin-top: 14px;
    padding-top: 12px;
    border-top: 1px solid var(--border);
  }
  .slot-editor {
    align-items: stretch;
    gap: 8px;
    margin-top: 6px;
  }
  .slot-editor input {
    width: 96px;
  }
  .day-picker {
    display: flex;
    gap: 3px;
  }
  .day-picker button {
    width: 28px;
    min-height: 36px;
    color: var(--faint);
    background: transparent;
    border: 1px solid var(--border);
    border-radius: 5px;
    cursor: pointer;
  }
  .day-picker button.selected {
    color: var(--ok-fg);
    background: var(--ok-bg);
    border-color: var(--led-ok);
  }
  @media (max-width: 760px) {
    .program-grid {
      grid-template-columns: 1fr;
    }
    .slot-row {
      grid-template-columns: auto auto 1fr auto;
    }
    .slot-start {
      grid-column: 1 / -1;
    }
    .slot-editor {
      flex-wrap: wrap;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    * {
      animation: none !important;
      transition: none !important;
    }
  }
</style>
