<script lang="ts">
  import type { StudyPulse } from '$lib/ipc';
  import AnimatedNumber from './AnimatedNumber.svelte';
  import { Flame, CalendarCheck, BookOpenCheck, Target, Trophy } from 'lucide-svelte';

  let { pulse }: { pulse: StudyPulse } = $props();

  /** Weeks as columns, seven rows each, Monday at the top. */
  const weeks = $derived.by(() => {
    const columns: StudyPulse['days'][] = [];
    for (let i = 0; i < pulse.days.length; i += 7) columns.push(pulse.days.slice(i, i + 7));
    return columns;
  });
  const busiest = $derived(Math.max(1, ...pulse.days.map((day) => day.minutes)));
  /** Intensity 0..4, by minutes against the busiest day so a light week still shows shape. */
  function level(minutes: number): number {
    if (minutes <= 0) return 0;
    const share = minutes / busiest;
    return share > 0.75 ? 4 : share > 0.5 ? 3 : share > 0.25 ? 2 : 1;
  }
  /** Month labels over the first column that starts a new month. */
  const months = $derived.by(() => {
    const labels: { column: number; label: string }[] = [];
    let last = '';
    weeks.forEach((week, column) => {
      const month = week[0]?.date.slice(0, 7) ?? '';
      if (month && month !== last) {
        labels.push({ column, label: new Date(`${week[0].date}T12:00:00`).toLocaleDateString(undefined, { month: 'short' }) });
        last = month;
      }
    });
    return labels;
  });
  const streakDates = $derived.by(() => {
    const set = new Set<string>();
    const today = new Date(`${pulse.today}T12:00:00`);
    // The streak may have started yesterday; mark its days back from the latest studied one.
    const days = [...pulse.days].reverse();
    let index = days.findIndex((day) => day.completed > 0);
    if (index < 0 || pulse.streak === 0) return set;
    const latest = new Date(`${days[index].date}T12:00:00`);
    if ((today.getTime() - latest.getTime()) / 86400000 > 1) return set;
    for (let count = 0; count < pulse.streak && index < days.length; index += 1) {
      if (days[index].completed > 0) { set.add(days[index].date); count += 1; } else break;
    }
    return set;
  });
  const weekShare = $derived(pulse.week_target_minutes > 0 ? Math.min(1, pulse.week_minutes / pulse.week_target_minutes) : 0);
  let tip = $state<{ day: StudyPulse['days'][number]; x: number; y: number } | null>(null);
  let grid = $state<HTMLElement | undefined>(undefined);
  function show(day: StudyPulse['days'][number], event: MouseEvent | FocusEvent) {
    const cell = event.currentTarget as HTMLElement;
    const rect = cell.getBoundingClientRect();
    const host = grid?.getBoundingClientRect();
    // Keep the tip inside the card, which clips what overflows it.
    const width = host?.width ?? 0;
    const x = rect.left + rect.width / 2 - (host?.left ?? 0);
    tip = { day, x: width ? Math.min(Math.max(x, 110), width - 110) : x, y: rect.top - (host?.top ?? 0) };
  }
  function dayLabel(date: string) {
    return new Date(`${date}T12:00:00`).toLocaleDateString(undefined, { weekday: 'short', month: 'short', day: 'numeric' });
  }
  function hours(minutes: number) {
    return minutes >= 60 ? `${(minutes / 60).toFixed(minutes % 60 ? 1 : 0)} h` : `${minutes} min`;
  }
</script>

<section class="pulse" aria-label="Study pulse">
  <div class="stats">
    <div class="stat streak" class:alight={pulse.streak > 0}>
      <span class="flame" aria-hidden="true"><Flame size={22} /></span>
      <div><strong><AnimatedNumber value={pulse.streak} /></strong><span class="mono">day streak</span></div>
      <small class="mono">best {pulse.longest_streak}</small>
    </div>
    <div class="stat"><span class="glyph"><BookOpenCheck size={16} /></span><div><strong><AnimatedNumber value={pulse.completed_sessions} /></strong><span class="mono">lessons done</span></div></div>
    <div class="stat"><span class="glyph"><CalendarCheck size={16} /></span><div><strong><AnimatedNumber value={pulse.study_days} /></strong><span class="mono">study days</span></div></div>
    <div class="stat"><span class="glyph"><Trophy size={16} /></span><div><strong>{#if pulse.pass_rate == null}—{:else}<AnimatedNumber value={pulse.pass_rate * 100} suffix="%" />{/if}</strong><span class="mono">checks passed</span></div></div>
    <div class="stat this-week">
      <span class="glyph"><Target size={16} /></span>
      <div class="week-body">
        <strong><AnimatedNumber value={pulse.week_minutes} /><em class="mono"> / {pulse.week_target_minutes || '—'} min</em></strong>
        <span class="mono">this week · {Math.round(weekShare * 100)}% of target</span>
        <div class="week-track" role="progressbar" aria-label="This week's study minutes against the target" aria-valuemin="0" aria-valuemax={pulse.week_target_minutes} aria-valuenow={pulse.week_minutes}><i style:width={`${weekShare * 100}%`}></i></div>
      </div>
    </div>
  </div>

  <div class="map" bind:this={grid}>
    <div class="months mono" aria-hidden="true">
      {#each months as month (month.column)}<span style:grid-column={month.column + 2}>{month.label}</span>{/each}
    </div>
    <div class="grid">
      <div class="weekdays mono" aria-hidden="true"><span>Mon</span><span></span><span>Wed</span><span></span><span>Fri</span><span></span><span></span></div>
      {#each weeks as week, column (week[0]?.date ?? column)}
        <div class="week">
          {#each week as day, row (day.date)}
            <button
              type="button"
              class={`cell level-${level(day.minutes)}`}
              class:today={day.date === pulse.today}
              class:streak={streakDates.has(day.date)}
              style:animation-delay={`${Math.min(900, (column * 7 + row) * 4)}ms`}
              aria-label={`${dayLabel(day.date)}: ${day.completed ? `${day.completed} lesson${day.completed === 1 ? '' : 's'}, ${hours(day.minutes)}` : 'no study'}`}
              onmouseenter={(event) => show(day, event)}
              onfocus={(event) => show(day, event)}
              onmouseleave={() => (tip = null)}
              onblur={() => (tip = null)}
            ></button>
          {/each}
        </div>
      {/each}
    </div>
    {#if tip}
      <div class="tip" style:left={`${tip.x}px`} style:top={`${tip.y}px`} role="tooltip">
        <strong>{dayLabel(tip.day.date)}</strong>
        <span>{tip.day.completed ? `${tip.day.completed} lesson${tip.day.completed === 1 ? '' : 's'} · ${hours(tip.day.minutes)}${tip.day.classes.length ? ` · ${tip.day.classes.join(' ')}` : ''}` : 'No study'}</span>
      </div>
    {/if}
    <div class="legend mono" aria-hidden="true"><span>less</span><i class="cell level-0"></i><i class="cell level-1"></i><i class="cell level-2"></i><i class="cell level-3"></i><i class="cell level-4"></i><span>more</span></div>
  </div>
</section>

<style>
  .pulse { display: flex; flex-direction: column; gap: 18px; }
  .stats { display: grid; grid-template-columns: repeat(6, minmax(0, 1fr)); gap: 10px; }
  @media (max-width: 900px) { .stats { grid-template-columns: repeat(2, minmax(0, 1fr)); } .this-week { grid-column: span 2; } }
  .stat { display: flex; align-items: center; gap: 12px; padding: 12px 14px; border: 1px solid var(--node-border); border-radius: var(--radius-panel); background: var(--node-bg); min-width: 0; animation: rise 500ms cubic-bezier(0.22, 1, 0.36, 1) both; }
  .stat:nth-child(2) { animation-delay: 60ms; } .stat:nth-child(3) { animation-delay: 120ms; } .stat:nth-child(4) { animation-delay: 180ms; } .stat:nth-child(5) { animation-delay: 240ms; }
  .stat strong { display: block; font-size: 22px; line-height: 1.1; color: var(--fg); font-weight: 500; }
  .stat strong em { font-style: normal; font-size: 11px; color: var(--muted); }
  .stat > div > span { display: block; margin-top: 4px; font-size: 9px; letter-spacing: 1px; text-transform: uppercase; color: var(--muted); }
  .stat small { margin-left: auto; font-size: 9px; color: var(--faint); white-space: nowrap; }
  .glyph { display: grid; place-items: center; width: 34px; height: 34px; flex: none; border-radius: var(--radius-control); background: var(--surface-2); color: var(--accent); }
  .streak { border-color: color-mix(in srgb, var(--accent) 35%, var(--node-border)); background: linear-gradient(135deg, color-mix(in srgb, var(--accent) 12%, var(--node-bg)), var(--node-bg) 70%); }
  .flame { display: grid; place-items: center; width: 34px; height: 34px; flex: none; border-radius: var(--radius-control); background: color-mix(in srgb, var(--accent) 18%, var(--surface-2)); color: var(--faint); }
  .streak.alight .flame { color: var(--accent); animation: flicker 1.6s ease-in-out infinite; filter: drop-shadow(0 0 8px color-mix(in srgb, var(--accent) 70%, transparent)); }
  .this-week { grid-column: span 2; }
  .week-body { flex: 1; min-width: 0; }
  .week-track { position: relative; height: 6px; margin-top: 10px; border-radius: 3px; background: var(--surface-2); overflow: hidden; }
  .week-track i { position: absolute; inset: 0 auto 0 0; border-radius: 3px; background: linear-gradient(90deg, var(--accent), var(--ok-fg)); transition: width 900ms cubic-bezier(0.22, 1, 0.36, 1); }

  .map { position: relative; padding: 14px 18px 12px; border: 1px solid var(--node-border); border-radius: var(--radius-panel); background: var(--node-bg); overflow-x: auto; }
  .months { display: grid; grid-template-columns: 30px repeat(26, minmax(12px, 1fr)); gap: 0 4px; margin-bottom: 8px; font-size: 8.5px; color: var(--faint); height: 12px; }
  .months span { white-space: nowrap; }
  .grid { display: grid; grid-template-columns: 30px repeat(26, minmax(12px, 1fr)); gap: 4px; }
  .weekdays { display: grid; grid-template-rows: repeat(7, 1fr); gap: 4px; font-size: 8px; color: var(--faint); align-items: center; }
  .week { display: grid; grid-template-rows: repeat(7, 1fr); gap: 4px; }
  .cell { width: 100%; aspect-ratio: 1; padding: 0; border: 0; border-radius: 4px; background: var(--surface-2); cursor: default; animation: pop 420ms cubic-bezier(0.22, 1, 0.36, 1) both; }
  button.cell:focus-visible { outline: 2px solid var(--accent); outline-offset: 1px; }
  .cell.level-1 { background: color-mix(in srgb, var(--accent) 28%, var(--surface-2)); }
  .cell.level-2 { background: color-mix(in srgb, var(--accent) 50%, var(--surface-2)); }
  .cell.level-3 { background: color-mix(in srgb, var(--accent) 75%, var(--surface-2)); }
  .cell.level-4 { background: var(--accent); box-shadow: 0 0 8px color-mix(in srgb, var(--accent) 55%, transparent); }
  .cell.streak { box-shadow: inset 0 0 0 1.5px color-mix(in srgb, var(--fg) 60%, transparent); }
  .cell.today { outline: 1.5px solid var(--fg); outline-offset: 1px; }
  .tip { position: absolute; transform: translate(-50%, calc(-100% - 8px)); z-index: 4; padding: 7px 10px; border: 1px solid var(--node-border); border-radius: var(--radius-control); background: var(--surface); box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35); white-space: nowrap; pointer-events: none; }
  .tip strong { display: block; font-size: 11px; font-weight: 500; color: var(--fg); }
  .tip span { display: block; margin-top: 2px; font-size: 10px; color: var(--muted); }
  .legend { display: flex; align-items: center; gap: 4px; justify-content: flex-end; margin-top: 10px; font-size: 8.5px; color: var(--faint); }
  .legend .cell { width: 10px; height: 10px; aspect-ratio: auto; animation: none; }
  .legend span { margin: 0 4px; }

  @keyframes rise { from { opacity: 0; transform: translateY(8px); } to { opacity: 1; transform: none; } }
  @keyframes pop { from { opacity: 0; transform: scale(0.4); } to { opacity: 1; transform: scale(1); } }
  @keyframes flicker { 0%, 100% { transform: scale(1) rotate(-2deg); opacity: 1; } 50% { transform: scale(1.12) rotate(3deg); opacity: 0.85; } }
  @media (prefers-reduced-motion: reduce) { .stat, .cell { animation: none; } .streak.alight .flame { animation: none; } .week-track i { transition: none; } }
</style>
