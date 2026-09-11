import { describe, expect, it, vi } from 'vitest';
import { api, type AppStateView } from './ipc';
import { mockEmit } from './mock';
import { app, resolveRoute, shouldShowEscapeHatch } from './stores.svelte';

function state(overrides: Partial<AppStateView> = {}): AppStateView {
  return {
    onboarded: true,
    session: {
      session_id: "primary-fixture",
      date: '2026-07-28',
      status: 'pending',
      step: 'quiz',
      quiz_score: null,
      streak: 3,
      locked: false,
      session_type: 'lesson',
      plan_reason: '',
      focus: 'javascript',
    },
    selected_focus: 'javascript',
    owed: false,
    schedule_hour: 9,
    schedule_minute: 0,
    debug_day: false,
    enforcement_disarmed: false,
    schedule_paused: false,
    kiosk_level: 'hard',
    model: 'opus',
    agent: 'claude',
    custom_agent_bin: '',
    deepseek_key_configured: false,
    classroom_programs: [],
    classroom_slots: [],
    classroom_due_count: 0,
    active_classroom_sessions: [],
    appointments: [],
    focus: null,
    ...overrides,
  };
}

describe('authoritative screen routing', () => {
  it('returns to Today when the desk becomes owed', () => {
    expect(resolveRoute(state({ owed: true }), 'classroom')).toEqual({ screen: 'idle' });
  });

  it('keeps a saved daily-routine session as recovery data without opening a screen', () => {
    const current = state({
      session: { ...state().session, status: 'in_progress', step: 'course', locked: true },
    });
    expect(resolveRoute(current, 'loading')).toEqual({ screen: 'idle' });
    expect(resolveRoute(current, 'classroom')).toEqual({ screen: 'classroom' });
  });

  it('leaves the setup wizard only once onboarding is complete', () => {
    expect(resolveRoute(state({ onboarded: false }), 'idle')).toEqual({ screen: 'setup' });
    expect(resolveRoute(state(), 'setup')).toEqual({ screen: 'idle' });
  });
});

describe('escape hatch visibility', () => {
  it('is visible in the pending-before-start owed window', () => {
    expect(shouldShowEscapeHatch(state({ owed: true }))).toBe(true);
  });

  it('is visible whenever the session is locked or in progress', () => {
    expect(
      shouldShowEscapeHatch(
        state({
          session: { ...state().session, locked: true },
        }),
      ),
    ).toBe(true);
    expect(
      shouldShowEscapeHatch(
        state({
          session: { ...state().session, status: 'in_progress' },
        }),
      ),
    ).toBe(true);
  });

  it('stays hidden after a normal completed session', () => {
    expect(
      shouldShowEscapeHatch(
        state({
          session: { ...state().session, status: 'completed', step: 'done' },
        }),
      ),
    ).toBe(false);
  });
});

it('keeps the latest app state when refreshes resolve out of order', async () => {
  const first = state();
  const second = state({ session: { ...first.session, session_id: 'primary-next' } });
  const getState = vi.spyOn(api, 'getAppState').mockResolvedValue(first);
  vi.spyOn(api, 'markFrontendReady').mockResolvedValue(undefined);
  try {
    await app.init();
    let releaseOld!: (value: AppStateView) => void;
    getState.mockImplementationOnce(() => new Promise((resolve) => { releaseOld = resolve; }));
    const oldRefresh = app.refresh();
    getState.mockResolvedValueOnce(second);
    await app.refresh();
    expect(app.session?.session_id).toBe('primary-next');

    releaseOld(first);
    await oldRefresh;
    expect(app.session?.session_id).toBe('primary-next');
    expect(app.screen).toBe('idle');
  } finally {
    vi.restoreAllMocks();
  }
});

it('keeps preparation visible across navigation and prevents duplicate starts until failure releases it', async () => {
  app.state = state();
  app.screen = 'idle';
  let rejectStart!: (cause: Error) => void;
  const start = vi.spyOn(api, 'startClassroomSession').mockImplementationOnce(() => new Promise((_, reject) => { rejectStart = reject; }));
  try {
    const pending = app.startClass('linux-bash', 12);
    expect(app.preparingClass?.subjectId).toBe('linux-bash');
    app.navigate('classes');
    await app.startClass('german', 13);
    expect(start).toHaveBeenCalledTimes(1);
    expect(app.preparingClass?.subjectId).toBe('linux-bash');
    rejectStart(new Error('Provider unavailable'));
    await pending;
    expect(app.preparingClass).toBeNull();
    expect(app.error).toContain('Provider unavailable');
    start.mockRejectedValueOnce(new Error('Authentication required'));
    await app.startClass('german', 13);
    expect(start).toHaveBeenCalledTimes(2);
    expect(app.preparingClass).toBeNull();
    expect(app.error).toContain('Authentication required');
  } finally {
    vi.restoreAllMocks();
    app.error = '';
  }
});

describe('focused class sessions', () => {
  const focus = { session_id: 'study-1', course_id: 'typescript', policy: 'focused' as const, locked: true };

  it('shows the escape hatch only once the kiosk is engaged for the holder', () => {
    expect(shouldShowEscapeHatch(state({ focus }))).toBe(true);
    expect(shouldShowEscapeHatch(state({ focus: { ...focus, locked: false } }))).toBe(false);
  });

  it('locks navigation and reports the holder to other classes', () => {
    app.state = state({ focus });
    app.screen = 'idle';
    expect(app.locked).toBe(true);
    expect(app.isFocusLocked('study-1')).toBe(true);
    expect(app.isFocusLocked('study-2')).toBe(false);
    expect(app.heldByOtherClass('javascript')).toBe('typescript');
    expect(app.heldByOtherClass('typescript')).toBeNull();
    app.navigate('progress');
    expect(app.screen).toBe('idle');
    app.state = state();
    expect(app.locked).toBe(false);
    app.navigate('progress');
    expect(app.screen).toBe('dashboard');
    app.navigate('today');
  });
});
