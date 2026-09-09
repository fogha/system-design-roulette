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
    ...overrides,
  };
}

describe('authoritative screen routing', () => {
  it('routes away from a classroom when the enforced primary session becomes owed', () => {
    expect(resolveRoute(state({ owed: true }), 'classroom', false)).toEqual({
      screen: 'idle',
      stepAway: false,
    });
  });

  it('allows an unlocked voluntary primary session to remain stepped away', () => {
    const current = state({
      session: {
        ...state().session,
        status: 'in_progress',
        step: 'course',
      },
    });
    expect(resolveRoute(current, 'idle', true)).toEqual({
      screen: 'idle',
      stepAway: true,
    });
  });

  it('clears step-away and restores the primary step when locked', () => {
    const current = state({
      session: {
        ...state().session,
        status: 'in_progress',
        step: 'course',
        locked: true,
      },
    });
    expect(resolveRoute(current, 'classroom', true)).toEqual({
      screen: 'course',
      stepAway: false,
    });
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

it('keeps the latest session when refreshes and timer events arrive out of order', async () => {
  const first = state();
  const second = state({ session: { ...first.session, session_id: 'primary-next', status: 'in_progress', step: 'course' } });
  const getState = vi.spyOn(api, 'getAppState').mockResolvedValue(first);
  vi.spyOn(api, 'markFrontendReady').mockResolvedValue(undefined);
  try {
    await app.init();
    mockEmit('timer:tick', { session_id: first.session.session_id, remaining: 17 });
    expect(app.timerRemaining).toBe(17);

    let releaseOld!: (value: AppStateView) => void;
    getState.mockImplementationOnce(() => new Promise((resolve) => { releaseOld = resolve; }));
    const oldRefresh = app.refresh();
    getState.mockResolvedValueOnce(second);
    await app.refresh();
    expect(app.timerRemaining).toBe(-1);
    mockEmit('timer:tick', { session_id: first.session.session_id, remaining: 0 });
    expect(app.timerRemaining).toBe(-1);
    mockEmit('timer:tick', { session_id: second.session.session_id, remaining: 29 });
    expect(app.timerRemaining).toBe(29);

    releaseOld(first);
    await oldRefresh;
    expect(app.session?.session_id).toBe(second.session.session_id);
    expect(app.screen).toBe('course');
    expect(app.timerRemaining).toBe(29);
  } finally {
    vi.restoreAllMocks();
  }
});
