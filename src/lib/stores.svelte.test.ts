import { describe, expect, it } from 'vitest';
import type { AppStateView } from './ipc';
import { resolveRoute, shouldShowEscapeHatch } from './stores.svelte';

function state(overrides: Partial<AppStateView> = {}): AppStateView {
  return {
    onboarded: true,
    session: {
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
    language_programs: [],
    language_slots: [],
    language_due_count: 0,
    active_language_session: null,
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
