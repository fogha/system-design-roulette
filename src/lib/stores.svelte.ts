import {
  api,
  onEvent,
  type AppStateView,
  type ClassroomSessionStart,
  type ClassroomSubjectId,
  type EngineeringLessonView,
  type LanguageLessonView,
  type SessionView,
} from './ipc';
import type { ClassTab } from './features/classes/class-navigation';
import type { Destination } from './app/navigation';

export type Screen =
  | 'loading'
  | 'setup'
  | 'idle'
  | 'language'
  | 'classroom'
  | 'dashboard';

export interface RouteDecision {
  screen: Screen;
}

/** The retired daily routine no longer opens screens: a saved primary session
 *  is recovery data, so routing follows onboarding and owed state only. */
export function resolveRoute(state: AppStateView, currentScreen: Screen): RouteDecision {
  if (!state.onboarded) return { screen: 'setup' };
  if (state.owed) return { screen: 'idle' };
  return {
    screen: currentScreen === 'loading' || currentScreen === 'setup' ? 'idle' : currentScreen,
  };
}

export function shouldShowEscapeHatch(state: AppStateView | null): boolean {
  if (!state) return false;
  return (
    state.owed ||
    state.session.locked ||
    state.session.status === 'in_progress' ||
    !!state.focus?.locked
  );
}

class AppStore {
  private refreshRequest = 0;
  state = $state<AppStateView | null>(null);
  screen = $state<Screen>('loading');
  destination = $state<Destination>('today');
  classSelection = $state<ClassroomSubjectId | null>(null);
  classTab = $state<ClassTab>('overview');
  openClass(subjectId: ClassroomSubjectId | null = null, tab: ClassTab = 'overview') {
    this.classSelection = subjectId;
    this.classTab = tab;
    this.navigate('classes');
  }
  genStatus = $state<string>('');
  genLog = $state<string[]>([]);
  preparingClass = $state<{ subjectId: ClassroomSubjectId; label: string; agent: string; model: string; startedAt: number } | null>(null);
  error = $state<string>('');
  languageLesson = $state<LanguageLessonView | null>(null);
  engineeringLesson = $state<EngineeringLessonView | null>(null);

  get session(): SessionView | null {
    return this.state?.session ?? null;
  }

  /** True while the desk is enforced by a legacy day or a focused class session. */
  get locked(): boolean {
    return !!(this.session?.locked || this.state?.focus?.locked);
  }

  /** Whether the focus coordinator currently locks the given study session. */
  isFocusLocked(sessionId: string | undefined): boolean {
    const focus = this.state?.focus;
    return !!focus && focus.locked && focus.session_id === sessionId;
  }

  /** The class whose focused session holds the desk, when it is not `subjectId`. */
  heldByOtherClass(subjectId: string): string | null {
    const focus = this.state?.focus;
    return focus && focus.course_id !== subjectId ? focus.course_id : null;
  }

  navigate(destination: Destination) {
    if (this.locked) return;
    this.destination = destination;
    this.screen = destination === 'progress' ? 'dashboard' : 'idle';
  }

  async refresh() {
    const request = ++this.refreshRequest;
    try {
      const next = await api.getAppState();
      if (request !== this.refreshRequest) return;
      this.state = next;
      this.route();
      this.followFocus();
    } catch (e) {
      if (request === this.refreshRequest) this.error = String(e);
    }
  }

  /** Decide which screen to show from authoritative Rust state. */
  route() {
    const s = this.state;
    if (!s) return;
    this.screen = resolveRoute(s, this.screen).screen;
  }

  private focusFollowed: string | null = null;

  /** A locked focused class session belongs on screen: reopen it after a
   *  restart or when the desk was shown elsewhere. One attempt per session. */
  private followFocus() {
    const focus = this.state?.focus;
    if (!focus?.locked) {
      this.focusFollowed = null;
      return;
    }
    const language = this.languageLesson?.session_id === focus.session_id;
    if (language || this.engineeringLesson?.session_id === focus.session_id) {
      if (this.screen !== 'classroom' && this.screen !== 'language') {
        this.screen = language ? 'language' : 'classroom';
      }
      return;
    }
    if (this.focusFollowed === focus.session_id) return;
    this.focusFollowed = focus.session_id;
    void this.resumeClass(focus.course_id as ClassroomSubjectId);
  }

  /** After the escape hatch trips: a class lesson leaves the screen and the
   *  authoritative state decides what is shown next. */
  async escaped() {
    if (this.screen === 'classroom' || this.screen === 'language') {
      this.languageLesson = null;
      this.engineeringLesson = null;
      this.screen = 'idle';
    }
    this.focusFollowed = null;
    await this.refresh();
  }

  async startClass(subjectId: ClassroomSubjectId, slotId?: number | null, revisit = false, occurrenceId?: string | null) {
    if (this.preparingClass) return;
    const program = this.state?.classroom_programs.find(p => p.subject_id === subjectId);
    this.preparingClass = { subjectId, label: program?.label ?? subjectId, agent: program?.agent ?? '', model: program?.model ?? '', startedAt: Date.now() };
    this.error = '';
    this.genLog = [];
    try {
      const session = await api.startClassroomSession(subjectId, slotId, revisit, occurrenceId);
      this.showLesson(session);
    } catch (e) {
      this.error = String(e);
    } finally {
      this.preparingClass = null;
    }
  }

  /** Delayed retrieval for a class with review due; prepared without a provider. */
  async startReview(subjectId: ClassroomSubjectId) {
    if (this.preparingClass) return;
    this.error = '';
    try {
      this.showLesson(await api.startClassReview(subjectId));
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
      this.showLesson(session);
    } catch (e) {
      this.error = String(e);
    }
  }

  /** Put an opened lesson on screen, then reload the authoritative state:
   *  activation may have engaged focus, which the lesson header reflects. */
  private showLesson(session: ClassroomSessionStart) {
    if (session.kind === 'language') {
      this.languageLesson = session.lesson;
      this.screen = 'language';
    } else {
      this.engineeringLesson = session.lesson;
      this.screen = 'classroom';
    }
    void this.refresh();
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
  }
}

export const app = new AppStore();
