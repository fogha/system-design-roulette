import {
  api,
  onEvent,
  type AppStateView,
  type ClassroomSubjectId,
  type EngineeringLessonView,
  type LanguageLessonView,
  type SessionView,
} from './ipc';
import type { Destination } from './app/navigation';

export type Screen =
  | 'loading'
  | 'setup'
  | 'idle'
  | 'quiz'
  | 'review'
  | 'roulette'
  | 'course'
  | 'completion'
  | 'language'
  | 'classroom'
  | 'dashboard';

export interface RouteDecision {
  screen: Screen;
  stepAway: boolean;
}

export function resolveRoute(
  state: AppStateView,
  currentScreen: Screen,
  currentStepAway: boolean,
): RouteDecision {
  if (!state.onboarded) return { screen: 'setup', stepAway: false };
  const session = state.session;
  if (session.status === 'in_progress') {
    const stepAway = session.locked || state.owed ? false : currentStepAway;
    if (stepAway) {
      return {
        screen: currentScreen === 'loading' ? 'idle' : currentScreen,
        stepAway,
      };
    }
    return {
      screen: session.step === 'done' ? 'completion' : (session.step as Screen),
      stepAway: false,
    };
  }
  if (
    (session.status === 'completed' || session.status === 'skipped') &&
    !['dashboard', 'idle', 'language', 'classroom'].includes(currentScreen)
  ) {
    return { screen: 'completion', stepAway: false };
  }
  if (state.owed) return { screen: 'idle', stepAway: false };
  return {
    screen:
      currentScreen === 'loading' || currentScreen === 'setup' ? 'idle' : currentScreen,
    stepAway: false,
  };
}

export function shouldShowEscapeHatch(state: AppStateView | null): boolean {
  if (!state) return false;
  return state.owed || state.session.locked || state.session.status === 'in_progress';
}

class AppStore {
  private refreshRequest = 0;
  state = $state<AppStateView | null>(null);
  screen = $state<Screen>('loading');
  destination = $state<Destination>('today');
  genStatus = $state<string>('');
  genLog = $state<string[]>([]);
  timerRemaining = $state<number>(-1);
  error = $state<string>('');
  languageLesson = $state<LanguageLessonView | null>(null);
  engineeringLesson = $state<EngineeringLessonView | null>(null);
  /** User stepped away from an UNLOCKED in-progress session (early start /
   *  extension). Cleared the moment the session is owed or locked. */
  stepAway = $state(false);

  get session(): SessionView | null {
    return this.state?.session ?? null;
  }

  navigate(destination: Destination) {
    if (this.session?.locked) return;
    this.destination = destination;
    this.screen = destination === 'progress' ? 'dashboard' : 'idle';
  }

  async refresh() {
    const request = ++this.refreshRequest;
    try {
      const next = await api.getAppState();
      if (request !== this.refreshRequest) return;
      if (next.session.session_id !== this.session?.session_id) this.timerRemaining = -1;
      this.state = next;
      this.route();
    } catch (e) {
      if (request === this.refreshRequest) this.error = String(e);
    }
  }

  /** Decide which screen to show from authoritative Rust state. */
  route() {
    const s = this.state;
    if (!s) return;
    const decision = resolveRoute(s, this.screen, this.stepAway);
    this.screen = decision.screen;
    this.stepAway = decision.stepAway;
  }

  /** Leave an unlocked in-progress session for the idle/dashboard screens. */
  leaveSession() {
    if (this.session?.locked) return;
    this.stepAway = true;
    this.screen = 'idle';
  }

  /** Return to the in-progress session at its saved step. */
  resumeSession() {
    this.stepAway = false;
    this.route();
  }

  async startClass(subjectId: ClassroomSubjectId, slotId?: number | null, revisit = false) {
    try {
      const session = await api.startClassroomSession(subjectId, slotId, revisit);
      if (session.kind === 'language') {
        this.languageLesson = session.lesson;
        this.screen = 'language';
      } else {
        this.engineeringLesson = session.lesson;
        this.screen = 'classroom';
      }
    } catch (e) {
      this.error = String(e);
    }
  }

  async resumeClass(subjectId: ClassroomSubjectId) {
    try {
      const session = await api.resumeClassroomSession(subjectId);
      if (!session) {
        await this.refresh();
        return;
      }
      if (session.kind === 'language') {
        this.languageLesson = session.lesson;
        this.screen = 'language';
      } else {
        this.engineeringLesson = session.lesson;
        this.screen = 'classroom';
      }
    } catch (e) {
      this.error = String(e);
    }
  }

  async finishClass() {
    this.languageLesson = null;
    this.engineeringLesson = null;
    this.screen = 'idle';
    await this.refresh();
  }

  async finishLanguage() {
    await this.finishClass();
  }

  async init() {
    // Tell Rust the webview booted — the kiosk refuses to lock before this.
    await api.markFrontendReady().catch(() => {});
    await this.refresh();
    await onEvent('session:owed', () => this.refresh());
    await onEvent('session:state', () => this.refresh());
    await onEvent('classroom:owed', () => this.refresh());
    await onEvent('classroom:state', () => this.refresh());
    await onEvent<string>('gen:status', (msg) => {
      this.genStatus = msg;
    });
    await onEvent<string>('gen:log', (line) => {
      const ts = new Date().toTimeString().slice(0, 8);
      this.genLog = [...this.genLog.slice(-49), `${ts}  ${line}`];
    });
    await onEvent<{ session_id: string; remaining: number }>('timer:tick', (tick) => {
      if (tick.session_id === this.session?.session_id) this.timerRemaining = tick.remaining;
    });
  }
}

export const app = new AppStore();
